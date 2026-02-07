import { LniError } from '../errors.js';
import { buildUrl, requestJson, requestText, resolveFetch, toTimeoutMs } from '../internal/http.js';
import { pollInvoiceEvents } from '../internal/polling.js';
import { btcToMsats, emptyNodeInfo, emptyTransaction, matchesSearch, msatsToBtc, parseOptionalNumber, toUnixSeconds } from '../internal/transform.js';
import { InvoiceType, type CreateInvoiceParams, type CreateOfferParams, type InvoiceEventCallback, type LightningNode, type ListTransactionsParams, type LookupInvoiceParams, type NodeInfo, type NodeRequestOptions, type Offer, type OnInvoiceEventParams, type PayInvoiceParams, type PayInvoiceResponse, type StrikeConfig, type Transaction } from '../types.js';

interface StrikeBalance {
  currency: string;
  current: string;
}

interface StrikeAmount {
  amount: string;
  currency: string;
}

interface StrikeCreateReceiveResponse {
  receiveRequestId: string;
  created: string;
  bolt11?: {
    invoice: string;
    paymentHash: string;
    description?: string;
    descriptionHash?: string;
    expires: string;
  };
}

interface StrikePaymentQuoteResponse {
  paymentQuoteId: string;
}

interface StrikePaymentExecutionResponse {
  paymentId: string;
}

interface StrikePaymentResponse {
  id: string;
  state: string;
  created: string;
  completed?: string;
  description?: string;
  amount: StrikeAmount;
  lightning?: {
    paymentHash?: string;
    paymentRequest?: string;
    networkFee?: StrikeAmount;
  };
}

interface StrikeReceivesResponse {
  items: Array<{
    receiveRequestId: string;
    state: string;
    created: string;
    completed?: string;
    amountReceived: StrikeAmount;
    lightning?: {
      invoice: string;
      preimage: string;
      description?: string;
      descriptionHash?: string;
      paymentHash: string;
    };
  }>;
}

interface StrikePaymentsResponse {
  data: StrikePaymentResponse[];
}

export class StrikeNode implements LightningNode {
  private readonly fetchFn;
  private readonly timeoutMs?: number;
  private readonly baseUrl: string;

  constructor(private readonly config: StrikeConfig, options: NodeRequestOptions = {}) {
    this.fetchFn = resolveFetch(options.fetch);
    this.timeoutMs = toTimeoutMs(config.httpTimeout);
    this.baseUrl = config.baseUrl ?? 'https://api.strike.me/v1';
  }

