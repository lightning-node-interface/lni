import { LniError } from '../errors.js';
import { buildUrl, requestJson, requestText, resolveFetch, toTimeoutMs } from '../internal/http.js';
import { pollInvoiceEvents } from '../internal/polling.js';
import { emptyNodeInfo, emptyTransaction, parseOptionalNumber, rHashToHex } from '../internal/transform.js';
import { InvoiceType, type CreateInvoiceParams, type CreateOfferParams, type InvoiceEventCallback, type LightningNode, type ListTransactionsParams, type LookupInvoiceParams, type LndConfig, type NodeInfo, type NodeRequestOptions, type Offer, type OnInvoiceEventParams, type PayInvoiceParams, type PayInvoiceResponse, type Transaction } from '../types.js';

interface LndGetInfoResponse {
  alias: string;
  color: string;
  identity_pubkey: string;
  block_height: number;
  block_hash: string;
  chains: Array<{ network: string }>;
}

interface LndBalancesResponse {
  local_balance?: { msat?: string };
  remote_balance?: { msat?: string };
  unsettled_local_balance?: { msat?: string };
  unsettled_remote_balance?: { msat?: string };
  pending_open_local_balance?: { msat?: string };
  pending_open_remote_balance?: { msat?: string };
}

interface LndCreateInvoiceResponse {
  r_hash: string;
  payment_request: string;
}

interface LndInvoiceResponse {
  memo?: string;
  r_preimage?: string;
  r_hash?: string;
  value_msat?: string;
  creation_date?: string;
  settle_date?: string;
  payment_request?: string;
  description_hash?: string;
  expiry?: string;
  amt_paid_msat?: string;
}

interface LndInvoiceListResponse {
  invoices: LndInvoiceResponse[];
}

interface LndPayResult {
  payment_hash: string;
  payment_preimage: string;
  fee_msat: string;
  status: string;
  failure_reason?: string;
}

interface LndPayResponseWrapper {
  result?: LndPayResult;
  error?: {
    message?: string;
  };
}

export class LndNode implements LightningNode {
  private readonly fetchFn;
  private readonly timeoutMs?: number;

  constructor(private readonly config: LndConfig, options: NodeRequestOptions = {}) {
    this.fetchFn = resolveFetch(options.fetch);
    this.timeoutMs = toTimeoutMs(config.httpTimeout);
  }

  private headers(extra?: HeadersInit): HeadersInit {
    return {
      'grpc-metadata-macaroon': this.config.macaroon,
      ...(extra ?? {}),
    };
  }

  private async getJson<T>(path: string): Promise<T> {
    return requestJson<T>(this.fetchFn, buildUrl(this.config.url, path), {
      method: 'GET',
      headers: this.headers(),
      timeoutMs: this.timeoutMs,
    });
  }

  private async postJson<T>(path: string, json: unknown): Promise<T> {
    return requestJson<T>(this.fetchFn, buildUrl(this.config.url, path), {
      method: 'POST',
      headers: this.headers({ 'content-type': 'application/json' }),
      json,
      timeoutMs: this.timeoutMs,
    });
  }

  private isPermissionDenied(error: unknown): boolean {
    if (!(error instanceof LniError)) {
      return false;
    }

    if (error.code !== 'Http') {
      return false;
    }

    const details = `${error.message} ${error.body ?? ''}`.toLowerCase();
    return details.includes('permission denied');
  }

  private mapInvoice(invoice: LndInvoiceResponse): Transaction {
    return emptyTransaction({
      type: 'incoming',
      invoice: invoice.payment_request ?? '',
      preimage: rHashToHex(invoice.r_preimage ?? ''),
      paymentHash: rHashToHex(invoice.r_hash ?? ''),
      amountMsats: parseOptionalNumber(invoice.amt_paid_msat),
      feesPaid: parseOptionalNumber(invoice.value_msat),
      createdAt: parseOptionalNumber(invoice.creation_date),
      expiresAt: parseOptionalNumber(invoice.expiry),
      settledAt: parseOptionalNumber(invoice.settle_date),
      description: invoice.memo ?? '',
      descriptionHash: invoice.description_hash ?? '',
      payerNote: '',
      externalId: '',
    });
  }

  async getInfo(): Promise<NodeInfo> {
    const info = await this.getJson<LndGetInfoResponse>('/v1/getinfo');

    let balances: LndBalancesResponse = {};
    try {
      balances = await this.getJson<LndBalancesResponse>('/v1/balance/channels');
    } catch (error) {
      if (!this.isPermissionDenied(error)) {
        throw error;
      }
    }

    return emptyNodeInfo({
      alias: info.alias,
      color: info.color,
      pubkey: info.identity_pubkey,
      network: info.chains[0]?.network ?? '',
      blockHeight: info.block_height,
      blockHash: info.block_hash,
      sendBalanceMsat: parseOptionalNumber(balances.local_balance?.msat),
      receiveBalanceMsat: parseOptionalNumber(balances.remote_balance?.msat),
      unsettledSendBalanceMsat: parseOptionalNumber(balances.unsettled_local_balance?.msat),
      unsettledReceiveBalanceMsat: parseOptionalNumber(balances.unsettled_remote_balance?.msat),
      pendingOpenSendBalance: parseOptionalNumber(balances.pending_open_local_balance?.msat),
      pendingOpenReceiveBalance: parseOptionalNumber(balances.pending_open_remote_balance?.msat),
    });
  }

