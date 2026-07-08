import { LniError, NwcError, type NwcErrorCode, type NwcErrorOperation } from '../errors.js';
import { decodeBolt11ToJson, decodeOfferToJson } from '../decode.js';
import { isRecord, mapProviderMessage, providerInfoFromJsonErrorBody, throwNormalizedProviderError, type ProviderErrorInfo } from '../internal/error-normalization.js';
import { buildUrl, requestJson, requestText, resolveFetch, toTimeoutMs } from '../internal/http.js';
import { pollInvoiceEvents } from '../internal/polling.js';
import { emptyNodeInfo, emptyTransaction, matchesSearch, satsToMsats, toUnixSeconds } from '../internal/transform.js';
import { encodeBase64 } from '../internal/encoding.js';
import { InvoiceType, type CreateInvoiceParams, type CreateOfferParams, type InvoiceEventCallback, type LightningNode, type ListTransactionsParams, type LookupInvoiceParams, type NodeRequestOptions, type Offer, type OnInvoiceEventParams, type PayInvoiceParams, type PayInvoiceResponse, type Permissions, type PhoenixdConfig, type Transaction, type NodeInfo } from '../types.js';

interface PhoenixdInfoResponse {
  nodeId: string;
  channels: Array<{
    balanceSat: number;
    inboundLiquiditySat: number;
  }>;
}

interface PhoenixdBalanceResponse {
  feeCreditSat: number;
}

interface PhoenixdBolt11Response {
  serialized: string;
  paymentHash: string;
}

interface PhoenixdPayResponse {
  paymentHash: string;
  paymentPreimage: string;
  routingFeeSat: number;
}

interface PhoenixdInvoiceResponse {
  preimage: string;
  paymentHash: string;
  receivedSat: number;
  fees: number;
  completedAt?: number;
  createdAt: number;
  isPaid: boolean;
  invoice?: string;
  description?: string;
  payerNote?: string;
  externalId?: string;
}

interface PhoenixdOutgoingPaymentResponse {
  paymentId?: string;
  preimage?: string;
  paymentHash?: string;
  sent: number;
  fees: number;
  createdAt: number;
  completedAt: number;
  payerNote?: string;
  externalId?: string;
}

function mapPhoenixdProviderError(info: ProviderErrorInfo): NwcErrorCode | undefined {
  const messageCode = mapProviderMessage(info.message);
  if (messageCode) {
    return messageCode;
  }

  switch (info.status) {
    case 401:
      return 'UNAUTHORIZED';
    case 403:
      return 'RESTRICTED';
    case 404:
      return 'NOT_FOUND';
    default:
      return undefined;
  }
}

function throwPhoenixdError(error: unknown, operation: NwcErrorOperation): never {
  throwNormalizedProviderError(error, {
    provider: 'phoenixd',
    operation,
    extractProviderError: providerInfoFromJsonErrorBody,
    mapProviderError: mapPhoenixdProviderError,
  });
}

function phoenixdNwcError(
  code: NwcErrorCode,
  message: string,
  operation: NwcErrorOperation,
  info?: ProviderErrorInfo,
): NwcError {
  return new NwcError(code, message, {
    operation,
    provider: 'phoenixd',
    providerCode: info?.code,
    providerStatus: info?.status,
    providerMessage: info?.message ?? message,
  });
}

function phoenixdPaymentFailure(payload: unknown, operation: NwcErrorOperation): NwcError | undefined {
  if (!isRecord(payload)) {
    return undefined;
  }

  if (typeof payload.paymentHash === 'string' && typeof payload.paymentPreimage === 'string') {
    return undefined;
  }

  const message =
    typeof payload.message === 'string'
      ? payload.message
      : typeof payload.reason === 'string'
        ? payload.reason
        : typeof payload.error === 'string'
          ? payload.error
          : undefined;

  if (!message) {
    return undefined;
  }

  const info = {
    code: typeof payload.code === 'string' ? payload.code : undefined,
    message,
  };

  return phoenixdNwcError(mapPhoenixdProviderError(info) ?? 'PAYMENT_FAILED', message, operation, info);
}

export class PhoenixdNode implements LightningNode {
  private readonly fetchFn;
  private readonly timeoutMs?: number;

  constructor(private readonly config: PhoenixdConfig, options: NodeRequestOptions = {}) {
    this.fetchFn = resolveFetch(options.fetch);
    this.timeoutMs = toTimeoutMs(config.httpTimeout);
  }

  private authHeader(): string {
    return `Basic ${encodeBase64(`:${this.config.password}`)}`;
  }

