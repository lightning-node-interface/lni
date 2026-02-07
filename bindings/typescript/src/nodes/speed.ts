import { LniError } from '../errors.js';
import { buildUrl, requestJson, resolveFetch, toTimeoutMs } from '../internal/http.js';
import { pollInvoiceEvents } from '../internal/polling.js';
import { emptyNodeInfo, emptyTransaction, satsToMsats } from '../internal/transform.js';
import { encodeBase64 } from '../internal/encoding.js';
import { InvoiceType, type CreateInvoiceParams, type CreateOfferParams, type InvoiceEventCallback, type LightningNode, type ListTransactionsParams, type LookupInvoiceParams, type NodeInfo, type NodeRequestOptions, type Offer, type OnInvoiceEventParams, type PayInvoiceParams, type PayInvoiceResponse, type SpeedConfig, type Transaction } from '../types.js';

interface SpeedBalanceResponse {
  available: Array<{
    amount: number;
    target_currency: string;
  }>;
}

interface SpeedCreatePaymentResponse {
  id: string;
  amount: number;
  created: number;
  modified?: number;
  statement_descriptor?: string;
  target_amount_paid_at?: number;
  speed_fee?: { amount?: number };
  payment_method_options?: {
    lightning?: {
      payment_request?: string;
      payment_hash?: string;
    };
  };
}

interface SpeedSendResponse {
  id: string;
  status: string;
  target_amount: number;
  withdraw_method: string;
  withdraw_request: string;
  note?: string;
  created: number;
  modified?: number;
  speed_fee: { amount: number };
}

interface SpeedSendFilterResponse {
  data: SpeedSendResponse[];
}

export class SpeedNode implements LightningNode {
  private readonly fetchFn;
  private readonly timeoutMs?: number;
  private readonly baseUrl: string;

  constructor(private readonly config: SpeedConfig, options: NodeRequestOptions = {}) {
    this.fetchFn = resolveFetch(options.fetch);
    this.timeoutMs = toTimeoutMs(config.httpTimeout);
    this.baseUrl = config.baseUrl ?? 'https://api.tryspeed.com';
  }

  private headers(extra?: HeadersInit): HeadersInit {
    const auth = encodeBase64(`${this.config.apiKey}:`);
    return {
      authorization: `Basic ${auth}`,
      'content-type': 'application/json',
      ...(extra ?? {}),
    };
  }

  private async getJson<T>(path: string, query?: Record<string, string | number | undefined>): Promise<T> {
    return requestJson<T>(this.fetchFn, buildUrl(this.baseUrl, path, query), {
      method: 'GET',
      headers: this.headers(),
      timeoutMs: this.timeoutMs,
    });
  }

  private async postJson<T>(path: string, json?: unknown): Promise<T> {
    return requestJson<T>(this.fetchFn, buildUrl(this.baseUrl, path), {
      method: 'POST',
      headers: this.headers(),
      json,
      timeoutMs: this.timeoutMs,
    });
  }

  private async fetchSendTransactions(status?: string[], withdrawRequest?: string): Promise<SpeedSendResponse[]> {
    const payload = await this.postJson<SpeedSendFilterResponse>('/send/filter', {
      status,
      withdraw_request: withdrawRequest,
    });

    return payload.data;
  }

  private sendToTransaction(send: SpeedSendResponse): Transaction {
    return emptyTransaction({
      type: send.withdraw_method === 'lightning' ? 'outgoing' : 'outgoing',
      invoice: send.withdraw_request,
      paymentHash: '',
      amountMsats: satsToMsats(send.target_amount),
      feesPaid: satsToMsats(send.speed_fee.amount),
      createdAt: send.created,
      settledAt: send.status === 'paid' ? send.modified ?? send.created : 0,
      description: send.note ?? '',
      descriptionHash: '',
      payerNote: send.note,
      externalId: send.id,
    });
  }

  async getInfo(): Promise<NodeInfo> {
    const payload = await this.getJson<SpeedBalanceResponse>('/balances');
    const sats = payload.available.find((item) => item.target_currency === 'SATS');

    return emptyNodeInfo({
      alias: 'Speed Node',
      network: 'mainnet',
      sendBalanceMsat: sats ? satsToMsats(sats.amount) : 0,
    });
  }

