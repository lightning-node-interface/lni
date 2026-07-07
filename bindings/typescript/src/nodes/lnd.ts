import { decodeBolt11ToJson, decodeOfferToJson } from '../decode.js';
import { LniError, NwcError, type NwcErrorCode, type NwcErrorOperation } from '../errors.js';
import { encodeBase64Bytes, hexToBytes } from '../internal/encoding.js';
import { mapProviderMessage, providerInfoFromJsonErrorBody, throwNormalizedProviderError, type ProviderErrorInfo } from '../internal/error-normalization.js';
import { buildUrl, requestJson, requestText, resolveFetch, toTimeoutMs } from '../internal/http.js';
import { isEmptyPermissions, normalizeLndPermissions, parseLndMacaroonPermissions } from '../internal/permissions.js';
import { pollInvoiceEvents } from '../internal/polling.js';
import { emptyNodeInfo, emptyTransaction, parseOptionalNumber, rHashToHex } from '../internal/transform.js';
import { InvoiceType, type CreateInvoiceParams, type CreateOfferParams, type InvoiceEventCallback, type LightningNode, type ListTransactionsParams, type LookupInvoiceParams, type LndConfig, type NodeInfo, type NodeRequestOptions, type Offer, type OnInvoiceEventParams, type PayInvoiceParams, type PayInvoiceResponse, type Permissions, type Transaction } from '../types.js';

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
    code?: number | string;
    message?: string;
  };
}

interface LndMacaroonPermission {
  entity: string;
  action: string;
}

interface LndPermissionList {
  permissions?: LndMacaroonPermission[];
}

interface LndListPermissionsResponse {
  method_permissions?: Record<string, LndPermissionList>;
}

interface LndCheckMacaroonPermissionsResponse {
  valid?: boolean;
}

function mapLndProviderError(info: ProviderErrorInfo): NwcErrorCode | undefined {
  const code = String(info.code ?? '').toLowerCase();
  const message = info.message?.toLowerCase() ?? '';

  if (code.includes('unauthenticated') || message.includes('unauthenticated')) {
    return 'UNAUTHORIZED';
  }
  if (code.includes('permission_denied') || message.includes('permission denied')) {
    return 'RESTRICTED';
  }

  return mapProviderMessage(info.message);
}

function mapLndFailureReason(reason: string | undefined): NwcErrorCode {
  switch (reason) {
    case 'FAILURE_REASON_INSUFFICIENT_BALANCE':
      return 'INSUFFICIENT_BALANCE';
    case 'FAILURE_REASON_NO_ROUTE':
    case 'FAILURE_REASON_TIMEOUT':
    case 'FAILURE_REASON_INCORRECT_PAYMENT_DETAILS':
    case 'FAILURE_REASON_ERROR':
      return 'PAYMENT_FAILED';
    case 'FAILURE_REASON_CANCELED':
    case 'FAILURE_REASON_NONE':
    default:
      return 'OTHER';
  }
}

function throwLndError(error: unknown, operation: NwcErrorOperation): never {
  throwNormalizedProviderError(error, {
    provider: 'lnd',
    operation,
    extractProviderError: providerInfoFromJsonErrorBody,
    mapProviderError: mapLndProviderError,
  });
}