  private async requestJson<T>(
    path: string,
    args: Parameters<typeof requestJson<T>>[2],
    operation?: NwcErrorOperation,
  ): Promise<T> {
    try {
      return await requestJson<T>(this.fetchFn, buildUrl(this.config.url, path, args?.query), {
        ...args,
        timeoutMs: args?.timeoutMs ?? this.timeoutMs,
        headers: {
          authorization: this.authHeader(),
          ...(args?.headers ?? {}),
        },
      });
    } catch (error) {
      if (operation) {
        throwPhoenixdError(error, operation);
      }
      throw error;
    }
  }

  private async requestText(
    path: string,
    args: Parameters<typeof requestText>[2],
    operation?: NwcErrorOperation,
  ): Promise<string> {
    try {
      return await requestText(this.fetchFn, buildUrl(this.config.url, path, args?.query), {
        ...args,
        timeoutMs: args?.timeoutMs ?? this.timeoutMs,
        headers: {
          authorization: this.authHeader(),
          ...(args?.headers ?? {}),
        },
      });
    } catch (error) {
      if (operation) {
        throwPhoenixdError(error, operation);
      }
      throw error;
    }
  }

  async getPermissions(): Promise<Permissions> {
    throw new LniError(
      'InvalidInput',
      'Phoenixd passwords cannot be introspected. Manually test permissions against Phoenixd REST endpoints.',
    );
  }

  async getInfo(): Promise<NodeInfo> {
    const [info, balance] = await Promise.all([
      this.requestJson<PhoenixdInfoResponse>('/getinfo', { method: 'GET' }, 'get_info'),
      this.requestJson<PhoenixdBalanceResponse>('/getbalance', { method: 'GET' }, 'get_info'),
    ]);

    const firstChannel = info.channels[0];

    return emptyNodeInfo({
      alias: 'Phoenixd',
      pubkey: info.nodeId,
      network: 'bitcoin',
      sendBalanceMsat: satsToMsats(firstChannel?.balanceSat ?? 0),
      receiveBalanceMsat: satsToMsats(firstChannel?.inboundLiquiditySat ?? 0),
      feeCreditBalanceMsat: satsToMsats(balance.feeCreditSat),
    });
  }

  async createInvoice(params: CreateInvoiceParams): Promise<Transaction> {
    const invoiceType = params.invoiceType ?? InvoiceType.Bolt11;

    if (invoiceType === InvoiceType.Bolt12) {
      const offer = await this.requestText('/createoffer', {
        method: 'POST',
        form: {
          description: params.description,
          amountSat: params.amountMsats ? Math.floor(params.amountMsats / 1000) : undefined,
        },
      }, 'make_invoice');

      return emptyTransaction({
        type: 'incoming',
        invoice: offer.trim(),
        amountMsats: params.amountMsats ?? 0,
        expiresAt: params.expiry ?? 3600,
        description: params.description ?? '',
        descriptionHash: params.descriptionHash ?? '',
        payerNote: '',
        externalId: '',
      });
    }

    const payload = await this.requestJson<PhoenixdBolt11Response>('/createinvoice', {
      method: 'POST',
      form: {
        amountSat: params.amountMsats ? Math.floor(params.amountMsats / 1000) : 0,
        expirySeconds: params.expiry ?? 3600,
        description: params.description,
      },
    }, 'make_invoice');

    return emptyTransaction({
      type: 'incoming',
      invoice: payload.serialized,
      paymentHash: payload.paymentHash,
      amountMsats: params.amountMsats ?? 0,
      expiresAt: params.expiry ?? 3600,
      description: params.description ?? '',
      descriptionHash: params.descriptionHash ?? '',
      payerNote: '',
      externalId: '',
    });
  }

  async payInvoice(params: PayInvoiceParams): Promise<PayInvoiceResponse> {
    const payload = await this.requestJson<PhoenixdPayResponse>('/payinvoice', {
      method: 'POST',
      form: {
        invoice: params.invoice,
        amountSat: params.amountMsats ? Math.floor(params.amountMsats / 1000) : undefined,
      },
    }, 'pay_invoice');

    const paymentFailure = phoenixdPaymentFailure(payload, 'pay_invoice');
    if (paymentFailure) {
      throw paymentFailure;
    }

    return {
      paymentHash: payload.paymentHash,
      preimage: payload.paymentPreimage,
      feeMsats: satsToMsats(payload.routingFeeSat),
    };
  }

  async createOffer(params: CreateOfferParams): Promise<Offer> {
    const bolt12 = await this.requestText('/createoffer', {
      method: 'POST',
      form: {
        description: params.description,
        amountSat: params.amountMsats ? Math.floor(params.amountMsats / 1000) : undefined,
      },
    }, 'make_invoice');

    return {
      offerId: '',
      bolt12: bolt12.trim(),
      label: params.description,
      active: true,
      singleUse: false,
      used: false,
      amountMsats: params.amountMsats,
    };
  }

  async getOffer(): Promise<Offer> {
    const bolt12 = await this.requestText('/getoffer', { method: 'GET' }, 'lookup_invoice');
    return {
      offerId: '',
      bolt12: bolt12.trim(),
    };
  }

