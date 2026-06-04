import { LniError } from '../errors.js';
import { decodeBolt11ToJson, decodeOfferToJson } from '../decode.js';
import { buildUrl, requestJson, requestText, resolveFetch, toTimeoutMs } from '../internal/http.js';
import { pollInvoiceEvents } from '../internal/polling.js';
import { emptyNodeInfo, emptyTransaction, matchesSearch, satsToMsats, toUnixSeconds } from '../internal/transform.js';
import { encodeBase64 } from '../internal/encoding.js';
import { DEFAULT_ONCHAIN_FEE_GUARDRAIL, InvoiceType, type CreateInvoiceParams, type CreateOfferParams, type InvoiceEventCallback, type LightningNode, type ListTransactionsParams, type LookupInvoiceParams, type NodeRequestOptions, type Offer, type OnInvoiceEventParams, type OnchainFeeGuardrail, type OnchainFeePayer, type OnchainFeePreference, type OnchainPayments, type OnchainTransaction, type PayInvoiceParams, type PayInvoiceResponse, type PayOnchainOptions, type PayOnchainResponse, type Permissions, type PhoenixdConfig, type PrepareOnchainTransactionParams, type Transaction, type NodeInfo } from '../types.js';

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

function assertValidOnchainAmount(amountSats: number): void {
  if (!Number.isSafeInteger(amountSats) || amountSats <= 0) {
    throw new LniError('InvalidInput', 'payOnchain requires a positive integer amountSats.');
  }
}

function resolvePhoenixdFeePayer(feePayer?: OnchainFeePayer): OnchainFeePayer {
  if (feePayer === 'recipient') {
    throw new LniError('InvalidInput', 'Phoenixd payOnchain only supports sender-paid on-chain fees.');
  }

  return 'sender';
}

function resolvePhoenixdFeeRequest(fee?: OnchainFeePreference): { fee: OnchainFeePreference; feerateSatByte: number } {
  const resolvedFee = fee ?? { type: 'default' };

  switch (resolvedFee.type) {
    case 'satsPerVbyte':
      if (!Number.isFinite(resolvedFee.satsPerVbyte) || resolvedFee.satsPerVbyte <= 0) {
        throw new LniError('InvalidInput', 'Phoenixd satsPerVbyte fee preference requires a positive fee rate.');
      }
      return { fee: resolvedFee, feerateSatByte: Math.ceil(resolvedFee.satsPerVbyte) };
    case 'backend': {
      const feerateSatByte = Number(resolvedFee.value);
      if (!Number.isSafeInteger(feerateSatByte) || feerateSatByte <= 0) {
        throw new LniError('InvalidInput', 'Phoenixd backend fee preference must be a positive integer feerateSatByte value.');
      }
      return { fee: resolvedFee, feerateSatByte };
    }
    case 'default':
    case 'speed':
    case 'targetConf':
      throw new LniError('InvalidInput', 'Phoenixd payOnchain requires an explicit satsPerVbyte fee preference.');
  }
}

function assertOnchainFeeGuardrail(transaction: OnchainTransaction, options?: PayOnchainOptions): void {
  if (options?.dangerouslyDisableFeeGuardrail) {
    return;
  }

  const guardrail: Required<OnchainFeeGuardrail> = {
    maxFeeSats: options?.feeGuardrail?.maxFeeSats ?? DEFAULT_ONCHAIN_FEE_GUARDRAIL.maxFeeSats,
    maxFeePercent: options?.feeGuardrail?.maxFeePercent ?? DEFAULT_ONCHAIN_FEE_GUARDRAIL.maxFeePercent,
  };
  if (transaction.feeSats === undefined) {
    throw new LniError('InvalidInput', 'Cannot pay on-chain transaction because feeSats is unknown. Re-prepare the transaction or pass dangerouslyDisableFeeGuardrail: true.');
  }
  if (!Number.isFinite(transaction.feeSats) || transaction.feeSats < 0) {
    throw new LniError('InvalidInput', 'Cannot pay on-chain transaction because feeSats is invalid.');
  }
  if (!Number.isFinite(transaction.amountSats) || transaction.amountSats <= 0) {
    throw new LniError('InvalidInput', 'Cannot pay on-chain transaction because amountSats is invalid.');
  }

  const maxFeeByPercent = Math.floor((transaction.amountSats * guardrail.maxFeePercent) / 100);
  const maxAllowedFee = Math.min(guardrail.maxFeeSats, maxFeeByPercent);
  if (transaction.feeSats > maxAllowedFee) {
    throw new LniError('InvalidInput', `Cannot pay on-chain transaction because feeSats ${transaction.feeSats} exceeds guardrail ${maxAllowedFee} sats.`);
  }
}

function txidFromText(value: string): string | undefined {
  const trimmed = value.trim();
  return /^[0-9a-fA-F]{64}$/.test(trimmed) ? trimmed : undefined;
}

export class PhoenixdNode implements LightningNode, OnchainPayments {
  private readonly fetchFn;
  private readonly timeoutMs?: number;