  private headers(extra?: HeadersInit): HeadersInit {
    return {
      authorization: `Bearer ${this.config.apiKey}`,
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

  private async patchJson<T>(path: string): Promise<T> {
    return requestJson<T>(this.fetchFn, buildUrl(this.baseUrl, path), {
      method: 'PATCH',
      headers: this.headers(),
      timeoutMs: this.timeoutMs,
    });
  }

  private isNotFoundError(error: unknown): boolean {
    return error instanceof LniError && error.code === 'Http' && error.status === 404;
  }

  async getInfo(): Promise<NodeInfo> {
    const balances = await this.getJson<StrikeBalance[]>('/balances');

    const btcBalance = balances.find((balance) => balance.currency === 'BTC');

    return emptyNodeInfo({
      alias: 'Strike Node',
      network: 'mainnet',
      sendBalanceMsat: btcBalance ? btcToMsats(btcBalance.current) : 0,
    });
  }

  async createInvoice(params: CreateInvoiceParams): Promise<Transaction> {
    if ((params.invoiceType ?? InvoiceType.Bolt11) !== InvoiceType.Bolt11) {
      throw new LniError('Api', 'Bolt12 is not implemented for StrikeNode.');
    }

    const response = await this.postJson<StrikeCreateReceiveResponse>('/receive-requests', {
      bolt11: {
        amount:
          params.amountMsats !== undefined
            ? {
                amount: msatsToBtc(params.amountMsats),
                currency: 'BTC',
              }
            : undefined,
        description: params.description,
        descriptionHash: params.descriptionHash,
        expiryInSeconds: params.expiry,
      },
      onchain: null,
      targetCurrency: 'BTC',
    });

    const bolt11 = response.bolt11;
    if (!bolt11) {
      throw new LniError('Json', 'No bolt11 payload returned from Strike create invoice call.');
    }

    return emptyTransaction({
      type: 'incoming',
      invoice: bolt11.invoice,
      paymentHash: bolt11.paymentHash,
      amountMsats: params.amountMsats ?? 0,
      createdAt: toUnixSeconds(Date.parse(response.created)),
      expiresAt: toUnixSeconds(Date.parse(bolt11.expires)),
      description: bolt11.description ?? params.description ?? '',
      descriptionHash: bolt11.descriptionHash ?? params.descriptionHash ?? '',
      externalId: response.receiveRequestId,
      payerNote: '',
    });
  }

  async payInvoice(params: PayInvoiceParams): Promise<PayInvoiceResponse> {
    const quote = await this.postJson<StrikePaymentQuoteResponse>('/payment-quotes/lightning', {
      lnInvoice: params.invoice,
      sourceCurrency: 'BTC',
      amount:
        params.amountMsats !== undefined
          ? {
              amount: msatsToBtc(params.amountMsats),
              currency: 'BTC',
            }
          : undefined,
    });

    const execution = await this.patchJson<StrikePaymentExecutionResponse>(`/payment-quotes/${quote.paymentQuoteId}/execute`);
    const payment = await this.getJson<StrikePaymentResponse>(`/payments/${execution.paymentId}`);

    const feeMsats = payment.lightning?.networkFee ? btcToMsats(payment.lightning.networkFee.amount) : 0;

    return {
      paymentHash: payment.lightning?.paymentHash ?? '',
      preimage: '',
      feeMsats,
    };
  }

  async createOffer(_params: CreateOfferParams): Promise<Offer> {
    throw new LniError('Api', 'Bolt12 is not implemented for StrikeNode.');
  }

  async getOffer(_search?: string): Promise<Offer> {
    throw new LniError('Api', 'Bolt12 is not implemented for StrikeNode.');
  }

  async listOffers(_search?: string): Promise<Offer[]> {
    throw new LniError('Api', 'Bolt12 is not implemented for StrikeNode.');
  }

  async payOffer(_offer: string, _amountMsats: number, _payerNote?: string): Promise<PayInvoiceResponse> {
    throw new LniError('Api', 'Bolt12 is not implemented for StrikeNode.');
  }

  async lookupInvoice(params: LookupInvoiceParams): Promise<Transaction> {
    if (!params.paymentHash) {
      throw new LniError('InvalidInput', 'lookupInvoice requires paymentHash for StrikeNode.');
    }

    const receives = await this.getJson<StrikeReceivesResponse>('/receive-requests/receives', {
      '$paymentHash': params.paymentHash,
    });

    const item = receives.items[0];
    if (!item?.lightning) {
      throw new LniError('Api', `No receive found for payment hash: ${params.paymentHash}`);
    }

    return emptyTransaction({
      type: 'incoming',
      invoice: item.lightning.invoice,
      preimage: item.lightning.preimage,
      paymentHash: item.lightning.paymentHash,
      amountMsats: btcToMsats(item.amountReceived.amount),
      feesPaid: 0,
      createdAt: toUnixSeconds(Date.parse(item.created)),
      settledAt: item.state === 'COMPLETED' ? toUnixSeconds(Date.parse(item.completed ?? '')) : 0,
      description: item.lightning.description ?? item.lightning.descriptionHash ?? '',
      descriptionHash: item.lightning.descriptionHash ?? '',
      externalId: item.receiveRequestId,
      payerNote: '',
    });
  }

  async listTransactions(params: ListTransactionsParams): Promise<Transaction[]> {
    const receives = await this.getJson<StrikeReceivesResponse>('/receive-requests/receives', {
      '$skip': params.from,
      '$top': params.limit,
    });

    let outgoing: StrikePaymentsResponse = { data: [] };
    try {
      outgoing = await this.getJson<StrikePaymentsResponse>('/payments', {
        skip: params.from,
        top: params.limit,
      });
    } catch (error) {
      if (!this.isNotFoundError(error)) {
        throw error;
      }
      // Strike can return 404 when there are no outgoing payments for the account.
    }

    const txs: Transaction[] = [];

    for (const receive of receives.items) {
      if (!receive.lightning) {
        continue;
      }

      const tx = emptyTransaction({
        type: 'incoming',
        invoice: receive.lightning.invoice,
        preimage: receive.lightning.preimage,
        paymentHash: receive.lightning.paymentHash,
        amountMsats: btcToMsats(receive.amountReceived.amount),
        feesPaid: 0,
        createdAt: toUnixSeconds(Date.parse(receive.created)),
        settledAt: receive.state === 'COMPLETED' ? toUnixSeconds(Date.parse(receive.completed ?? '')) : 0,
        description: receive.lightning.description ?? receive.lightning.descriptionHash ?? '',
        descriptionHash: receive.lightning.descriptionHash ?? '',
        externalId: receive.receiveRequestId,
        payerNote: '',
      });

      txs.push(tx);
    }

    for (const payment of outgoing.data) {
      const tx = emptyTransaction({
        type: 'outgoing',
        invoice: payment.lightning?.paymentRequest ?? '',
        paymentHash: payment.lightning?.paymentHash ?? '',
        amountMsats: btcToMsats(payment.amount.amount),
        feesPaid: payment.lightning?.networkFee ? btcToMsats(payment.lightning.networkFee.amount) : 0,
        createdAt: toUnixSeconds(Date.parse(payment.created)),
        settledAt: payment.state === 'COMPLETED' ? toUnixSeconds(Date.parse(payment.completed ?? '')) : 0,
        description: payment.description ?? '',
        descriptionHash: '',
        externalId: payment.id,
        payerNote: '',
      });

      txs.push(tx);
    }

    const filtered = txs.filter((tx) => {
      if (params.paymentHash && tx.paymentHash !== params.paymentHash) {
        return false;
      }
      return matchesSearch(tx, params.search);
    });

    return filtered.sort((a, b) => b.createdAt - a.createdAt);
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
