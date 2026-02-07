import { LniError } from '../errors.js';
import { buildUrl, requestJson, requestText, resolveFetch, toTimeoutMs } from '../internal/http.js';
import { pollInvoiceEvents } from '../internal/polling.js';
import { emptyNodeInfo, emptyTransaction, parseOptionalNumber } from '../internal/transform.js';
import { InvoiceType, type ClnConfig, type CreateInvoiceParams, type CreateOfferParams, type InvoiceEventCallback, type LightningNode, type ListTransactionsParams, type LookupInvoiceParams, type NodeInfo, type NodeRequestOptions, type Offer, type OnInvoiceEventParams, type PayInvoiceParams, type PayInvoiceResponse, type Transaction } from '../types.js';

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

  private async postJson<T>(path: string, json: unknown = {}): Promise<T> {
    return requestJson<T>(this.fetchFn, buildUrl(this.config.url, path), {
      method: 'POST',
      headers: this.headers(),
      json,
      timeoutMs: this.timeoutMs,
    });
  }

  private async postText(path: string, json: unknown = {}): Promise<string> {
    return requestText(this.fetchFn, buildUrl(this.config.url, path), {
      method: 'POST',
      headers: this.headers(),
      json,
      timeoutMs: this.timeoutMs,
    });
  }

  private async fetchInvoiceFromOffer(offer: string, amountMsats: number, payerNote?: string): Promise<string> {
    const payload = await this.postJson<ClnFetchInvoiceResponse>('/v1/fetchinvoice', {
      offer,
      amount_msat: amountMsats,
      payer_note: payerNote,
      timeout: 60,
    });

    if (!payload.invoice) {
      throw new LniError('Api', 'Missing BOLT12 invoice');
    }

    return payload.invoice;
  }

  private invoiceToTransaction(invoice: ClnInvoice): Transaction {
    return emptyTransaction({
      type: 'incoming',
      invoice: invoice.bolt11 ?? invoice.bolt12 ?? '',
      preimage: invoice.payment_preimage ?? '',
      paymentHash: invoice.payment_hash,
      amountMsats: invoice.amount_received_msat ?? invoice.amount_msat ?? 0,
      feesPaid: 0,
      createdAt: 0,
      expiresAt: invoice.expires_at ?? 0,
      settledAt: invoice.paid_at ?? 0,
      description: invoice.description ?? '',
      descriptionHash: '',
      payerNote: invoice.invreq_payer_note ?? '',
      externalId: invoice.label,
    });
  }

  async getInfo(): Promise<NodeInfo> {
    const [info, funds] = await Promise.all([
      this.postJson<ClnInfoResponse>('/v1/getinfo', {}),
      this.postJson<ClnListFundsResponse>('/v1/listfunds', {}),
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

    if (invoiceType === InvoiceType.Bolt12) {
      if (!params.offer) {
        throw new LniError('InvalidInput', 'Offer is required for BOLT12 invoice creation with CLN.');
      }

      const invoice = await this.fetchInvoiceFromOffer(
        params.offer,
        params.amountMsats ?? 0,
        params.description,
      );

      return emptyTransaction({
        type: 'incoming',
        invoice,
        amountMsats: params.amountMsats ?? 0,
        expiresAt: params.expiry ?? 0,
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
      label: `lni.${Math.floor(Math.random() * 1_000_000_000)}`,
    });

    return emptyTransaction({
      type: 'incoming',
      invoice: payload.bolt11,
      paymentHash: payload.payment_hash,
      amountMsats: params.amountMsats ?? 0,
      expiresAt: params.expiry ?? 3600,
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

    const payload = await this.postJson<ClnPayResponse>('/v1/pay', body);

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
    });

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
    return (
      offers[0] ?? {
        offerId: '',
        bolt12: '',
      }
    );
  }

  async listOffers(search?: string): Promise<Offer[]> {
    const payload = await this.postJson<ClnListOffersResponse>('/v1/listoffers', {
      ...(search ? { offer_id: search } : {}),
    });

    return payload.offers;
  }

  async payOffer(offer: string, amountMsats: number, payerNote?: string): Promise<PayInvoiceResponse> {
    const bolt11 = await this.fetchInvoiceFromOffer(offer, amountMsats, payerNote);
    const payload = await this.postJson<ClnPayResponse>('/v1/pay', {
      bolt11,
      maxfeepercent: 1,
      retry_for: 60,
    });

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

    const payload = await this.postJson<ClnInvoicesResponse>('/v1/listinvoices', query);

    const invoice = payload.invoices[0];
    if (!invoice) {
      throw new LniError('Api', 'No matching invoice found');
    }

    return this.invoiceToTransaction(invoice);
  }

  async listTransactions(params: ListTransactionsParams): Promise<Transaction[]> {
    const payload = await this.postJson<ClnInvoicesResponse>('/v1/listinvoices', {
      start: params.from,
      index: 'created',
      limit: params.limit || undefined,
      payment_hash: params.paymentHash,
    });

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
    return this.postText('/v1/decode', { string: str });
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