  async createInvoice(params: CreateInvoiceParams): Promise<Transaction> {
    if ((params.invoiceType ?? InvoiceType.Bolt11) !== InvoiceType.Bolt11) {
      throw new LniError('Api', 'Bolt12 is not implemented for SpeedNode.');
    }

    const payload = await this.postJson<SpeedCreatePaymentResponse>('/payments', {
      amount: (params.amountMsats ?? 0) / 1000,
      currency: 'SATS',
      memo: params.description,
      external_id: null,
    });

    const lightning = payload.payment_method_options?.lightning;

    return emptyTransaction({
      type: 'incoming',
      invoice: lightning?.payment_request ?? '',
      paymentHash: lightning?.payment_hash ?? '',
      amountMsats: satsToMsats(payload.amount),
      feesPaid: satsToMsats(payload.speed_fee?.amount ?? 0),
      createdAt: payload.created,
      settledAt: payload.target_amount_paid_at ?? 0,
      expiresAt: 0,
      description: payload.statement_descriptor ?? params.description ?? '',
      descriptionHash: params.descriptionHash ?? '',
      payerNote: '',
      externalId: payload.id,
    });
  }

  async payInvoice(params: PayInvoiceParams): Promise<PayInvoiceResponse> {
    if (params.amountMsats === undefined) {
      throw new LniError('InvalidInput', 'Speed payInvoice requires amountMsats for frontend mode.');
    }

    const payload = await this.postJson<SpeedSendResponse>('/send', {
      amount: params.amountMsats / 1000,
      currency: 'SATS',
      target_currency: 'SATS',
      withdraw_method: 'lightning',
      withdraw_request: params.invoice,
      note: 'LNI payment',
      external_id: null,
    });

    return {
      paymentHash: '',
      preimage: '',
      feeMsats: satsToMsats(payload.speed_fee.amount),
    };
  }

  async createOffer(_params: CreateOfferParams): Promise<Offer> {
    throw new LniError('Api', 'Bolt12 is not implemented for SpeedNode.');
  }

  async getOffer(_search?: string): Promise<Offer> {
    throw new LniError('Api', 'Bolt12 is not implemented for SpeedNode.');
  }

  async listOffers(_search?: string): Promise<Offer[]> {
    throw new LniError('Api', 'Bolt12 is not implemented for SpeedNode.');
  }

  async payOffer(_offer: string, _amountMsats: number, _payerNote?: string): Promise<PayInvoiceResponse> {
    throw new LniError('Api', 'Bolt12 is not implemented for SpeedNode.');
  }

  async lookupInvoice(params: LookupInvoiceParams): Promise<Transaction> {
    const rows = await this.fetchSendTransactions(
      ['paid', 'unpaid', 'failed'],
      params.search,
    );

    const txs = rows.map((row) => this.sendToTransaction(row));

    if (params.paymentHash) {
      const tx = txs.find((candidate) => candidate.paymentHash === params.paymentHash);
      if (!tx) {
        throw new LniError('Api', `Transaction not found for payment hash: ${params.paymentHash}`);
      }
      return tx;
    }

    if (!txs.length) {
      throw new LniError('Api', 'No transactions found matching lookup parameters.');
    }

    return txs[0]!;
  }

  async listTransactions(params: ListTransactionsParams): Promise<Transaction[]> {
    const rows = await this.fetchSendTransactions(
      params.search ? undefined : ['unpaid', 'paid', 'failed'],
      params.search,
    );

    const txs = rows
      .map((row) => this.sendToTransaction(row))
      .sort((a, b) => b.createdAt - a.createdAt)
      .slice(0, params.limit > 0 ? params.limit : undefined);

    if (!params.paymentHash) {
      return txs;
    }

    return txs.filter((tx) => tx.paymentHash === params.paymentHash);
  }

  async decode(str: string): Promise<string> {
    return str;
  }

  async onInvoiceEvents(params: OnInvoiceEventParams, callback: InvoiceEventCallback): Promise<void> {
    await pollInvoiceEvents({
      params,
      callback,
      lookup: () => this.lookupInvoice({ paymentHash: params.paymentHash, search: params.search }),
    });
  }
}
