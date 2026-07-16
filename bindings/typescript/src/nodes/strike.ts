import { decode as decodeBolt11, decodeBolt11ToJson, decodeOfferToJson } from '../decode.js';
import { LniError, NwcError, type NwcErrorCode, type NwcErrorOperation } from '../errors.js';
import {
  findStringProperty,
  providerInfoFromJsonErrorBody,
  throwNormalizedProviderError,
  type ProviderErrorInfo,
} from '../internal/error-normalization.js';
import { buildUrl, requestJson, requestText, resolveFetch, toTimeoutMs } from '../internal/http.js';
import { getStrikeOauthPermissions } from '../internal/permissions.js';
import { pollInvoiceEvents } from '../internal/polling.js';
import {
  btcToMsats,
  emptyNodeInfo,
  emptyTransaction,
  matchesSearch,
  msatsToBtc,
  parseOptionalNumber,
  toUnixSeconds,
} from '../internal/transform.js';
import {
  DEFAULT_ONCHAIN_FEE_GUARDRAIL,
  InvoiceType,
  type CreateInvoiceParams,
  type CreateOfferParams,
  type InvoiceEventCallback,
  type LightningNode,
  type ListTransactionsParams,
  type LookupInvoiceParams,
  type NodeInfo,
  type NodeRequestOptions,
  type Offer,
  type OnInvoiceEventParams,
  type OnchainFeeGuardrail,
  type OnchainFeePayer,
  type OnchainFeePreference,
  type OnchainPayments,
  type PayInvoiceParams,
  type PayInvoiceResponse,
  type PayOnchainOptions,
  type PayOnchainResponse,
  type Permissions,
  type OnchainTransaction,
  type PrepareOnchainTransactionParams,
  type StrikeConfig,
  type Transaction,
} from '../types.js';

interface StrikeBalance {
  currency: string;
  current: string;
}

interface StrikeAmount {
  amount: string;
  currency: string;
  feePolicy?: 'EXCLUSIVE' | 'INCLUSIVE';
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
  state?: string;
  amount?: StrikeAmount;
  totalFee?: StrikeAmount;
  totalAmount?: StrikeAmount;
  onchain?: {
    txnId?: string;
  };
}

interface StrikePaymentResponse {
  id: string;
  paymentId?: string;
  state: string;
  created: string;
  completed?: string;
  description?: string;
  amount: StrikeAmount;
  totalFee?: StrikeAmount;
  totalAmount?: StrikeAmount;
  lightning?: {
    paymentHash?: string;
    paymentRequest?: string;
    networkFee?: StrikeAmount;
    preImage?: string;
  };
  onchain?: {
    txnId?: string;
  };
}

interface StrikeOnchainTierResponse {
  id: string;
  estimatedDeliveryDurationInMin?: number;
  estimatedFee?: StrikeAmount;
  minimumAmount?: StrikeAmount;
}

interface StrikeOnchainPaymentQuoteResponse {
  paymentQuoteId: string;
  estimatedDeliveryDurationInMin?: number;
  description?: string;
  validUntil?: string;
  amount: StrikeAmount;
  totalFee?: StrikeAmount;
  totalAmount: StrikeAmount;
}

