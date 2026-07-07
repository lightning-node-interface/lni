import { decodeBolt11ToJson, decodeOfferToJson } from '../decode.js';
import { LniError, NwcError, type NwcErrorCode, type NwcErrorOperation } from '../errors.js';
import { providerInfoFromJsonErrorBody, throwNormalizedProviderError, type ProviderErrorInfo } from '../internal/error-normalization.js';
import { buildUrl, requestJson, requestText, resolveFetch, toTimeoutMs } from '../internal/http.js';
import { parseClnRunePermissions } from '../internal/permissions.js';
import { pollInvoiceEvents } from '../internal/polling.js';
import { emptyNodeInfo, emptyTransaction, parseOptionalNumber } from '../internal/transform.js';
import { InvoiceType, type ClnConfig, type CreateInvoiceParams, type CreateOfferParams, type InvoiceEventCallback, type LightningNode, type ListTransactionsParams, type LookupInvoiceParams, type NodeInfo, type NodeRequestOptions, type Offer, type OnInvoiceEventParams, type PayInvoiceParams, type PayInvoiceResponse, type Permissions, type Transaction } from '../types.js';

interface ClnInfoResponse {
  id: string;
  alias: string;
  color: string;
  network: string;
  blockheight: number;
}

interface ClnListFundsResponse {
  channels: Array<{
    connected: boolean;
    state: string;
    our_amount_msat: number;
    amount_msat: number;
  }>;
}

interface ClnBolt11Response {
  payment_hash: string;
  bolt11: string;
}

interface ClnPayResponse {
  payment_hash: string;
  payment_preimage: string;
  amount_msat: number;
  amount_sent_msat: number;
}

interface ClnFetchInvoiceResponse {
  invoice: string;
}

interface ClnInvoice {
  label: string;
  bolt11?: string;
  bolt12?: string;
  payment_hash: string;
  amount_received_msat?: number;
  payment_preimage?: string;
  description?: string;
  expires_at: number;
  expiry?: number;
  expiry_seconds?: number;
  paid_at?: number;
  amount_msat?: number;
  invreq_payer_note?: string;
}

interface ClnInvoicesResponse {
  invoices: ClnInvoice[];
}

interface ClnOfferResponse {
  offer_id?: string;
  bolt12: string;
  active: boolean;
  single_use: boolean;
  used: boolean;
}

interface ClnListOffersResponse {
  offers: Offer[];
}

function newInvoiceLabel(): string {
  if (globalThis.crypto?.randomUUID) {
    return `lni.${globalThis.crypto.randomUUID()}`;
  }

  return `lni.${Date.now()}.${Math.floor(Math.random() * 1_000_000)}`;
}

function mapClnProviderError(info: ProviderErrorInfo): NwcErrorCode | undefined {
  const numericCode =
    typeof info.code === 'number'
      ? info.code
      : typeof info.code === 'string'
        ? Number.parseInt(info.code, 10)
        : undefined;

  switch (numericCode) {
    case 203:
    case 205:
    case 206:
    case 207:
    case 210:
      return 'PAYMENT_FAILED';
    case 201:
    case -1:
    case 900:
    case 901:
    case 902:
    case -32602:
      return 'OTHER';
    default:
      return undefined;
  }
}

function throwClnError(error: unknown, operation: NwcErrorOperation): never {
  throwNormalizedProviderError(error, {
    provider: 'cln',
    operation,
    extractProviderError: providerInfoFromJsonErrorBody,
    mapProviderError: mapClnProviderError,
  });
}

function clnNotFound(message: string, operation: NwcErrorOperation): NwcError {
  return new NwcError('NOT_FOUND', message, {
    operation,
    provider: 'cln',
  });
}

function clnInternal(message: string, operation: NwcErrorOperation): NwcError {
  return new NwcError('INTERNAL', message, {
    operation,
    provider: 'cln',
  });
}

export class ClnNode implements LightningNode {
  private readonly fetchFn;
  private readonly timeoutMs?: number;

  constructor(private readonly config: ClnConfig, options: NodeRequestOptions = {}) {
    this.fetchFn = resolveFetch(options.fetch);
    this.timeoutMs = toTimeoutMs(config.httpTimeout);
  }

  private headers(extra?: HeadersInit): HeadersInit {
    return {
      rune: this.config.rune,
      'content-type': 'application/json',
      ...(extra ?? {}),
    };
  }

  private async postJson<T>(path: string, json: unknown = {}, operation?: NwcErrorOperation): Promise<T> {
    try {
      return await requestJson<T>(this.fetchFn, buildUrl(this.config.url, path), {
        method: 'POST',
        headers: this.headers(),
        json,
        timeoutMs: this.timeoutMs,
      });
    } catch (error) {
      if (operation) {
        throwClnError(error, operation);
      }
      throw error;
    }
  }

  private async postText(path: string, json: unknown = {}, operation?: NwcErrorOperation): Promise<string> {
    try {
      return await requestText(this.fetchFn, buildUrl(this.config.url, path), {
        method: 'POST',
        headers: this.headers(),
        json,
        timeoutMs: this.timeoutMs,
      });
    } catch (error) {
      if (operation) {
        throwClnError(error, operation);
      }
      throw error;
    }
  }