function lndNwcError(
  code: NwcErrorCode,
  message: string,
  operation: NwcErrorOperation,
  providerCode?: string | number,
): NwcError {
  return new NwcError(code, message, {
    operation,
    provider: 'lnd',
    providerCode,
    providerMessage: message,
  });
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

  private async getJson<T>(path: string, operation?: NwcErrorOperation): Promise<T> {
    try {
      return await requestJson<T>(this.fetchFn, buildUrl(this.config.url, path), {
        method: 'GET',
        headers: this.headers(),
        timeoutMs: this.timeoutMs,
      });
    } catch (error) {
      if (operation) {
        throwLndError(error, operation);
      }
      throw error;
    }
  }

  private async postJson<T>(path: string, json: unknown, operation?: NwcErrorOperation): Promise<T> {
    try {
      return await requestJson<T>(this.fetchFn, buildUrl(this.config.url, path), {
        method: 'POST',
        headers: this.headers({ 'content-type': 'application/json' }),
        json,
        timeoutMs: this.timeoutMs,
      });
    } catch (error) {
      if (operation) {
        throwLndError(error, operation);
      }
      throw error;
    }
  }

  async getPermissions(): Promise<Permissions> {
    const macaroonBytes = hexToBytes(this.config.macaroon);

    try {
      const payload = await this.getJson<LndListPermissionsResponse>('/v1/macaroon/permissions', 'get_info');
      const methodPermissions = Object.entries(payload.method_permissions ?? {});
      const macaroon = encodeBase64Bytes(macaroonBytes);
      const granted: string[] = [];

      await Promise.all(
        methodPermissions.map(async ([method, permissionList]) => {
          const response = await this.postJson<LndCheckMacaroonPermissionsResponse>('/v1/macaroon/checkpermissions', {
            macaroon,
            permissions: permissionList.permissions ?? [],
          }, 'get_info');

          if (response.valid) {
            granted.push(method);
          }
        }),
      );

      return normalizeLndPermissions(granted);
    } catch (error) {
      const parsed = parseLndMacaroonPermissions(macaroonBytes);
      if (!isEmptyPermissions(parsed)) {
        return parsed;
      }
      throw error;
    }
  }

  private isPermissionDenied(error: unknown): boolean {
    if (error instanceof NwcError) {
      return error.nwcCode === 'RESTRICTED';
    }

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
    const info = await this.getJson<LndGetInfoResponse>('/v1/getinfo', 'get_info');

    let balances: LndBalancesResponse = {};
    try {
      balances = await this.getJson<LndBalancesResponse>('/v1/balance/channels', 'get_info');
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
      throw lndNwcError('NOT_IMPLEMENTED', 'Bolt12 is not implemented for LndNode.', 'make_invoice');
    }

    const payload = await this.postJson<LndCreateInvoiceResponse>('/v1/invoices', {
      value_msat: params.amountMsats ?? 0,
      memo: params.description ?? '',
      expiry: params.expiry ?? 86400,
      private: params.isPrivate ?? false,
      ...(params.rPreimage ? { r_preimage: params.rPreimage } : {}),
      ...(params.isBlinded ? { is_blinded: true } : {}),
    }, 'make_invoice');

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

    let responseText: string;
    try {
      responseText = await requestText(this.fetchFn, buildUrl(this.config.url, '/v2/router/send'), {
        method: 'POST',
        headers: this.headers({ 'content-type': 'application/json' }),
        json: body,
        timeoutMs: this.timeoutMs,
      });
    } catch (error) {
      throwLndError(error, 'pay_invoice');
    }

    const finalLine = responseText
      .split('\n')
      .map((line) => line.trim())
      .filter(Boolean)
      .at(-1);

    if (!finalLine) {
      throw lndNwcError('INTERNAL', 'Missing payment response from LND router endpoint.', 'pay_invoice');
    }

    let wrapped: LndPayResponseWrapper;
    try {
      wrapped = JSON.parse(finalLine) as LndPayResponseWrapper;
    } catch (error) {
      throw lndNwcError('INTERNAL', `Failed to parse LND pay response: ${(error as Error).message}`, 'pay_invoice');
    }

    if (wrapped.error) {
      const message = wrapped.error.message ?? 'unknown reason';
      throw lndNwcError(mapProviderMessage(message) ?? 'PAYMENT_FAILED', `Payment failed: ${message}`, 'pay_invoice', wrapped.error.code);
    }

    if (!wrapped.result) {
      throw lndNwcError('INTERNAL', 'Missing result payload in LND pay response.', 'pay_invoice');
    }

    if (wrapped.result.status === 'FAILED') {
      const reason = wrapped.result.failure_reason ?? 'unknown reason';
      const mappedCode = wrapped.result.failure_reason
        ? mapLndFailureReason(wrapped.result.failure_reason)
        : undefined;
      const code = mappedCode && mappedCode !== 'OTHER' ? mappedCode : 'PAYMENT_FAILED';
      throw lndNwcError(code, `Payment failed: ${reason}`, 'pay_invoice', reason);
    }

    if (wrapped.result.status === 'IN_FLIGHT') {
      throw lndNwcError('OTHER', 'Payment is still in-flight. Increase timeoutSeconds and retry.', 'pay_invoice', wrapped.result.status);
    }

    if (wrapped.result.status !== 'SUCCEEDED') {
      throw lndNwcError('OTHER', `Unknown payment status: ${wrapped.result.status}`, 'pay_invoice', wrapped.result.status);
    }

    return {
      paymentHash: wrapped.result.payment_hash,
      preimage: wrapped.result.payment_preimage,
      feeMsats: parseOptionalNumber(wrapped.result.fee_msat),
    };
  }

  async createOffer(_params: CreateOfferParams): Promise<Offer> {
    throw lndNwcError('NOT_IMPLEMENTED', 'Bolt12 is not implemented for LndNode.', 'make_invoice');
  }

  async getOffer(_search?: string): Promise<Offer> {
    throw lndNwcError('NOT_IMPLEMENTED', 'Bolt12 is not implemented for LndNode.', 'lookup_invoice');
  }

  async listOffers(_search?: string): Promise<Offer[]> {
    throw lndNwcError('NOT_IMPLEMENTED', 'Bolt12 is not implemented for LndNode.', 'list_transactions');
  }

  async payOffer(_offer: string, _amountMsats: number, _payerNote?: string): Promise<PayInvoiceResponse> {
    throw lndNwcError('NOT_IMPLEMENTED', 'Bolt12 is not implemented for LndNode.', 'pay_invoice');
  }

  async lookupInvoice(params: LookupInvoiceParams): Promise<Transaction> {
    if (!params.paymentHash) {
      throw new LniError('InvalidInput', 'lookupInvoice requires paymentHash for LndNode.');
    }

    const payload = await this.getJson<LndInvoiceResponse>(`/v1/invoice/${params.paymentHash}`, 'lookup_invoice');
    return this.mapInvoice(payload);
  }

  async listTransactions(params: ListTransactionsParams): Promise<Transaction[]> {
    const payload = await this.getJson<LndInvoiceListResponse>('/v1/invoices', 'list_transactions');
    const sorted = payload.invoices
      .map((invoice) => this.mapInvoice(invoice))
      .sort((a, b) => b.createdAt - a.createdAt);

    const filtered = sorted.filter((tx) => {
      if (params.paymentHash && tx.paymentHash !== params.paymentHash) {
        return false;
      }

      if (!params.search) {
        return true;
      }

      const search = params.search.toLowerCase();
      return (
        tx.paymentHash.toLowerCase().includes(search) ||
        tx.description.toLowerCase().includes(search) ||
        tx.invoice.toLowerCase().includes(search)
      );
    });

    const from = Math.max(params.from, 0);
    const end = params.limit > 0 ? from + params.limit : undefined;
    return filtered.slice(from, end);
  }

  async decode(str: string): Promise<string> {
    return decodeBolt11ToJson(str);
  }

  async decodeOffer(offer: string): Promise<string> {
    return decodeOfferToJson(offer);
  }

  async onInvoiceEvents(params: OnInvoiceEventParams, callback: InvoiceEventCallback): Promise<void> {
    await pollInvoiceEvents({
      params,
      callback,
      lookup: () => this.lookupInvoice({ paymentHash: params.paymentHash, search: params.search }),
    });
  }
}
