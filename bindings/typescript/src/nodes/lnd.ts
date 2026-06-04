import { decodeBolt11ToJson, decodeOfferToJson } from '../decode.js';
import { LniError } from '../errors.js';
import { encodeBase64Bytes, hexToBytes } from '../internal/encoding.js';
import { buildUrl, requestJson, requestText, resolveFetch, toTimeoutMs } from '../internal/http.js';
import { isEmptyPermissions, normalizeLndPermissions, parseLndMacaroonPermissions } from '../internal/permissions.js';
import { pollInvoiceEvents } from '../internal/polling.js';
import { emptyNodeInfo, emptyTransaction, parseOptionalNumber, rHashToHex } from '../internal/transform.js';
import { DEFAULT_ONCHAIN_FEE_GUARDRAIL, InvoiceType, type CreateInvoiceParams, type CreateOfferParams, type InvoiceEventCallback, type LightningNode, type ListTransactionsParams, type LookupInvoiceParams, type LndConfig, type NodeInfo, type NodeRequestOptions, type Offer, type OnInvoiceEventParams, type OnchainFeeGuardrail, type OnchainFeePayer, type OnchainFeePreference, type OnchainPayments, type OnchainTransaction, type PayInvoiceParams, type PayInvoiceResponse, type PayOnchainOptions, type PayOnchainResponse, type Permissions, type PrepareOnchainTransactionParams, type Transaction } from '../types.js';

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

interface LndEstimateFeeResponse {
  fee_sat?: string;
  feerate_sat_per_byte?: string;
  sat_per_vbyte?: string;
}

interface LndSendCoinsResponse {
  txid?: string;
}

interface LndOnchainFeeRequest {
  target_conf?: number;
  sat_per_vbyte?: string;
}

function defaultOnchainFee(): OnchainFeePreference {
  return { type: 'targetConf', blocks: 6 };
}

function resolveLndFeePayer(feePayer?: OnchainFeePayer): OnchainFeePayer {
  if (feePayer === 'recipient') {
    throw new LniError('InvalidInput', 'LND payOnchain only supports sender-paid on-chain fees.');
  }

  return 'sender';
}

function resolveLndFeeRequest(fee: OnchainFeePreference): LndOnchainFeeRequest {
  switch (fee.type) {
    case 'default':
      return { target_conf: 6 };
    case 'speed':
      switch (fee.speed) {
        case 'fast':
          return { target_conf: 1 };
        case 'normal':
          return { target_conf: 6 };
        case 'slow':
          return { target_conf: 12 };
        case 'free':
          throw new LniError('InvalidInput', 'LND payOnchain does not support free on-chain fee speed.');
      }
    case 'targetConf':
      if (!Number.isSafeInteger(fee.blocks) || fee.blocks <= 0) {
        throw new LniError('InvalidInput', 'LND targetConf fee preference requires a positive integer block target.');
      }
      return { target_conf: fee.blocks };
    case 'satsPerVbyte':
      if (!Number.isFinite(fee.satsPerVbyte) || fee.satsPerVbyte <= 0) {
        throw new LniError('InvalidInput', 'LND satsPerVbyte fee preference requires a positive number.');
      }
      return { sat_per_vbyte: String(Math.ceil(fee.satsPerVbyte)) };
    case 'backend':
      throw new LniError('InvalidInput', 'LND payOnchain does not support backend fee preferences.');
  }
}

function normalizeOnchainState(txid?: string): PayOnchainResponse['state'] {
  return txid ? 'pending' : 'failed';
}

function parseOptionalFeeSats(value: unknown): number | undefined {
  if (value === undefined || value === null || value === '') {
    return undefined;
  }

  const parsed = parseOptionalNumber(value);
  return Number.isSafeInteger(parsed) && parsed >= 0 ? parsed : undefined;
}

function assertValidOnchainAmount(amountSats: number): void {
  if (!Number.isSafeInteger(amountSats) || amountSats <= 0) {
    throw new LniError('InvalidInput', 'payOnchain requires a positive integer amountSats.');
  }
}

function assertValidGuardrailLimit(value: number, name: string): void {
  if (!Number.isFinite(value) || value < 0) {
    throw new LniError('InvalidInput', `${name} must be a non-negative finite number.`);
  }

  if (name.endsWith('maxFeeSats') && !Number.isSafeInteger(value)) {
    throw new LniError('InvalidInput', `${name} must be a safe integer.`);
  }
}

function resolveOnchainFeeGuardrail(options?: PayOnchainOptions): Required<OnchainFeeGuardrail> | undefined {
  if (options?.dangerouslyDisableFeeGuardrail) {
    return undefined;
  }

  const guardrail = {
    maxFeeSats: options?.feeGuardrail?.maxFeeSats ?? DEFAULT_ONCHAIN_FEE_GUARDRAIL.maxFeeSats,
    maxFeePercent: options?.feeGuardrail?.maxFeePercent ?? DEFAULT_ONCHAIN_FEE_GUARDRAIL.maxFeePercent,
  };

  assertValidGuardrailLimit(guardrail.maxFeeSats, 'feeGuardrail.maxFeeSats');
  assertValidGuardrailLimit(guardrail.maxFeePercent, 'feeGuardrail.maxFeePercent');

  return guardrail;
}

