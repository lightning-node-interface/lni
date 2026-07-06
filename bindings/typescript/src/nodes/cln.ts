import { decodeBolt11ToJson, decodeOfferToJson } from '../decode.js';
import { LniError } from '../errors.js';
import { buildUrl, requestJson, requestText, resolveFetch, toTimeoutMs } from '../internal/http.js';
import { parseClnRunePermissions } from '../internal/permissions.js';
import { pollInvoiceEvents } from '../internal/polling.js';
import { emptyNodeInfo, emptyTransaction, parseOptionalNumber } from '../internal/transform.js';
import { DEFAULT_ONCHAIN_FEE_GUARDRAIL, InvoiceType, type ClnConfig, type CreateInvoiceParams, type CreateOfferParams, type InvoiceEventCallback, type LightningNode, type ListTransactionsParams, type LookupInvoiceParams, type NodeInfo, type NodeRequestOptions, type Offer, type OnInvoiceEventParams, type OnchainFeeGuardrail, type OnchainFeePayer, type OnchainFeePreference, type OnchainPayments, type OnchainTransaction, type PayInvoiceParams, type PayInvoiceResponse, type PayOnchainOptions, type PayOnchainResponse, type Permissions, type PrepareOnchainTransactionParams, type Transaction } from '../types.js';

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

interface ClnTxPrepareResponse {
  psbt?: string;
  unsigned_tx?: string;
  txid?: string;
}

interface ClnTxSendResponse {
  txid?: string;
  tx?: string;
}

interface ClnOnchainFeeRequest {
  feerate?: string;
}

function newInvoiceLabel(): string {
  if (globalThis.crypto?.randomUUID) {
    return `lni.${globalThis.crypto.randomUUID()}`;
  }

  return `lni.${Date.now()}.${Math.floor(Math.random() * 1_000_000)}`;
}

function assertValidOnchainAmount(amountSats: number): void {
  if (!Number.isSafeInteger(amountSats) || amountSats <= 0) {
    throw new LniError('InvalidInput', 'payOnchain requires a positive integer amountSats.');
  }
}

function defaultOnchainFee(): OnchainFeePreference {
  return { type: 'speed', speed: 'normal' };
}

function resolveClnFeePayer(feePayer?: OnchainFeePayer): OnchainFeePayer {
  if (feePayer === 'recipient') {
    throw new LniError('InvalidInput', 'CLN payOnchain only supports sender-paid on-chain fees.');
  }

  return 'sender';
}

function resolveClnFeeRequest(fee: OnchainFeePreference): ClnOnchainFeeRequest {
  switch (fee.type) {
    case 'default':
      return { feerate: 'normal' };
    case 'speed':
      switch (fee.speed) {
        case 'fast':
          return { feerate: 'urgent' };
        case 'normal':
          return { feerate: 'normal' };
        case 'slow':
          return { feerate: 'slow' };
        case 'free':
          throw new LniError('InvalidInput', 'CLN payOnchain does not support free on-chain fee speed.');
      }
    case 'satsPerVbyte':
      if (!Number.isFinite(fee.satsPerVbyte) || fee.satsPerVbyte <= 0) {
        throw new LniError('InvalidInput', 'CLN satsPerVbyte fee preference requires a positive fee rate.');
      }
      return { feerate: `${Math.ceil(fee.satsPerVbyte * 1000)}perkb` };
    case 'backend':
      if (!fee.value.trim()) {
        throw new LniError('InvalidInput', 'CLN backend fee preference requires a feerate value.');
      }
      return { feerate: fee.value };
    case 'targetConf':
      throw new LniError('InvalidInput', 'CLN payOnchain does not support target-confirmation fee preferences.');
  }
}