  private async fetchInvoiceFromOffer(
    offer: string,
    amountMsats: number,
    payerNote: string | undefined,
    operation: NwcErrorOperation,
  ): Promise<string> {
    const payload = await this.postJson<ClnFetchInvoiceResponse>('/v1/fetchinvoice', {
      offer,
      amount_msat: amountMsats,
      payer_note: payerNote,
      timeout: 60,
    }, operation);

    if (!payload.invoice) {
      throw clnInternal('Missing BOLT12 invoice', operation);
    }

    return payload.invoice;
  }

  async getPermissions(): Promise<Permissions> {
    return parseClnRunePermissions(this.config.rune);
  }

  private invoiceToTransaction(invoice: ClnInvoice): Transaction {
    const expiresAt = parseOptionalNumber(invoice.expires_at);
    const expirySeconds = parseOptionalNumber(invoice.expiry_seconds ?? invoice.expiry);

    let createdAt = 0;
    if (expiresAt > 0 && expirySeconds > 0) {
      createdAt = Math.max(expiresAt - expirySeconds, 0);
    }
    if (createdAt <= 0) {
      createdAt = parseOptionalNumber(invoice.paid_at);
    }
    if (createdAt <= 0) {
      createdAt = Math.floor(Date.now() / 1000);
    }

    return emptyTransaction({
      type: 'incoming',
      invoice: invoice.bolt11 ?? invoice.bolt12 ?? '',
      preimage: invoice.payment_preimage ?? '',
      paymentHash: invoice.payment_hash,
      amountMsats: invoice.amount_received_msat ?? invoice.amount_msat ?? 0,
      feesPaid: 0,
      createdAt,
      expiresAt,
      settledAt: parseOptionalNumber(invoice.paid_at),
      description: invoice.description ?? '',
      descriptionHash: '',
      payerNote: invoice.invreq_payer_note ?? '',
      externalId: invoice.label,
    });
  }

  async getInfo(): Promise<NodeInfo> {
    const [info, funds] = await Promise.all([
      this.postJson<ClnInfoResponse>('/v1/getinfo', {}, 'get_info'),
      this.postJson<ClnListFundsResponse>('/v1/listfunds', {}, 'get_info'),
    ]);

    let sendBalanceMsat = 0;
    let receiveBalanceMsat = 0;
    let unsettledSendBalanceMsat = 0;
    let unsettledReceiveBalanceMsat = 0;
    let pendingOpenSendBalance = 0;
    let pendingOpenReceiveBalance = 0;

    for (const channel of funds.channels) {
      const channelAmount = parseOptionalNumber(channel.amount_msat);
      const localAmount = parseOptionalNumber(channel.our_amount_msat);
      const remoteAmount = channelAmount - localAmount;

      if (channel.state === 'CHANNELD_NORMAL' && channel.connected) {
        sendBalanceMsat += localAmount;
        receiveBalanceMsat += remoteAmount;
        continue;
      }

      if (channel.state === 'CHANNELD_NORMAL' && !channel.connected) {
        unsettledSendBalanceMsat += localAmount;
        unsettledReceiveBalanceMsat += remoteAmount;
        continue;
      }

      if (
        channel.state === 'CHANNELD_AWAITING_LOCKIN' ||
        channel.state === 'DUALOPEND_AWAITING_LOCKIN' ||
        channel.state === 'DUALOPEND_OPEN_INIT' ||
        channel.state === 'DUALOPEND_OPEN_COMMITTED' ||
        channel.state === 'DUALOPEND_OPEN_COMMIT_READY' ||
        channel.state === 'OPENINGD'
      ) {
        pendingOpenSendBalance += localAmount;
        pendingOpenReceiveBalance += remoteAmount;
      }
    }

    return emptyNodeInfo({
      alias: info.alias,
      color: info.color,
      pubkey: info.id,
      network: info.network,
      blockHeight: info.blockheight,
      sendBalanceMsat,
      receiveBalanceMsat,
      unsettledSendBalanceMsat,
      unsettledReceiveBalanceMsat,
      pendingOpenSendBalance,
      pendingOpenReceiveBalance,
    });
  }

  async createInvoice(params: CreateInvoiceParams): Promise<Transaction> {
    const invoiceType = params.invoiceType ?? InvoiceType.Bolt11;
    const now = Math.floor(Date.now() / 1000);
    const expirySeconds = Math.max(params.expiry ?? 3600, 0);
    const expiresAt = now + expirySeconds;

    if (invoiceType === InvoiceType.Bolt12) {
      if (!params.offer) {
        throw new LniError('InvalidInput', 'Offer is required for BOLT12 invoice creation with CLN.');
      }

      const invoice = await this.fetchInvoiceFromOffer(
        params.offer,
        params.amountMsats ?? 0,
        params.description,
        'make_invoice',
      );

      return emptyTransaction({
        type: 'incoming',
        invoice,
        amountMsats: params.amountMsats ?? 0,
        expiresAt,
        description: params.description ?? '',
        descriptionHash: params.descriptionHash ?? '',
        payerNote: '',
        externalId: '',
      });
    }

    const payload = await this.postJson<ClnBolt11Response>('/v1/invoice', {
      description: params.description ?? '',
      amount_msat: params.amountMsats !== undefined ? String(params.amountMsats) : 'any',
      expiry: params.expiry,
      label: newInvoiceLabel(),
    }, 'make_invoice');

    return emptyTransaction({
      type: 'incoming',
      invoice: payload.bolt11,
      paymentHash: payload.payment_hash,
      amountMsats: params.amountMsats ?? 0,
      expiresAt,
      description: params.description ?? '',
      descriptionHash: params.descriptionHash ?? '',
      payerNote: '',
      externalId: '',
    });
  }