  constructor(private readonly config: PhoenixdConfig, options: NodeRequestOptions = {}) {
    this.fetchFn = resolveFetch(options.fetch);
    this.timeoutMs = toTimeoutMs(config.httpTimeout);
  }

  private authHeader(): string {
    return `Basic ${encodeBase64(`:${this.config.password}`)}`;
  }

  private async requestJson<T>(path: string, args: Parameters<typeof requestJson<T>>[2]): Promise<T> {
    return requestJson<T>(this.fetchFn, buildUrl(this.config.url, path), {
      ...args,
      timeoutMs: args?.timeoutMs ?? this.timeoutMs,
      headers: {
        authorization: this.authHeader(),
        ...(args?.headers ?? {}),
      },
    });
  }

  private async requestText(path: string, args: Parameters<typeof requestText>[2]): Promise<string> {
    return requestText(this.fetchFn, buildUrl(this.config.url, path), {
      ...args,
      timeoutMs: args?.timeoutMs ?? this.timeoutMs,
      headers: {
        authorization: this.authHeader(),
        ...(args?.headers ?? {}),
      },
    });
  }

  async getPermissions(): Promise<Permissions> {
    throw new LniError(
      'InvalidInput',
      'Phoenixd passwords cannot be introspected. Manually test permissions against Phoenixd REST endpoints.',
    );
  }

  async getInfo(): Promise<NodeInfo> {
    const [info, balance] = await Promise.all([
      this.requestJson<PhoenixdInfoResponse>('/getinfo', { method: 'GET' }),
      this.requestJson<PhoenixdBalanceResponse>('/getbalance', { method: 'GET' }),
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
      });

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
    });

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
    });

    return {
      paymentHash: payload.paymentHash,
      preimage: payload.paymentPreimage,
      feeMsats: satsToMsats(payload.routingFeeSat),
    };
  }

  async prepareOnchainTransaction(params: PrepareOnchainTransactionParams): Promise<OnchainTransaction> {
    assertValidOnchainAmount(params.amountSats);
    const feePayer = resolvePhoenixdFeePayer(params.feePayer);
    const { fee, feerateSatByte } = resolvePhoenixdFeeRequest(params.fee);

    return {
      address: params.address,
      amountSats: params.amountSats,
      recipientAmountSats: params.amountSats,
      feePayer,
      fee,
      raw: {
        sendRequest: {
          address: params.address,
          amountSat: params.amountSats,
          feerateSatByte,
        },
        description: params.description,
      },
    };
  }

  async payOnchain(transaction: OnchainTransaction, options?: PayOnchainOptions): Promise<PayOnchainResponse> {
    assertValidOnchainAmount(transaction.amountSats);
    resolvePhoenixdFeePayer(transaction.feePayer);
    const { feerateSatByte } = resolvePhoenixdFeeRequest(transaction.fee);
    assertOnchainFeeGuardrail(transaction, options);

    const response = await this.requestText('/sendtoaddress', {
      method: 'POST',
      form: {
        address: transaction.address,
        amountSat: transaction.amountSats,
        feerateSatByte,
      },
    });
    const txid = txidFromText(response);
    if (!txid) {
      throw new LniError('Api', response.trim() || 'Phoenixd sendtoaddress did not return a txid.');
    }

    return {
      txid,
      state: 'pending',
      address: transaction.address,
      amountSats: transaction.amountSats,
      feeSats: transaction.feeSats,
      totalAmountSats: transaction.totalAmountSats,
      recipientAmountSats: transaction.recipientAmountSats ?? transaction.amountSats,
      raw: response,
    };
  }

  async createOffer(params: CreateOfferParams): Promise<Offer> {
    const bolt12 = await this.requestText('/createoffer', {
      method: 'POST',
      form: {
        description: params.description,
        amountSat: params.amountMsats ? Math.floor(params.amountMsats / 1000) : undefined,
      },
    });

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
    const bolt12 = await this.requestText('/getoffer', { method: 'GET' });
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
    });

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
        throw new LniError('Api', 'No matching transactions');
      }
      return tx;
    }

    const invoice = await this.requestJson<PhoenixdInvoiceResponse>(`/payments/incoming/${params.paymentHash}`, {
      method: 'GET',
    });

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

    const incoming = await requestJson<PhoenixdInvoiceResponse[]>(
      this.fetchFn,
      buildUrl(this.config.url, '/payments/incoming', query),
      {
        method: 'GET',
        timeoutMs: this.timeoutMs,
        headers: { authorization: this.authHeader() },
      },
    );

    const outgoing = await requestJson<PhoenixdOutgoingPaymentResponse[]>(
      this.fetchFn,
      buildUrl(this.config.url, '/payments/outgoing', query),
      {
        method: 'GET',
        timeoutMs: this.timeoutMs,
        headers: { authorization: this.authHeader() },
      },
    );

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
            throw new LniError('Api', 'No matching transactions');
          }
          return tx;
        });
      },
    });
  }
}