  async listOffers(): Promise<Offer[]> {
    return [];
  }

  async payOffer(offer: string, amountMsats: number, payerNote?: string): Promise<PayInvoiceResponse> {
    const payload = await this.requestJson<PhoenixdPayResponse>('/payoffer', {
      method: 'POST',
      form: {
        offer,
        amountSat: Math.floor(amountMsats / 1000),
        message: payerNote,
      },
    }, 'pay_invoice');

    const paymentFailure = phoenixdPaymentFailure(payload, 'pay_invoice');
    if (paymentFailure) {
      throw paymentFailure;
    }

    return {
      paymentHash: payload.paymentHash,
      preimage: payload.paymentPreimage,
      feeMsats: satsToMsats(payload.routingFeeSat),
    };
  }

  async lookupInvoice(params: LookupInvoiceParams): Promise<Transaction> {
    if (!params.paymentHash) {
      if (!params.search) {
        throw new LniError('InvalidInput', 'lookupInvoice requires paymentHash or search for PhoenixdNode.');
      }

      const txs = await this.listTransactions({ from: 0, limit: 100, search: params.search });
      const tx = txs[0];
      if (!tx) {
        throw phoenixdNwcError('NOT_FOUND', 'No matching transactions', 'lookup_invoice');
      }
      return tx;
    }

    const invoice = await this.requestJson<PhoenixdInvoiceResponse>(`/payments/incoming/${params.paymentHash}`, {
      method: 'GET',
    }, 'lookup_invoice');

    const settledAt = invoice.completedAt && invoice.isPaid ? toUnixSeconds(invoice.completedAt) : 0;

    return emptyTransaction({
      type: 'incoming',
      invoice: invoice.invoice ?? '',
      preimage: invoice.preimage,
      paymentHash: invoice.paymentHash,
      amountMsats: satsToMsats(invoice.receivedSat),
      feesPaid: satsToMsats(invoice.fees),
      createdAt: toUnixSeconds(invoice.createdAt),
      expiresAt: 0,
      settledAt,
      description: invoice.description ?? '',
      descriptionHash: '',
      payerNote: invoice.payerNote ?? '',
      externalId: invoice.externalId ?? '',
    });
  }

  async listTransactions(params: ListTransactionsParams): Promise<Transaction[]> {
    const query = {
      from: params.from ? params.from * 1000 : undefined,
      limit: params.limit || undefined,
      all: false,
    };

    const incoming = await this.requestJson<PhoenixdInvoiceResponse[]>('/payments/incoming', {
      method: 'GET',
      query,
    }, 'list_transactions');

    const outgoing = await this.requestJson<PhoenixdOutgoingPaymentResponse[]>('/payments/outgoing', {
      method: 'GET',
      query,
    }, 'list_transactions');

    const txs: Transaction[] = [];

    for (const item of incoming) {
      const tx = emptyTransaction({
        type: 'incoming',
        preimage: item.preimage,
        paymentHash: item.paymentHash,
        amountMsats: satsToMsats(item.receivedSat),
        feesPaid: satsToMsats(item.fees),
        createdAt: toUnixSeconds(item.createdAt),
        settledAt: item.isPaid && item.completedAt ? toUnixSeconds(item.completedAt) : 0,
        payerNote: item.payerNote ?? '',
        externalId: item.externalId ?? '',
      });

      if (params.paymentHash && tx.paymentHash !== params.paymentHash) {
        continue;
      }
      if (!matchesSearch(tx, params.search)) {
        continue;
      }
      txs.push(tx);
    }

    for (const item of outgoing) {
      const tx = emptyTransaction({
        type: 'outgoing',
        preimage: item.preimage ?? '',
        paymentHash: item.paymentHash ?? '',
        amountMsats: satsToMsats(item.sent),
        feesPaid: satsToMsats(item.fees),
        createdAt: toUnixSeconds(item.createdAt),
        settledAt: item.completedAt ? toUnixSeconds(item.completedAt) : 0,
        payerNote: item.payerNote ?? '',
        externalId: item.externalId ?? item.paymentId ?? '',
      });

      if (params.paymentHash && tx.paymentHash !== params.paymentHash) {
        continue;
      }
      if (!matchesSearch(tx, params.search)) {
        continue;
      }
      txs.push(tx);
    }

    txs.sort((a, b) => b.createdAt - a.createdAt);
    return txs;
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
      lookup: () => {
        if (params.paymentHash || params.search) {
          return this.lookupInvoice({ paymentHash: params.paymentHash, search: params.search });
        }

        return this.listTransactions({ from: 0, limit: 100 }).then((txs) => {
          const tx = txs[0];
          if (!tx) {
            throw phoenixdNwcError('NOT_FOUND', 'No matching transactions', 'lookup_invoice');
          }
          return tx;
        });
      },
    });
  }
}