  async createInvoice(params: CreateInvoiceParams): Promise<Transaction> {
    if ((params.invoiceType ?? InvoiceType.Bolt11) !== InvoiceType.Bolt11) {
      throw new LniError('Api', 'Bolt12 is not implemented for LndNode.');
    }

    const payload = await this.postJson<LndCreateInvoiceResponse>('/v1/invoices', {
      value_msat: params.amountMsats ?? 0,
      memo: params.description ?? '',
      expiry: params.expiry ?? 86400,
      private: params.isPrivate ?? false,
      ...(params.rPreimage ? { r_preimage: params.rPreimage } : {}),
      ...(params.isBlinded ? { is_blinded: true } : {}),
    });

    return emptyTransaction({
      type: 'incoming',
      invoice: payload.payment_request,
      paymentHash: rHashToHex(payload.r_hash),
      amountMsats: params.amountMsats ?? 0,
      expiresAt: params.expiry ?? 86400,
      description: params.description ?? '',
      descriptionHash: params.descriptionHash ?? '',
      payerNote: '',
      externalId: '',
    });
  }

  async payInvoice(params: PayInvoiceParams): Promise<PayInvoiceResponse> {
    const body: Record<string, unknown> = {
      payment_request: params.invoice,
      allow_self_payment: params.allowSelfPayment ?? false,
      timeout_seconds: params.timeoutSeconds ?? 60,
    };

    if (params.feeLimitPercentage !== undefined && params.amountMsats !== undefined) {
      body.fee_limit = {
        fixed_msat: String(params.amountMsats),
        percent: params.feeLimitPercentage,
      };
    }

    const responseText = await requestText(this.fetchFn, buildUrl(this.config.url, '/v2/router/send'), {
      method: 'POST',
      headers: this.headers({ 'content-type': 'application/json' }),
      json: body,
      timeoutMs: this.timeoutMs,
    });

    const finalLine = responseText
      .split('\n')
      .map((line) => line.trim())
      .filter(Boolean)
      .at(-1);

    if (!finalLine) {
      throw new LniError('Json', 'Missing payment response from LND router endpoint.');
    }

    let wrapped: LndPayResponseWrapper;
    try {
      wrapped = JSON.parse(finalLine) as LndPayResponseWrapper;
    } catch (error) {
      throw new LniError('Json', `Failed to parse LND pay response: ${(error as Error).message}`);
    }

    if (wrapped.error) {
      throw new LniError('Api', `Payment failed: ${wrapped.error.message ?? 'unknown reason'}`);
    }

    if (!wrapped.result) {
      throw new LniError('Json', 'Missing result payload in LND pay response.');
    }

    if (wrapped.result.status === 'FAILED') {
      throw new LniError('Api', `Payment failed: ${wrapped.result.failure_reason ?? 'unknown reason'}`);
    }

    if (wrapped.result.status === 'IN_FLIGHT') {
      throw new LniError('Api', 'Payment is still in-flight. Increase timeoutSeconds and retry.');
    }

    if (wrapped.result.status !== 'SUCCEEDED') {
      throw new LniError('Api', `Unknown payment status: ${wrapped.result.status}`);
    }

    return {
      paymentHash: wrapped.result.payment_hash,
      preimage: wrapped.result.payment_preimage,
      feeMsats: parseOptionalNumber(wrapped.result.fee_msat),
    };
  }

  async createOffer(_params: CreateOfferParams): Promise<Offer> {
    throw new LniError('Api', 'Bolt12 is not implemented for LndNode.');
  }

  async getOffer(_search?: string): Promise<Offer> {
    throw new LniError('Api', 'Bolt12 is not implemented for LndNode.');
  }

  async listOffers(_search?: string): Promise<Offer[]> {
    throw new LniError('Api', 'Bolt12 is not implemented for LndNode.');
  }

  async payOffer(_offer: string, _amountMsats: number, _payerNote?: string): Promise<PayInvoiceResponse> {
    throw new LniError('Api', 'Bolt12 is not implemented for LndNode.');
  }

  async lookupInvoice(params: LookupInvoiceParams): Promise<Transaction> {
    if (!params.paymentHash) {
      throw new LniError('InvalidInput', 'lookupInvoice requires paymentHash for LndNode.');
    }

    const payload = await this.getJson<LndInvoiceResponse>(`/v1/invoice/${params.paymentHash}`);
    return this.mapInvoice(payload);
  }

  async listTransactions(_params: ListTransactionsParams): Promise<Transaction[]> {
    const payload = await this.getJson<LndInvoiceListResponse>('/v1/invoices');
    return payload.invoices.map((invoice) => this.mapInvoice(invoice)).sort((a, b) => b.createdAt - a.createdAt);
  }

  async decode(str: string): Promise<string> {
    return requestText(this.fetchFn, buildUrl(this.config.url, `/v1/payreq/${encodeURIComponent(str)}`), {
      method: 'GET',
      headers: this.headers(),
      timeoutMs: this.timeoutMs,
    });
  }

  async onInvoiceEvents(params: OnInvoiceEventParams, callback: InvoiceEventCallback): Promise<void> {
    await pollInvoiceEvents({
      params,
      callback,
      lookup: () => this.lookupInvoice({ paymentHash: params.paymentHash, search: params.search }),
    });
  }
}