interface DuplicatePaymentQuote {
  paymentQuoteId: string;
  raw: unknown;
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

function paymentHashFromInvoice(invoice: string): string {
  try {
    const decoded = decodeBolt11(invoice);
    return decoded.payment_hash ?? '';
  } catch {
    return '';
  }
}

function normalizeOnchainState(state?: string): PayOnchainResponse['state'] {
  switch (state?.toUpperCase()) {
    case 'PENDING':
      return 'pending';
    case 'COMPLETED':
    case 'SUCCESS':
      return 'completed';
    case 'FAILED':
    case 'FAILURE':
      return 'failed';
    default:
      return state?.toLowerCase() ?? 'pending';
  }
}

function satsToBtc(amountSats: number): string {
  return (amountSats / 100_000_000).toFixed(8);
}

function onchainAmountToSats(amount?: StrikeAmount): number | undefined {
  if (!amount) {
    return undefined;
  }

  if (amount.currency !== 'BTC') {
    return undefined;
  }

  const btc = Number.parseFloat(amount.amount);
  return Number.isFinite(btc) ? Math.round(btc * 100_000_000) : undefined;
}

function defaultOnchainFee(): OnchainFeePreference {
  return { type: 'speed', speed: 'normal' };
}

function resolveOnchainFeePayer(feePayer?: OnchainFeePayer): OnchainFeePayer {
  return feePayer ?? 'sender';
}

function strikeFeePolicy(feePayer: OnchainFeePayer): 'EXCLUSIVE' | 'INCLUSIVE' {
  return feePayer === 'recipient' ? 'INCLUSIVE' : 'EXCLUSIVE';
}

function normalizeStrikeTierSpeed(fee: OnchainFeePreference): 'fast' | 'standard' | 'free' {
  if (fee.type === 'default') {
    return 'standard';
  }

  if (fee.type !== 'speed') {
    throw new LniError(
      'InvalidInput',
      `Strike payOnchain does not support ${fee.type} fee preferences.`
    );
  }

  switch (fee.speed) {
    case 'fast':
      return 'fast';
    case 'normal':
      return 'standard';
    case 'slow':
    case 'free':
      return 'free';
  }
}

function assertValidOnchainAmount(amountSats: number): void {
  if (!Number.isSafeInteger(amountSats) || amountSats <= 0) {
    throw new LniError('InvalidInput', 'payOnchain requires a positive integer amountSats.');
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function strikeProviderErrorInfo(error: unknown): ProviderErrorInfo | undefined {
  return providerInfoFromJsonErrorBody(error);
}

function mapStrikeProviderError(info: ProviderErrorInfo): NwcErrorCode | undefined {
  switch (info.code) {
    case 'BALANCE_TOO_LOW':
      return 'INSUFFICIENT_BALANCE';
    case 'RATE_LIMIT_EXCEEDED':
    case 'TOO_MANY_ATTEMPTS':
      return 'RATE_LIMITED';
    case 'FORBIDDEN':
      return 'RESTRICTED';
    case 'UNAUTHORIZED':
      return 'UNAUTHORIZED';
    case 'AMOUNT_TOO_HIGH':
    case 'TOO_MANY_TRANSACTIONS':
    case 'DEPOSIT_LIMIT_EXCEEDED':
      return 'QUOTA_EXCEEDED';
    case 'INVALID_LN_INVOICE':
    case 'INVALID_STATE_FOR_INVOICE_EXPIRED':
    case 'LN_ROUTE_NOT_FOUND':
    case 'PAYMENT_QUOTE_EXPIRED':
    case 'INVALID_RECIPIENT':
      return 'PAYMENT_FAILED';
    case 'EXCHANGE_RATE_NOT_AVAILABLE':
    case 'LN_UNAVAILABLE':
    case 'SERVICE_UNAVAILABLE':
    case 'MAINTENANCE_MODE':
    case 'BAD_GATEWAY':
    case 'GATEWAY_TIMEOUT':
    case 'INTERNAL_SERVER_ERROR':
      return 'INTERNAL';
    case 'NOT_FOUND':
      return 'NOT_FOUND';
    default:
      return undefined;
  }
}

function strikeNwcError(
  code: NwcErrorCode,
  message: string,
  operation: NwcErrorOperation,
  info?: ProviderErrorInfo
): NwcError {
  return new NwcError(code, message, {
    operation,
    provider: 'strike',
    providerCode: info?.code,
    providerStatus: info?.status,
    providerMessage: info?.message ?? message,
  });
}

function throwStrikeError(error: unknown, operation: NwcErrorOperation): never {
  throwNormalizedProviderError(error, {
    provider: 'strike',
    operation,
    extractProviderError: strikeProviderErrorInfo,
    mapProviderError: mapStrikeProviderError,
  });
}

function containsDuplicatePaymentQuoteCode(value: unknown): boolean {
  if (Array.isArray(value)) {
    return value.some((child) => containsDuplicatePaymentQuoteCode(child));
  }

  if (!isRecord(value)) {
    return false;
  }

  for (const child of Object.values(value)) {
    if (child === 'DUPLICATE_PAYMENT_QUOTE') {
      return true;
    }

    if (containsDuplicatePaymentQuoteCode(child)) {
      return true;
    }
  }

  return false;
}

function duplicatePaymentQuoteFromError(error: unknown): DuplicatePaymentQuote | undefined {
  if (
    !(error instanceof LniError) ||
    error.code !== 'Http' ||
    error.status !== 422 ||
    !error.body
  ) {
    return undefined;
  }

  let raw: unknown;
  try {
    raw = JSON.parse(error.body);
  } catch {
    return undefined;
  }

  if (!containsDuplicatePaymentQuoteCode(raw)) {
    return undefined;
  }

  const paymentQuoteId = findStringProperty(raw, 'paymentQuoteId');
  return paymentQuoteId ? { paymentQuoteId, raw } : undefined;
}

function assertValidGuardrailLimit(value: number, name: string): void {
  if (!Number.isFinite(value) || value < 0) {
    throw new LniError('InvalidInput', `${name} must be a non-negative finite number.`);
  }

  if (name.endsWith('maxFeeSats') && !Number.isSafeInteger(value)) {
    throw new LniError('InvalidInput', `${name} must be a safe integer.`);
  }
}

function resolveOnchainFeeGuardrail(
  options?: PayOnchainOptions
): Required<OnchainFeeGuardrail> | undefined {
  if (options?.dangerouslyDisableFeeGuardrail) {
    return undefined;
  }

  const guardrail = {
    maxFeeSats: options?.feeGuardrail?.maxFeeSats ?? DEFAULT_ONCHAIN_FEE_GUARDRAIL.maxFeeSats,
    maxFeePercent:
      options?.feeGuardrail?.maxFeePercent ?? DEFAULT_ONCHAIN_FEE_GUARDRAIL.maxFeePercent,
  };

  assertValidGuardrailLimit(guardrail.maxFeeSats, 'feeGuardrail.maxFeeSats');
  assertValidGuardrailLimit(guardrail.maxFeePercent, 'feeGuardrail.maxFeePercent');

  return guardrail;
}

function assertOnchainFeeGuardrail(
  transaction: OnchainTransaction,
  options?: PayOnchainOptions
): void {
  const guardrail = resolveOnchainFeeGuardrail(options);
  if (!guardrail) {
    return;
  }

  const { feeSats } = transaction;
  if (feeSats === undefined) {
    throw new LniError(
      'InvalidInput',
      'Cannot pay on-chain transaction because feeSats is unknown. Re-prepare the transaction or pass dangerouslyDisableFeeGuardrail: true.'
    );
  }

  if (!Number.isSafeInteger(feeSats) || feeSats < 0) {
    throw new LniError(
      'InvalidInput',
      'Cannot pay on-chain transaction because feeSats is invalid.'
    );
  }

  if (!Number.isSafeInteger(transaction.amountSats) || transaction.amountSats <= 0) {
    throw new LniError(
      'InvalidInput',
      'Cannot pay on-chain transaction because amountSats is invalid.'
    );
  }

  if (feeSats > guardrail.maxFeeSats) {
    throw new LniError(
      'InvalidInput',
      `On-chain fee ${feeSats} sats exceeds guardrail maxFeeSats ${guardrail.maxFeeSats}.`
    );
  }

  const feePercent = (feeSats / transaction.amountSats) * 100;
  if (feePercent > guardrail.maxFeePercent) {
    throw new LniError(
      'InvalidInput',
      `On-chain fee ${feePercent.toFixed(2)}% exceeds guardrail maxFeePercent ${guardrail.maxFeePercent}%.`
    );
  }
}

export class StrikeNode implements LightningNode, OnchainPayments {
  private readonly fetchFn;
  private readonly timeoutMs?: number;
  private readonly baseUrl: string;

  constructor(
    private readonly config: StrikeConfig,
    options: NodeRequestOptions = {}
  ) {
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

  private async getJson<T>(
    path: string,
    query?: Record<string, string | number | undefined>,
    operation?: NwcErrorOperation
  ): Promise<T> {
    try {
      return await requestJson<T>(this.fetchFn, buildUrl(this.baseUrl, path, query), {
        method: 'GET',
        headers: this.headers(),
        timeoutMs: this.timeoutMs,
      });
    } catch (error) {
      if (operation) {
        throwStrikeError(error, operation);
      }
      throw error;
    }
  }

  private async postJson<T>(
    path: string,
    json?: unknown,
    headers?: HeadersInit,
    operation?: NwcErrorOperation
  ): Promise<T> {
    try {
      return await requestJson<T>(this.fetchFn, buildUrl(this.baseUrl, path), {
        method: 'POST',
        headers: this.headers(headers),
        json,
        timeoutMs: this.timeoutMs,
      });
    } catch (error) {
      if (operation) {
        throwStrikeError(error, operation);
      }
      throw error;
    }
  }

  private async patchJson<T>(path: string, operation?: NwcErrorOperation): Promise<T> {
    try {
      return await requestJson<T>(this.fetchFn, buildUrl(this.baseUrl, path), {
        method: 'PATCH',
        headers: this.headers(),
        timeoutMs: this.timeoutMs,
      });
    } catch (error) {
      if (operation) {
        throwStrikeError(error, operation);
      }
      throw error;
    }
  }

  private isNotFoundError(error: unknown): boolean {
    return (
      (error instanceof LniError && error.code === 'Http' && error.status === 404) ||
      (error instanceof NwcError && error.nwcCode === 'NOT_FOUND')
    );
  }

  async getPermissions(): Promise<Permissions> {
    const permissions = getStrikeOauthPermissions(this.config.apiKey);
    if (!permissions) {
      throw new LniError(
        'InvalidInput',
        'Strike API keys cannot be introspected. Use an OAuth access token or manually test permissions against Strike REST endpoints.'
      );
    }

    return permissions;
  }

  async getInfo(): Promise<NodeInfo> {
    const balances = await this.getJson<StrikeBalance[]>('/balances', undefined, 'get_info');

    const btcBalance = balances.find((balance) => balance.currency === 'BTC');

    return emptyNodeInfo({
      alias: 'Strike Node',
      network: 'mainnet',
      sendBalanceMsat: btcBalance ? btcToMsats(btcBalance.current) : 0,
    });
  }

  async createInvoice(params: CreateInvoiceParams): Promise<Transaction> {
    if ((params.invoiceType ?? InvoiceType.Bolt11) !== InvoiceType.Bolt11) {
      throw strikeNwcError(
        'NOT_IMPLEMENTED',
        'Bolt12 is not implemented for StrikeNode.',
        'make_invoice'
      );
    }

    const response = await this.postJson<StrikeCreateReceiveResponse>(
      '/receive-requests',
      {
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
      },
      undefined,
      'make_invoice'
    );

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
    const quote = await this.postJson<StrikePaymentQuoteResponse>(
      '/payment-quotes/lightning',
      {
        lnInvoice: params.invoice,
        sourceCurrency: 'BTC',
        amount:
          params.amountMsats !== undefined
            ? {
                amount: msatsToBtc(params.amountMsats),
                currency: 'BTC',
              }
            : undefined,
      },
      undefined,
      'pay_invoice'
    );

    const execution = await this.patchJson<StrikePaymentExecutionResponse>(
      `/payment-quotes/${quote.paymentQuoteId}/execute`,
      'pay_invoice'
    );
    let payment: StrikePaymentResponse | undefined;
    // The pre-image appears on the payment record once it settles (usually
    // immediately for Lightning) — poll the read briefly to capture the
    // proof of payment.
    for (let attempt = 0; attempt < 5 && !payment?.lightning?.preImage; attempt++) {
      if (attempt > 0) {
        await new Promise((resolve) => setTimeout(resolve, 400));
      }
      try {
        payment = await this.getJson<StrikePaymentResponse>(`/payments/${execution.paymentId}`);
      } catch {
        // payment.read is optional for payInvoice; without it we still know the payment was executed.
        break;
      }
    }

    const feeMsats = payment?.lightning?.networkFee
      ? btcToMsats(payment.lightning.networkFee.amount)
      : 0;

    return {
      paymentHash: payment?.lightning?.paymentHash ?? paymentHashFromInvoice(params.invoice),
      preimage: payment?.lightning?.preImage ?? '',
      feeMsats,
    };
  }

  async prepareOnchainTransaction(
    params: PrepareOnchainTransactionParams
  ): Promise<OnchainTransaction> {
    const amountSats = params.amountSats;
    assertValidOnchainAmount(amountSats);

    const fee = params.fee ?? defaultOnchainFee();
    const feePayer = resolveOnchainFeePayer(params.feePayer);
    const onchainTierId = await this.resolveOnchainTierId(params.address, amountSats, fee);

    let quote: StrikeOnchainPaymentQuoteResponse;
    try {
      quote = await this.postJson<StrikeOnchainPaymentQuoteResponse>(
        '/payment-quotes/onchain',
        {
          btcAddress: params.address,
          sourceCurrency: 'BTC',
          description: params.description,
          amount: {
            amount: satsToBtc(amountSats),
            currency: 'BTC',
            feePolicy: strikeFeePolicy(feePayer),
          },
          onchainTierId,
        },
        params.idempotencyKey ? { 'idempotency-key': params.idempotencyKey } : undefined
      );
    } catch (error) {
      const duplicate = duplicatePaymentQuoteFromError(error);
      if (!duplicate) {
        throw error;
      }

      return this.onchainTransactionFromDuplicate(
        params.address,
        amountSats,
        fee,
        feePayer,
        duplicate
      );
    }

    return this.onchainTransactionFromQuote(params.address, amountSats, fee, feePayer, quote);
  }

  async payOnchain(
    transaction: OnchainTransaction,
    options?: PayOnchainOptions
  ): Promise<PayOnchainResponse> {
    if (!transaction.id) {
      throw new LniError('InvalidInput', 'payOnchain requires an on-chain transaction id.');
    }

    assertOnchainFeeGuardrail(transaction, options);

    const execution = await this.patchJson<StrikePaymentExecutionResponse>(
      `/payment-quotes/${transaction.id}/execute`
    );
    let payment: StrikePaymentResponse | undefined;
    try {
      payment = await this.getJson<StrikePaymentResponse>(`/payments/${execution.paymentId}`);
    } catch {
      // payment.read is optional; execute may already include enough information.
    }

    return this.payOnchainResponseFromPayment(transaction, execution, payment);
  }

  private async resolveOnchainTierId(
    address: string,
    amountSats: number,
    fee: OnchainFeePreference
  ): Promise<string> {
    if (fee.type === 'backend') {
      if (!fee.value) {
        throw new LniError(
          'InvalidInput',
          'Strike backend fee preference requires a tier id value.'
        );
      }
      return fee.value;
    }

    const tierSpeed = normalizeStrikeTierSpeed(fee);
    const tiers = await this.postJson<StrikeOnchainTierResponse[]>(
      '/payment-quotes/onchain/tiers',
      {
        btcAddress: address,
        amount: {
          amount: satsToBtc(amountSats),
          currency: 'BTC',
        },
      }
    );

    const preferredTier =
      tiers.find((tier) => tier.id === `tier_${tierSpeed}`) ??
      tiers.find((tier) => tier.id.toLowerCase().includes(tierSpeed));

    if (!preferredTier) {
      throw new LniError('Api', `Strike did not return an on-chain fee tier for ${tierSpeed}.`);
    }

    return preferredTier.id;
  }

  private onchainTransactionFromQuote(
    address: string,
    amountSats: number,
    fee: OnchainFeePreference,
    feePayer: OnchainFeePayer,
    quote: StrikeOnchainPaymentQuoteResponse
  ): OnchainTransaction {
    return {
      id: quote.paymentQuoteId,
      address,
      amountSats,
      feeSats: onchainAmountToSats(quote.totalFee),
      totalAmountSats: onchainAmountToSats(quote.totalAmount),
      recipientAmountSats: onchainAmountToSats(quote.amount),
      feePayer,
      fee,
      expiresAt: quote.validUntil ? toUnixSeconds(Date.parse(quote.validUntil)) : undefined,
      estimatedDeliverySeconds:
        quote.estimatedDeliveryDurationInMin !== undefined
          ? quote.estimatedDeliveryDurationInMin * 60
          : undefined,
      raw: quote,
    };
  }

  private onchainTransactionFromDuplicate(
    address: string,
    amountSats: number,
    fee: OnchainFeePreference,
    feePayer: OnchainFeePayer,
    duplicate: DuplicatePaymentQuote
  ): OnchainTransaction {
    return {
      id: duplicate.paymentQuoteId,
      address,
      amountSats,
      feePayer,
      fee,
      raw: duplicate.raw,
    };
  }

  private payOnchainResponseFromPayment(
    transaction: OnchainTransaction,
    execution: StrikePaymentExecutionResponse,
    payment?: StrikePaymentResponse
  ): PayOnchainResponse {
    const amountSats =
      onchainAmountToSats(payment?.amount ?? execution.amount) ?? transaction.amountSats;
    const feeSats =
      onchainAmountToSats(payment?.totalFee ?? execution.totalFee) ?? transaction.feeSats;
    const totalAmountSats =
      onchainAmountToSats(payment?.totalAmount ?? execution.totalAmount) ??
      transaction.totalAmountSats;
    const createdAt = payment?.created ? toUnixSeconds(Date.parse(payment.created)) : undefined;

    return {
      paymentId: payment?.paymentId ?? payment?.id ?? execution.paymentId,
      txid: payment?.onchain?.txnId ?? execution.onchain?.txnId,
      state: normalizeOnchainState(payment?.state ?? execution.state),
      address: transaction.address,
      amountSats,
      feeSats,
      totalAmountSats,
      recipientAmountSats: transaction.recipientAmountSats,
      createdAt,
      raw: payment ?? execution,
    };
  }

  async createOffer(_params: CreateOfferParams): Promise<Offer> {
    throw strikeNwcError(
      'NOT_IMPLEMENTED',
      'Bolt12 is not implemented for StrikeNode.',
      'make_invoice'
    );
  }

  async getOffer(_search?: string): Promise<Offer> {
    throw strikeNwcError(
      'NOT_IMPLEMENTED',
      'Bolt12 is not implemented for StrikeNode.',
      'lookup_invoice'
    );
  }

  async listOffers(_search?: string): Promise<Offer[]> {
    throw strikeNwcError(
      'NOT_IMPLEMENTED',
      'Bolt12 is not implemented for StrikeNode.',
      'list_transactions'
    );
  }

  async payOffer(
    _offer: string,
    _amountMsats: number,
    _payerNote?: string
  ): Promise<PayInvoiceResponse> {
    throw strikeNwcError(
      'NOT_IMPLEMENTED',
      'Bolt12 is not implemented for StrikeNode.',
      'pay_invoice'
    );
  }

  async lookupInvoice(params: LookupInvoiceParams): Promise<Transaction> {
    if (!params.paymentHash) {
      throw new LniError('InvalidInput', 'lookupInvoice requires paymentHash for StrikeNode.');
    }

    const receives = await this.getJson<StrikeReceivesResponse>(
      '/receive-requests/receives',
      {
        $paymentHash: params.paymentHash,
      },
      'lookup_invoice'
    );

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
      settledAt:
        item.state === 'COMPLETED' && item.completed
          ? toUnixSeconds(Date.parse(item.completed))
          : 0,
      description: item.lightning.description ?? item.lightning.descriptionHash ?? '',
      descriptionHash: item.lightning.descriptionHash ?? '',
      externalId: item.receiveRequestId,
      payerNote: '',
    });
  }

  async listTransactions(params: ListTransactionsParams): Promise<Transaction[]> {
    const receives = await this.getJson<StrikeReceivesResponse>(
      '/receive-requests/receives',
      {
        $skip: params.from,
        $top: params.limit,
      },
      'list_transactions'
    );

    let outgoing: StrikePaymentsResponse = { data: [] };
    try {
      outgoing = await this.getJson<StrikePaymentsResponse>(
        '/payments',
        {
          $skip: params.from,
          $top: params.limit,
        },
        'list_transactions'
      );
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
        settledAt:
          receive.state === 'COMPLETED' && receive.completed
            ? toUnixSeconds(Date.parse(receive.completed))
            : 0,
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
        feesPaid: payment.lightning?.networkFee
          ? btcToMsats(payment.lightning.networkFee.amount)
          : 0,
        createdAt: toUnixSeconds(Date.parse(payment.created)),
        settledAt:
          payment.state === 'COMPLETED' && payment.completed
            ? toUnixSeconds(Date.parse(payment.completed))
            : 0,
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

    const sorted = filtered.sort((a, b) => b.createdAt - a.createdAt);
    return sorted.slice(0, params.limit > 0 ? params.limit : undefined);
  }

  async decode(str: string): Promise<string> {
    return decodeBolt11ToJson(str);
  }

  async decodeOffer(offer: string): Promise<string> {
    return decodeOfferToJson(offer);
  }

  async onInvoiceEvents(
    params: OnInvoiceEventParams,
    callback: InvoiceEventCallback
  ): Promise<void> {
    await pollInvoiceEvents({
      params,
      callback,
      lookup: () => this.lookupInvoice({ paymentHash: params.paymentHash, search: params.search }),
    });
  }
}