  async payInvoice(params: PayInvoiceParams): Promise<PayInvoiceResponse> {
    if (params.feeLimitMsat !== undefined && params.feeLimitPercentage !== undefined) {
      throw new LniError('InvalidInput', 'Cannot set both feeLimitMsat and feeLimitPercentage.');
    }

    const body: Record<string, unknown> = {
      bolt11: params.invoice,
    };

    if (params.amountMsats !== undefined) {
      body.amount_msat = String(params.amountMsats);
    }
    if (params.feeLimitMsat !== undefined) {
      body.maxfee = String(params.feeLimitMsat);
    }
    if (params.feeLimitPercentage !== undefined) {
      body.maxfeepercent = params.feeLimitPercentage;
    }
    if (params.timeoutSeconds !== undefined) {
      body.retry_for = String(params.timeoutSeconds);
    }

    const payload = await this.postJson<ClnPayResponse>('/v1/pay', body, 'pay_invoice');

    return {
      paymentHash: payload.payment_hash,
      preimage: payload.payment_preimage,
      feeMsats: parseOptionalNumber(payload.amount_sent_msat) - parseOptionalNumber(payload.amount_msat),
    };
  }

  async createOffer(params: CreateOfferParams): Promise<Offer> {
    const payload = await this.postJson<ClnOfferResponse>('/v1/offer', {
      amount: params.amountMsats !== undefined ? `${params.amountMsats}msat` : 'any',
      description: params.description,
    }, 'make_invoice');

    return {
      offerId: payload.offer_id ?? '',
      bolt12: payload.bolt12,
      label: params.description,
      active: payload.active,
      singleUse: payload.single_use,
      used: payload.used,
      amountMsats: params.amountMsats,
    };
  }

  async getOffer(search?: string): Promise<Offer> {
    const offers = await this.listOffers(search);
    if (!offers.length) {
      throw clnNotFound(search ? `Offer not found for search: ${search}` : 'Offer not found', 'lookup_invoice');
    }

    return offers[0]!;
  }

  async listOffers(search?: string): Promise<Offer[]> {
    const payload = await this.postJson<ClnListOffersResponse>('/v1/listoffers', {
      ...(search ? { offer_id: search } : {}),
    }, 'list_transactions');

    return payload.offers;
  }

  async payOffer(offer: string, amountMsats: number, payerNote?: string): Promise<PayInvoiceResponse> {
    const bolt11 = await this.fetchInvoiceFromOffer(offer, amountMsats, payerNote, 'pay_invoice');
    const payload = await this.postJson<ClnPayResponse>('/v1/pay', {
      bolt11,
      maxfeepercent: 1,
      retry_for: 60,
    }, 'pay_invoice');

    return {
      paymentHash: payload.payment_hash,
      preimage: payload.payment_preimage,
      feeMsats: parseOptionalNumber(payload.amount_sent_msat) - parseOptionalNumber(payload.amount_msat),
    };
  }

  async lookupInvoice(params: LookupInvoiceParams): Promise<Transaction> {
    const query: Record<string, unknown> = {};
    if (params.paymentHash) {
      query.payment_hash = params.paymentHash;
    } else if (params.search) {
      query.payment_hash = params.search;
    }

    const payload = await this.postJson<ClnInvoicesResponse>('/v1/listinvoices', query, 'lookup_invoice');

    const invoice = payload.invoices[0];
    if (!invoice) {
      throw clnNotFound('No matching invoice found', 'lookup_invoice');
    }

    return this.invoiceToTransaction(invoice);
  }

  async listTransactions(params: ListTransactionsParams): Promise<Transaction[]> {
    const payload = await this.postJson<ClnInvoicesResponse>('/v1/listinvoices', {
      start: params.from,
      index: 'created',
      limit: params.limit || undefined,
      payment_hash: params.paymentHash,
    }, 'list_transactions');

    const transactions = payload.invoices.map((invoice) => this.invoiceToTransaction(invoice));

    if (params.search) {
      return transactions.filter((tx) => {
        const normalized = params.search?.toLowerCase() ?? '';
        return (
          tx.paymentHash.toLowerCase().includes(normalized) ||
          tx.description.toLowerCase().includes(normalized) ||
          (tx.payerNote ?? '').toLowerCase().includes(normalized)
        );
      });
    }

    return transactions;
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
      lookup: () =>
        this.lookupInvoice({
          paymentHash: params.paymentHash,
          search: params.search,
      }),
    });
  }
}