function normalizeOnchainState(txid?: string): PayOnchainResponse['state'] {
  return txid ? 'pending' : 'failed';
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

function readUIntLE(bytes: Uint8Array, offset: number, length: number): number {
  let value = 0;
  for (let i = 0; i < length; i += 1) {
    value += (bytes[offset + i] ?? 0) * 2 ** (8 * i);
  }
  return value;
}

function readUInt64LE(bytes: Uint8Array, offset: number): bigint {
  let value = 0n;
  for (let i = 0; i < 8; i += 1) {
    value += BigInt(bytes[offset + i] ?? 0) << BigInt(8 * i);
  }
  return value;
}

function readCompactSize(bytes: Uint8Array, offset: number): { value: number; next: number } {
  const first = bytes[offset];
  if (first === undefined) {
    throw new Error('Unexpected end of compact size.');
  }
  if (first < 0xfd) {
    return { value: first, next: offset + 1 };
  }
  if (first === 0xfd) {
    return { value: readUIntLE(bytes, offset + 1, 2), next: offset + 3 };
  }
  if (first === 0xfe) {
    return { value: readUIntLE(bytes, offset + 1, 4), next: offset + 5 };
  }
  const value = Number(readUInt64LE(bytes, offset + 1));
  if (!Number.isSafeInteger(value)) {
    throw new Error('Compact size is too large.');
  }
  return { value, next: offset + 9 };
}

function parseTransaction(bytes: Uint8Array): {
  inputCount: number;
  prevouts: Array<{ vout: number }>;
  outputTotalSats: number;
  outputs: Array<{ amountSats: number }>;
} {
  let offset = 4;
  let inputCountInfo = readCompactSize(bytes, offset);
  if (inputCountInfo.value === 0 && bytes[inputCountInfo.next] !== undefined) {
    offset = inputCountInfo.next + 1;
    inputCountInfo = readCompactSize(bytes, offset);
  }

  const inputCount = inputCountInfo.value;
  offset = inputCountInfo.next;
  const prevouts: Array<{ vout: number }> = [];
  for (let i = 0; i < inputCount; i += 1) {
    offset += 32;
    const vout = readUIntLE(bytes, offset, 4);
    offset += 4;
    const script = readCompactSize(bytes, offset);
    offset = script.next + script.value + 4;
    prevouts.push({ vout });
  }

  const outputCount = readCompactSize(bytes, offset);
  offset = outputCount.next;
  const outputs: Array<{ amountSats: number }> = [];
  let outputTotalSats = 0;
  for (let i = 0; i < outputCount.value; i += 1) {
    const amountSats = Number(readUInt64LE(bytes, offset));
    offset += 8;
    const script = readCompactSize(bytes, offset);
    offset = script.next + script.value;
    outputs.push({ amountSats });
    outputTotalSats += amountSats;
  }

  return { inputCount, prevouts, outputTotalSats, outputs };
}

function parsePsbtMap(bytes: Uint8Array, offset: number): {
  entries: Array<{ key: Uint8Array; value: Uint8Array }>;
  next: number;
} {
  const entries: Array<{ key: Uint8Array; value: Uint8Array }> = [];
  while (offset < bytes.length) {
    const keyLen = readCompactSize(bytes, offset);
    offset = keyLen.next;
    if (keyLen.value === 0) {
      return { entries, next: offset };
    }
    const key = bytes.slice(offset, offset + keyLen.value);
    offset += keyLen.value;
    const valueLen = readCompactSize(bytes, offset);
    offset = valueLen.next;
    const value = bytes.slice(offset, offset + valueLen.value);
    offset += valueLen.value;
    entries.push({ key, value });
  }

  throw new Error('Unterminated PSBT map.');
}

function base64ToBytes(value: string): Uint8Array {
  if (typeof globalThis.atob === 'function') {
    const decoded = globalThis.atob(value);
    return Uint8Array.from(decoded, (char) => char.charCodeAt(0));
  }

  return Uint8Array.from(Buffer.from(value, 'base64'));
}

function parsePsbtFeeSats(psbt?: string): number | undefined {
  if (!psbt) {
    return undefined;
  }

  try {
    const bytes = base64ToBytes(psbt);
    if (bytes.length < 5 || bytes[0] !== 0x70 || bytes[1] !== 0x73 || bytes[2] !== 0x62 || bytes[3] !== 0x74 || bytes[4] !== 0xff) {
      return undefined;
    }

    const globalMap = parsePsbtMap(bytes, 5);
    const unsignedTx = globalMap.entries.find((entry) => entry.key[0] === 0x00)?.value;
    if (!unsignedTx) {
      return undefined;
    }

    const tx = parseTransaction(unsignedTx);
    let offset = globalMap.next;
    let inputTotalSats = 0;

    for (let i = 0; i < tx.inputCount; i += 1) {
      const inputMap = parsePsbtMap(bytes, offset);
      offset = inputMap.next;
      const witnessUtxo = inputMap.entries.find((entry) => entry.key[0] === 0x01)?.value;
      if (witnessUtxo) {
        inputTotalSats += Number(readUInt64LE(witnessUtxo, 0));
        continue;
      }

      const nonWitnessUtxo = inputMap.entries.find((entry) => entry.key[0] === 0x00)?.value;
      const prevout = tx.prevouts[i];
      if (nonWitnessUtxo && prevout) {
        const prevTx = parseTransaction(nonWitnessUtxo);
        const output = prevTx.outputs[prevout.vout];
        if (output) {
          inputTotalSats += output.amountSats;
        }
      }
    }

    const feeSats = inputTotalSats - tx.outputTotalSats;
    return inputTotalSats > 0 && feeSats >= 0 ? feeSats : undefined;
  } catch {
    return undefined;
  }
}

export class ClnNode implements LightningNode, OnchainPayments {
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
    });

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

    const payload = await this.postJson<ClnPayResponse>('/v1/pay', body);

    return {
      paymentHash: payload.payment_hash,
      preimage: payload.payment_preimage,
      feeMsats: parseOptionalNumber(payload.amount_sent_msat) - parseOptionalNumber(payload.amount_msat),
    };
  }

  async prepareOnchainTransaction(params: PrepareOnchainTransactionParams): Promise<OnchainTransaction> {
    assertValidOnchainAmount(params.amountSats);

    const fee = params.fee ?? defaultOnchainFee();
    const feePayer = resolveClnFeePayer(params.feePayer);
    const feeRequest = resolveClnFeeRequest(fee);
    const txPrepare = await this.postJson<ClnTxPrepareResponse>('/v1/txprepare', {
      outputs: [{ [params.address]: `${params.amountSats}sat` }],
      ...feeRequest,
    });
    const feeSats = parsePsbtFeeSats(txPrepare.psbt);

    return {
      id: txPrepare.txid,
      address: params.address,
      amountSats: params.amountSats,
      feeSats,
      totalAmountSats: feeSats === undefined ? undefined : params.amountSats + feeSats,
      recipientAmountSats: params.amountSats,
      feePayer,
      fee,
      raw: {
        txPrepare,
        txSendRequest: { txid: txPrepare.txid },
        feeRequest,
        description: params.description,
      },
    };
  }

  async payOnchain(transaction: OnchainTransaction, options?: PayOnchainOptions): Promise<PayOnchainResponse> {
    assertValidOnchainAmount(transaction.amountSats);
    resolveClnFeePayer(transaction.feePayer);
    resolveClnFeeRequest(transaction.fee);
    assertOnchainFeeGuardrail(transaction, options);
    if (!transaction.id) {
      throw new LniError('InvalidInput', 'CLN payOnchain requires a transaction id from prepareOnchainTransaction.');
    }

    const response = await this.postJson<ClnTxSendResponse>('/v1/txsend', {
      txid: transaction.id,
    });

    return {
      paymentId: transaction.id,
      txid: response.txid ?? transaction.id,
      state: normalizeOnchainState(response.txid ?? transaction.id),
      address: transaction.address,
      amountSats: transaction.amountSats,
      feeSats: transaction.feeSats,
      totalAmountSats: transaction.totalAmountSats,
      recipientAmountSats: transaction.recipientAmountSats ?? transaction.amountSats,
      raw: response,
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
    if (!offers.length) {
      throw new LniError('Api', search ? `Offer not found for search: ${search}` : 'Offer not found');
    }

    return offers[0]!;
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