function assertOnchainFeeGuardrail(transaction: OnchainTransaction, options?: PayOnchainOptions): void {
  const guardrail = resolveOnchainFeeGuardrail(options);
  if (!guardrail) {
    return;
  }

  const { feeSats } = transaction;
  if (feeSats === undefined) {
    throw new LniError(
      'InvalidInput',
      'Cannot pay on-chain transaction because feeSats is unknown. Re-prepare the transaction or pass dangerouslyDisableFeeGuardrail: true.',
    );
  }

  if (!Number.isSafeInteger(feeSats) || feeSats < 0) {
    throw new LniError('InvalidInput', 'Cannot pay on-chain transaction because feeSats is invalid.');
  }

  if (!Number.isSafeInteger(transaction.amountSats) || transaction.amountSats <= 0) {
    throw new LniError('InvalidInput', 'Cannot pay on-chain transaction because amountSats is invalid.');
  }

  if (feeSats > guardrail.maxFeeSats) {
    throw new LniError(
      'InvalidInput',
      `On-chain fee ${feeSats} sats exceeds guardrail maxFeeSats ${guardrail.maxFeeSats}.`,
    );
  }

  const feePercent = (feeSats / transaction.amountSats) * 100;
  if (feePercent > guardrail.maxFeePercent) {
    throw new LniError(
      'InvalidInput',
      `On-chain fee ${feePercent.toFixed(2)}% exceeds guardrail maxFeePercent ${guardrail.maxFeePercent}%.`,
    );
  }
}

export class LndNode implements LightningNode, OnchainPayments {
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

  private async getJsonWithQuery<T>(path: string, query: Record<string, string | number | boolean | undefined>): Promise<T> {
    return requestJson<T>(this.fetchFn, buildUrl(this.config.url, path, query), {
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

  async getPermissions(): Promise<Permissions> {
    const macaroonBytes = hexToBytes(this.config.macaroon);

    try {
      const payload = await this.getJson<LndListPermissionsResponse>('/v1/macaroon/permissions');
      const methodPermissions = Object.entries(payload.method_permissions ?? {});
      const macaroon = encodeBase64Bytes(macaroonBytes);
      const granted: string[] = [];

      await Promise.all(
        methodPermissions.map(async ([method, permissionList]) => {
          const response = await this.postJson<LndCheckMacaroonPermissionsResponse>('/v1/macaroon/checkpermissions', {
            macaroon,
            permissions: permissionList.permissions ?? [],
          });

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

  async prepareOnchainTransaction(params: PrepareOnchainTransactionParams): Promise<OnchainTransaction> {
    const amountSats = params.amountSats;
    assertValidOnchainAmount(amountSats);

    const fee = params.fee ?? defaultOnchainFee();
    const feePayer = resolveLndFeePayer(params.feePayer);
    const feeRequest = resolveLndFeeRequest(fee);
    const estimate = await this.getJsonWithQuery<LndEstimateFeeResponse>('/v1/transactions/fee', {
      [`AddrToAmount[${params.address}]`]: amountSats,
      ...feeRequest,
    });
    const feeSats = parseOptionalFeeSats(estimate.fee_sat);

    return {
      address: params.address,
      amountSats,
      feeSats,
      totalAmountSats: feeSats === undefined ? undefined : amountSats + feeSats,
      recipientAmountSats: amountSats,
      feePayer,
      fee,
      raw: {
        estimate,
        sendRequest: feeRequest,
        label: params.description,
      },
    };
  }

  async payOnchain(transaction: OnchainTransaction, options?: PayOnchainOptions): Promise<PayOnchainResponse> {
    assertValidOnchainAmount(transaction.amountSats);
    resolveLndFeePayer(transaction.feePayer);
    const feeRequest = resolveLndFeeRequest(transaction.fee);
    assertOnchainFeeGuardrail(transaction, options);

    const response = await this.postJson<LndSendCoinsResponse>('/v1/transactions', {
      addr: transaction.address,
      amount: transaction.amountSats,
      ...feeRequest,
      label: typeof transaction.raw === 'object' && transaction.raw !== null && !Array.isArray(transaction.raw)
        ? (transaction.raw as { label?: unknown }).label
        : undefined,
    });

    return {
      txid: response.txid,
      state: normalizeOnchainState(response.txid),
      address: transaction.address,
      amountSats: transaction.amountSats,
      feeSats: transaction.feeSats,
      totalAmountSats: transaction.totalAmountSats,
      recipientAmountSats: transaction.recipientAmountSats ?? transaction.amountSats,
      raw: response,
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

  async listTransactions(params: ListTransactionsParams): Promise<Transaction[]> {
    const payload = await this.getJson<LndInvoiceListResponse>('/v1/invoices');
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
