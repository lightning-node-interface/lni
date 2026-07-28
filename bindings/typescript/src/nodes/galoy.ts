import { LniError, NwcError, type NwcErrorCode, type NwcErrorOperation } from '../errors.js';
import { decode as decodeBolt11, decodeBolt11ToJson, decodeOfferToJson } from '../decode.js';
import {
  mapProviderMessage,
  throwNormalizedProviderError,
  type ProviderErrorInfo,
} from '../internal/error-normalization.js';
import { requestJson, resolveFetch, toTimeoutMs } from '../internal/http.js';
import { getGaloyTokenPermissions } from '../internal/permissions.js';
import { pollInvoiceEvents } from '../internal/polling.js';
import {
  emptyNodeInfo,
  emptyTransaction,
  matchesSearch,
  satsToMsats,
} from '../internal/transform.js';
import {
  DEFAULT_ONCHAIN_FEE_GUARDRAIL,
  InvoiceType,
  type GaloyConfig,
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
  type OnchainTransaction,
  type PayInvoiceParams,
  type PayInvoiceResponse,
  type PayOnchainOptions,
  type PayOnchainResponse,
  type Permissions,
  type PrepareOnchainTransactionParams,
  type Transaction,
} from '../types.js';

interface GraphQLError {
  message: string;
  code?: string;
  path?: string[];
}

interface GraphQLResponse<T> {
  data?: T;
  errors?: GraphQLError[];
}

interface GaloyMeQuery {
  me: {
    defaultAccount: {
      wallets: GaloyWallet[];
    };
  };
}

interface GaloyWallet {
  id: string;
  walletCurrency: string;
  balance?: number;
}

interface GaloyInvoiceCreateResult {
  invoice?: {
    paymentRequest: string;
    paymentHash: string;
    satoshis: number;
  };
  errors?: GraphQLError[];
}

interface GaloyInvoiceCreateResponse {
  lnInvoiceCreate?: GaloyInvoiceCreateResult;
  lnUsdInvoiceCreate?: GaloyInvoiceCreateResult;
}

interface GaloyFeeProbeResult {
  amount?: number;
  errors?: GraphQLError[];
}

interface GaloyFeeProbeResponse {
  lnInvoiceFeeProbe?: GaloyFeeProbeResult;
  lnUsdInvoiceFeeProbe?: GaloyFeeProbeResult;
}

interface GaloyPaymentSendResponse {
  lnInvoicePaymentSend: {
    status: string;
    transaction?: {
      settlementVia?: {
        preImage?: string;
      };
    };
    errors?: GraphQLError[];
  };
}

interface GaloyOnchainTxFeeResponse {
  onChainTxFee: {
    amount?: number;
  };
}

interface GaloyOnchainPaymentSendResponse {
  onChainPaymentSend: {
    status: string;
    transaction?: {
      id?: string;
      settlementAmount?: number;
      settlementCurrency?: string;
      settlementFee?: number;
      settlementVia?: {
        __typename: string;
        transactionHash?: string;
      };
    };
    errors?: GraphQLError[];
  };
}

/*
 * Galoy's non-BTC invoice operations retain the historical `lnUsd` names even
 * for wallets whose actual currency is neither USD nor BTC.
 */
type GaloyInvoiceOperationFamily = {
  createField: 'lnInvoiceCreate' | 'lnUsdInvoiceCreate';
  createInput: 'LnInvoiceCreateInput' | 'LnUsdInvoiceCreateInput';
  feeField: 'lnInvoiceFeeProbe' | 'lnUsdInvoiceFeeProbe';
  feeInput: 'LnInvoiceFeeProbeInput' | 'LnUsdInvoiceFeeProbeInput';
};

function invoiceOperationFamily(currency: string): GaloyInvoiceOperationFamily {
  return currency.toUpperCase() === 'BTC'
    ? {
        createField: 'lnInvoiceCreate',
        createInput: 'LnInvoiceCreateInput',
        feeField: 'lnInvoiceFeeProbe',
        feeInput: 'LnInvoiceFeeProbeInput',
      }
    : {
        createField: 'lnUsdInvoiceCreate',
        createInput: 'LnUsdInvoiceCreateInput',
        feeField: 'lnUsdInvoiceFeeProbe',
        feeInput: 'LnUsdInvoiceFeeProbeInput',
      };
}

interface GaloyTransactionsQuery {
  me: {
    defaultAccount: {
      transactions: {
        edges: Array<{
          cursor: string;
          node: {
            id: string;
            createdAt: number;
            direction: 'SEND' | 'RECEIVE';
            status: string;
            memo?: string;
            settlementAmount?: number;
            settlementCurrency?: string;
            settlementFee?: number;
            initiationVia?: {
              __typename: string;
              paymentHash?: string;
            };
            settlementVia?: {
              __typename: string;
              preImage?: string;
            };
          };
        }>;
        pageInfo: {
          hasNextPage: boolean;
          endCursor?: string | null;
        };
      };
    };
  };
}

type GaloyTransactionNode =
  GaloyTransactionsQuery['me']['defaultAccount']['transactions']['edges'][number]['node'];

interface GaloyTransactionsPage {
  transactions: Transaction[];
  nextCursor: string | null;
}

function defaultOnchainFee(): OnchainFeePreference {
  return { type: 'default' };
}

function resolveGaloyFeeSpeed(fee: OnchainFeePreference): 'FAST' | 'MEDIUM' | 'SLOW' {
  if (fee.type === 'default') {
    return 'FAST';
  }

  if (fee.type !== 'speed') {
    throw new LniError(
      'InvalidInput',
      `Galoy on-chain payments do not support ${fee.type} fee preferences.`
    );
  }

  switch (fee.speed) {
    case 'fast':
      return 'FAST';
    case 'normal':
      return 'MEDIUM';
    case 'slow':
      return 'SLOW';
    case 'free':
      throw new LniError(
        'InvalidInput',
        'Galoy on-chain payments do not support free on-chain fee speed.'
      );
  }
}

function resolveGaloyFeePayer(feePayer?: OnchainFeePayer): OnchainFeePayer {
  if (feePayer === 'recipient') {
    throw new LniError('InvalidInput', 'Galoy on-chain payments only support sender-paid fees.');
  }

  return 'sender';
}

function normalizeOnchainState(state?: string): PayOnchainResponse['state'] {
  switch (state?.toUpperCase()) {
    case 'PENDING':
      return 'pending';
    case 'SUCCESS':
    case 'ALREADY_PAID':
      return 'completed';
    case 'FAILED':
    case 'FAILURE':
      return 'failed';
    default:
      return state?.toLowerCase() ?? 'pending';
  }
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

function galoyTransactionAmountToSats(
  amount: number | undefined,
  currency: string | undefined
): number | undefined {
  return currency === 'BTC' && amount !== undefined ? Math.abs(amount) : undefined;
}

function galoyTransactionMemo(transaction: OnchainTransaction): string | undefined {
  const raw = transaction.raw;
  if (typeof raw !== 'object' || raw === null || Array.isArray(raw)) {
    return undefined;
  }

  const memo = (raw as { memo?: unknown }).memo;
  return typeof memo === 'string' && memo.length > 0 ? memo : undefined;
}

function galoyProviderInfoFromErrors(errors: GraphQLError[]): ProviderErrorInfo {
  const firstActionable = errors.find((error) => mapGaloyCode(error.code) !== undefined);
  const firstCoded = errors.find((error) => error.code !== undefined);
  const selected = firstActionable ?? firstCoded ?? errors[0];
  return {
    code: selected?.code,
    message: errors.map((error) => error.message).join(', '),
  };
}

function mapGaloyCode(code: unknown): NwcErrorCode | undefined {
  const normalized = String(code ?? '').toUpperCase();

  if (normalized.includes('AUTHENTICATION') || normalized.includes('UNAUTHORIZED')) {
    return 'UNAUTHORIZED';
  }
  if (
    normalized.includes('FORBIDDEN') ||
    normalized.includes('PERMISSION') ||
    normalized.includes('SCOPE')
  ) {
    return 'RESTRICTED';
  }
  if (normalized.includes('INSUFFICIENT') || normalized.includes('BALANCE')) {
    return 'INSUFFICIENT_BALANCE';
  }
  if (normalized.includes('LIMIT') || normalized.includes('QUOTA')) {
    return 'QUOTA_EXCEEDED';
  }
  if (
    normalized.includes('INVALID_INVOICE') ||
    normalized.includes('NO_ROUTE') ||
    normalized.includes('PAYMENT')
  ) {
    return 'PAYMENT_FAILED';
  }

  return undefined;
}

function mapGaloyProviderError(info: ProviderErrorInfo): NwcErrorCode | undefined {
  const code = mapGaloyCode(info.code);
  if (code) {
    return code;
  }

  return mapProviderMessage(info.message);
}

function redactSensitiveText(value: string): string {
  return value
    .replace(
      /("(?:apiKey|api_key|x-api-key|authorization|token|paymentRequest|payment_request|paymentHash|payment_hash|paymentSecret|payment_secret|preImage|preimage)"\s*:\s*")[^"]*(")/gi,
      '$1<redacted>$2'
    )
    .replace(/\b(?:lnbc|lntb|lnbcrt|lno|lnr|lni)[a-z0-9]+\b/gi, '<redacted>')
    .replace(/\b[0-9a-f]{64}\b/gi, '<redacted>');
}

class GaloyNodeImplementation implements LightningNode, OnchainPayments {
  private readonly fetchFn;
  private readonly timeoutMs?: number;
  private readonly baseUrl: string;
  private cachedWallet?: GaloyWallet;
  private static readonly MAX_TRANSACTION_FETCH = 1000;
  private static readonly DEFAULT_PAGE_SIZE = 100;

  private static readonly ME_QUERY = `
    query Me {
      me {
        defaultAccount {
          wallets {
            id
            walletCurrency
            balance
          }
        }
      }
    }
  `;

  private static readonly TRANSACTIONS_QUERY = `
    query TransactionsQuery($first: Int, $after: String) {
      me {
        defaultAccount {
          transactions(first: $first, after: $after) {
            edges {
              cursor
              node {
                id
                createdAt
                direction
                status
                memo
                settlementAmount
                settlementCurrency
                settlementFee
                initiationVia {
                  __typename
                  ... on InitiationViaLn {
                    paymentHash
                  }
                }
                settlementVia {
                  __typename
                  ... on SettlementViaLn {
                    preImage
                  }
                }
              }
            }
            pageInfo {
              hasNextPage
              endCursor
            }
          }
        }
      }
    }
  `;

  constructor(
    private readonly config: GaloyConfig,
    options: NodeRequestOptions = {}
  ) {
    this.fetchFn = resolveFetch(options.fetch);
    this.timeoutMs = toTimeoutMs(config.httpTimeout);
    this.baseUrl = config.baseUrl;
  }

  private headers(): Headers {
    const headers = new Headers(this.config.additionalHeaders);
    headers.set('x-api-key', this.config.apiKey);
    headers.set('content-type', 'application/json');
    return headers;
  }

  private sanitizeText(value: string): string {
    const sanitized = redactSensitiveText(value);
    return this.config.apiKey ? sanitized.split(this.config.apiKey).join('<redacted>') : sanitized;
  }

  private nwcError(
    code: NwcErrorCode,
    message: string,
    operation: NwcErrorOperation,
    info?: ProviderErrorInfo
  ): NwcError {
    const safeMessage = this.sanitizeText(message);
    return new NwcError(code, safeMessage, {
      operation,
      provider: this.config.provider.id,
      providerCode: typeof info?.code === 'string' ? this.sanitizeText(info.code) : info?.code,
      providerStatus: info?.status,
      providerMessage: this.sanitizeText(info?.message ?? safeMessage),
    });
  }

  private errorsToNwcError(
    errors: GraphQLError[],
    operation: NwcErrorOperation,
    fallbackCode: NwcErrorCode = 'OTHER'
  ): NwcError {
    const info = galoyProviderInfoFromErrors(errors);
    return this.nwcError(
      mapGaloyProviderError(info) ?? fallbackCode,
      info.message ?? `${this.config.provider.name} GraphQL error`,
      operation,
      info
    );
  }

  private throwProviderError(error: unknown, operation: NwcErrorOperation): never {
    let safeError = error;
    if (error instanceof LniError) {
      safeError = new LniError(error.code, this.sanitizeText(error.message), {
        status: error.status,
        body: error.body ? this.sanitizeText(error.body) : undefined,
      });
    }
    throwNormalizedProviderError(safeError, {
      provider: this.config.provider.id,
      operation,
      mapProviderError: mapGaloyProviderError,
    });
  }

  private async gql<T>(
    query: string,
    variables: Record<string, unknown> | undefined,
    operation: NwcErrorOperation
  ): Promise<T> {
    let payload: GraphQLResponse<T>;
    try {
      payload = await requestJson<GraphQLResponse<T>>(this.fetchFn, this.baseUrl, {
        method: 'POST',
        headers: this.headers(),
        json: {
          query,
          variables,
        },
        timeoutMs: this.timeoutMs,
      });
    } catch (error) {
      this.throwProviderError(error, operation);
    }

    if (payload.errors?.length) {
      throw this.errorsToNwcError(payload.errors, operation);
    }

    if (!payload.data) {
      throw this.nwcError(
        'INTERNAL',
        `No data in ${this.config.provider.name} GraphQL response.`,
        operation
      );
    }

    return payload.data;
  }

  private async resolveWallet(): Promise<GaloyWallet> {
    if (this.cachedWallet) {
      return this.cachedWallet;
    }

    if (this.config.wallet.mode === 'explicit') {
      this.cachedWallet = {
        id: this.config.wallet.id,
        walletCurrency: this.config.wallet.currency,
      };
      return this.cachedWallet;
    }

    const requestedCurrency = this.config.wallet.currency;
    const response = await this.gql<GaloyMeQuery>(
      GaloyNodeImplementation.ME_QUERY,
      undefined,
      'get_info'
    );
    const wallet = response.me.defaultAccount.wallets.find(
      (item) => item.walletCurrency.toUpperCase() === requestedCurrency.toUpperCase()
    );

    if (!wallet) {
      throw this.nwcError(
        'NOT_FOUND',
        `No ${requestedCurrency} wallet found for ${this.config.provider.name}.`,
        'get_info'
      );
    }

    this.cachedWallet = wallet;
    return wallet;
  }

  private notImplemented(message: string, operation: NwcErrorOperation): NwcError {
    return this.nwcError('NOT_IMPLEMENTED', message, operation);
  }

  private assertOnchainEnabled(): void {
    if (!this.config.capabilities.onchain) {
      throw this.notImplemented(
        `On-chain payments are disabled for ${this.config.provider.name}.`,
        'pay_invoice'
      );
    }
  }

  private async resolveBtcOnchainWallet(): Promise<GaloyWallet> {
    this.assertOnchainEnabled();
    const wallet = await this.resolveWallet();
    if (wallet.walletCurrency.toUpperCase() !== 'BTC') {
      throw this.notImplemented(
        `On-chain payments require a BTC wallet for ${this.config.provider.name}.`,
        'pay_invoice'
      );
    }
    return wallet;
  }

  async getPermissions(): Promise<Permissions> {
    if (this.config.permissions === 'configured') {
      return {
        getInfo: true,
        createInvoice: true,
        payInvoice: true,
        createOffer: false,
        getOffer: false,
        listOffers: false,
        payOffer: false,
        lookupInvoice: this.config.capabilities.transactionLookup,
        listTransactions: this.config.capabilities.transactionHistory,
        decode: true,
        onInvoiceEvents: this.config.capabilities.invoiceEvents,
      };
    }

    const permissions = getGaloyTokenPermissions(this.config.apiKey);
    if (!permissions) {
      throw new LniError(
        'InvalidInput',
        `${this.config.provider.name} API keys cannot be introspected. Use a JWT-style token with scopes or configure permissions explicitly.`
      );
    }

    return {
      ...permissions,
      lookupInvoice: permissions.lookupInvoice && this.config.capabilities.transactionLookup,
      listTransactions: permissions.listTransactions && this.config.capabilities.transactionHistory,
      onInvoiceEvents: permissions.onInvoiceEvents && this.config.capabilities.invoiceEvents,
    };
  }

  async getInfo(): Promise<NodeInfo> {
    const wallet = await this.resolveWallet();
    const sats =
      wallet.walletCurrency.toUpperCase() === 'BTC' && wallet.balance !== undefined
        ? wallet.balance
        : 0;

    return emptyNodeInfo({
      alias: `${this.config.provider.name} Node`,
      network: 'mainnet',
      sendBalanceMsat: satsToMsats(sats),
      receiveBalanceMsat: satsToMsats(sats),
    });
  }

  async createInvoice(params: CreateInvoiceParams): Promise<Transaction> {
    if ((params.invoiceType ?? InvoiceType.Bolt11) !== InvoiceType.Bolt11) {
      throw this.notImplemented(
        `Bolt12 is not implemented for ${this.config.provider.name}.`,
        'make_invoice'
      );
    }

    const wallet = await this.resolveWallet();
    const family = invoiceOperationFamily(wallet.walletCurrency);

    const query = `
      mutation ${family.createField}($input: ${family.createInput}!) {
        ${family.createField}(input: $input) {
          invoice {
            paymentRequest
            paymentHash
            satoshis
          }
          errors {
            code
            message
            path
          }
        }
      }
    `;

    const response = await this.gql<GaloyInvoiceCreateResponse>(
      query,
      {
        input: {
          amount: Math.floor((params.amountMsats ?? 0) / 1000),
          walletId: wallet.id,
          memo: params.description,
        },
      },
      'make_invoice'
    );

    const result = response[family.createField];
    if (!result) {
      throw this.nwcError(
        'INTERNAL',
        `No ${family.createField} result returned from ${this.config.provider.name}.`,
        'make_invoice'
      );
    }
    if (result.errors?.length) {
      throw this.errorsToNwcError(result.errors, 'make_invoice');
    }

    const invoice = result.invoice;
    if (!invoice) {
      throw this.nwcError(
        'INTERNAL',
        `No invoice returned from ${this.config.provider.name} invoice creation.`,
        'make_invoice'
      );
    }

    return emptyTransaction({
      type: 'incoming',
      invoice: invoice.paymentRequest,
      paymentHash: invoice.paymentHash,
      amountMsats: satsToMsats(invoice.satoshis),
      createdAt: Math.floor(Date.now() / 1000),
      description: params.description ?? '',
      descriptionHash: params.descriptionHash ?? '',
      payerNote: '',
      externalId: '',
    });
  }

  async payInvoice(params: PayInvoiceParams): Promise<PayInvoiceResponse> {
    const wallet = await this.resolveWallet();
    const family = invoiceOperationFamily(wallet.walletCurrency);

    const feeProbe = await this.gql<GaloyFeeProbeResponse>(
      `
      mutation ${family.feeField}($input: ${family.feeInput}!) {
        ${family.feeField}(input: $input) {
          errors {
            code
            message
            path
          }
          amount
        }
      }
      `,
      {
        input: {
          paymentRequest: params.invoice,
          walletId: wallet.id,
        },
      },
      'pay_invoice'
    );

    const feeResult = feeProbe[family.feeField];
    if (!feeResult) {
      throw this.nwcError(
        'INTERNAL',
        `No ${family.feeField} result returned from ${this.config.provider.name}.`,
        'pay_invoice'
      );
    }
    if (feeResult.errors?.length) {
      throw this.errorsToNwcError(feeResult.errors, 'pay_invoice', 'PAYMENT_FAILED');
    }

    const transactionSelection =
      this.config.payment.response === 'transaction-with-preimage'
        ? `
          transaction {
            settlementVia {
              ... on SettlementViaLn {
                preImage
              }
              ... on SettlementViaIntraLedger {
                preImage
              }
            }
          }
        `
        : '';
    const payment = await this.gql<GaloyPaymentSendResponse>(
      `
      mutation LnInvoicePaymentSend($input: LnInvoicePaymentInput!) {
        lnInvoicePaymentSend(input: $input) {
          status
          errors {
            code
            message
            path
          }
          ${transactionSelection}
        }
      }
      `,
      {
        input: {
          paymentRequest: params.invoice,
          walletId: wallet.id,
        },
      },
      'pay_invoice'
    );

    const status = payment.lnInvoicePaymentSend.status;
    if (!this.config.payment.acceptedStatuses.includes(status)) {
      const providerInfo = payment.lnInvoicePaymentSend.errors?.length
        ? galoyProviderInfoFromErrors(payment.lnInvoicePaymentSend.errors)
        : undefined;
      throw this.nwcError(
        providerInfo
          ? (mapGaloyProviderError(providerInfo) ?? 'PAYMENT_FAILED')
          : status === 'FAILED' || status === 'FAILURE'
            ? 'PAYMENT_FAILED'
            : 'OTHER',
        `${this.config.provider.name} payment was not accepted: ${status}${
          providerInfo?.message ? ` (${providerInfo.message})` : ''
        }`,
        'pay_invoice',
        {
          code: status,
          message: providerInfo?.message ?? status,
        }
      );
    }

    const preimage = payment.lnInvoicePaymentSend.transaction?.settlementVia?.preImage ?? '';
    let paymentHash = '';
    try {
      paymentHash = decodeBolt11(params.invoice).payment_hash ?? '';
    } catch {
      // The payment succeeded, so preserve the provider result even if the invoice cannot decode.
    }
    return {
      paymentHash,
      preimage,
      feeMsats:
        wallet.walletCurrency.toUpperCase() === 'BTC' ? satsToMsats(feeResult.amount ?? 0) : 0,
    };
  }

  async prepareOnchainTransaction(
    params: PrepareOnchainTransactionParams
  ): Promise<OnchainTransaction> {
    this.assertOnchainEnabled();
    const amountSats = params.amountSats;
    assertValidOnchainAmount(amountSats);

    const fee = params.fee ?? defaultOnchainFee();
    const feePayer = resolveGaloyFeePayer(params.feePayer);
    const speed = resolveGaloyFeeSpeed(fee);
    const walletId = (await this.resolveBtcOnchainWallet()).id;

    const response = await this.gql<GaloyOnchainTxFeeResponse>(
      `
      query onChainTxFee($walletId: WalletId!, $address: OnChainAddress!, $amount: SatAmount!, $speed: PayoutSpeed!) {
        onChainTxFee(walletId: $walletId, address: $address, amount: $amount, speed: $speed) {
          amount
        }
      }
      `,
      {
        walletId,
        address: params.address,
        amount: amountSats,
        speed,
      },
      'pay_invoice'
    );

    const feeSats = response.onChainTxFee.amount;

    return {
      address: params.address,
      amountSats,
      feeSats,
      totalAmountSats: feeSats === undefined ? undefined : amountSats + feeSats,
      recipientAmountSats: amountSats,
      feePayer,
      fee,
      raw: {
        walletId,
        speed,
        memo: params.description,
        fee: response.onChainTxFee,
      },
    };
  }

  async payOnchain(
    transaction: OnchainTransaction,
    options?: PayOnchainOptions
  ): Promise<PayOnchainResponse> {
    this.assertOnchainEnabled();
    assertValidOnchainAmount(transaction.amountSats);
    resolveGaloyFeePayer(transaction.feePayer);
    const speed = resolveGaloyFeeSpeed(transaction.fee);
    assertOnchainFeeGuardrail(transaction, options);

    const walletId = (await this.resolveBtcOnchainWallet()).id;
    const memo = galoyTransactionMemo(transaction);
    const payment = await this.gql<GaloyOnchainPaymentSendResponse>(
      `
      mutation onChainPaymentSend($input: OnChainPaymentSendInput!) {
        onChainPaymentSend(input: $input) {
          status
          transaction {
            id
            settlementAmount
            settlementCurrency
            settlementFee
            settlementVia {
              __typename
              ... on SettlementViaOnChain {
                transactionHash
              }
            }
          }
          errors {
            code
            message
            path
          }
        }
      }
      `,
      {
        input: {
          address: transaction.address,
          amount: transaction.amountSats,
          walletId,
          memo,
          speed,
        },
      },
      'pay_invoice'
    );

    if (payment.onChainPaymentSend.errors?.length) {
      throw this.errorsToNwcError(
        payment.onChainPaymentSend.errors,
        'pay_invoice',
        'PAYMENT_FAILED'
      );
    }

    const paymentTransaction = payment.onChainPaymentSend.transaction;
    const feeSats =
      galoyTransactionAmountToSats(
        paymentTransaction?.settlementFee,
        paymentTransaction?.settlementCurrency
      ) ?? transaction.feeSats;
    const amountSats =
      galoyTransactionAmountToSats(
        paymentTransaction?.settlementAmount,
        paymentTransaction?.settlementCurrency
      ) ?? transaction.amountSats;

    return {
      paymentId: paymentTransaction?.id,
      txid: paymentTransaction?.settlementVia?.transactionHash,
      state: normalizeOnchainState(payment.onChainPaymentSend.status),
      address: transaction.address,
      amountSats,
      feeSats,
      totalAmountSats: feeSats === undefined ? transaction.totalAmountSats : amountSats + feeSats,
      recipientAmountSats: transaction.recipientAmountSats ?? transaction.amountSats,
      raw: payment.onChainPaymentSend,
    };
  }

  async createOffer(_params: CreateOfferParams): Promise<Offer> {
    throw this.notImplemented(
      `Bolt12 is not implemented for ${this.config.provider.name}.`,
      'make_invoice'
    );
  }

  async getOffer(_search?: string): Promise<Offer> {
    throw this.notImplemented(
      `Bolt12 is not implemented for ${this.config.provider.name}.`,
      'lookup_invoice'
    );
  }

  async listOffers(_search?: string): Promise<Offer[]> {
    throw this.notImplemented(
      `Bolt12 is not implemented for ${this.config.provider.name}.`,
      'list_transactions'
    );
  }

  async payOffer(
    _offer: string,
    _amountMsats: number,
    _payerNote?: string
  ): Promise<PayInvoiceResponse> {
    throw this.notImplemented(
      `Bolt12 is not implemented for ${this.config.provider.name}.`,
      'pay_invoice'
    );
  }

  private mapTransaction(node: GaloyTransactionNode): Transaction {
    const paymentHash =
      node.initiationVia?.__typename === 'InitiationViaLn'
        ? (node.initiationVia.paymentHash ?? '')
        : '';
    const preimage =
      node.settlementVia?.__typename === 'SettlementViaLn'
        ? (node.settlementVia.preImage ?? '')
        : '';

    const amountMsats =
      node.settlementCurrency === 'BTC' ? satsToMsats(Math.abs(node.settlementAmount ?? 0)) : 0;
    const feeMsats =
      node.settlementCurrency === 'BTC' ? satsToMsats(Math.abs(node.settlementFee ?? 0)) : 0;

    return emptyTransaction({
      type: node.direction === 'SEND' ? 'outgoing' : 'incoming',
      paymentHash,
      preimage,
      amountMsats,
      feesPaid: feeMsats,
      createdAt: node.createdAt,
      settledAt: node.status === 'SUCCESS' ? node.createdAt : 0,
      description: node.memo ?? '',
      descriptionHash: '',
      payerNote: '',
      externalId: node.id,
    });
  }

  private async listTransactionsPage(args: {
    first: number;
    after?: string | null;
    paymentHash?: string;
    search?: string;
  }): Promise<GaloyTransactionsPage> {
    const response: GaloyTransactionsQuery = await this.gql<GaloyTransactionsQuery>(
      GaloyNodeImplementation.TRANSACTIONS_QUERY,
      {
        first: Math.max(args.first, 1),
        after: args.after ?? null,
      },
      args.paymentHash ? 'lookup_invoice' : 'list_transactions'
    );

    const page: GaloyTransactionsQuery['me']['defaultAccount']['transactions'] =
      response.me.defaultAccount.transactions;
    const edges = page.edges;
    const transactions = edges
      .map(({ node }) => this.mapTransaction(node))
      .filter((tx) => {
        if (args.paymentHash && tx.paymentHash !== args.paymentHash) {
          return false;
        }
        return matchesSearch(tx, args.search);
      });

    if (!page.pageInfo.hasNextPage) {
      return {
        transactions,
        nextCursor: null,
      };
    }

    const nextCursor: string | null =
      page.pageInfo.endCursor ?? edges[edges.length - 1]?.cursor ?? null;
    return {
      transactions,
      nextCursor: nextCursor && nextCursor !== args.after ? nextCursor : null,
    };
  }

  async lookupInvoice(params: LookupInvoiceParams): Promise<Transaction> {
    if (!this.config.capabilities.transactionLookup) {
      throw this.notImplemented(
        `Transaction lookup is disabled for ${this.config.provider.name}.`,
        'lookup_invoice'
      );
    }
    if (!params.paymentHash) {
      throw new LniError('InvalidInput', 'lookupInvoice requires paymentHash for a Galoy node.');
    }

    let after: string | null = null;

    while (true) {
      const page = await this.listTransactionsPage({
        first: 100,
        after,
        paymentHash: params.paymentHash,
        search: params.search,
      });

      const match = page.transactions.find((tx) => tx.paymentHash === params.paymentHash);
      if (match) {
        return match;
      }

      if (!page.nextCursor) {
        break;
      }

      after = page.nextCursor;
    }

    throw this.nwcError(
      'NOT_FOUND',
      'Transaction not found for the requested payment hash.',
      'lookup_invoice'
    );
  }

  async listTransactions(params: ListTransactionsParams): Promise<Transaction[]> {
    if (!this.config.capabilities.transactionHistory) {
      throw this.notImplemented(
        `Transaction history is disabled for ${this.config.provider.name}.`,
        'list_transactions'
      );
    }
    const limit =
      params.limit > 0
        ? params.limit
        : Math.min(
            GaloyNodeImplementation.MAX_TRANSACTION_FETCH,
            GaloyNodeImplementation.DEFAULT_PAGE_SIZE * 10
          );
    const from = Math.max(params.from, 0);
    const pageSize = Math.max(Math.min(limit, GaloyNodeImplementation.DEFAULT_PAGE_SIZE), 1);

    let after: string | null = null;
    let skipped = 0;
    const transactions: Transaction[] = [];

    while (transactions.length < limit) {
      const page = await this.listTransactionsPage({
        first: pageSize,
        after,
        paymentHash: params.paymentHash,
        search: params.search,
      });
      if (!page.transactions.length && !page.nextCursor) {
        break;
      }

      for (const tx of page.transactions) {
        if (skipped < from) {
          skipped += 1;
          continue;
        }

        transactions.push(tx);
        if (transactions.length >= limit) {
          break;
        }
      }

      if (!page.nextCursor) {
        break;
      }

      after = page.nextCursor;
    }

    return transactions;
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
    if (!this.config.capabilities.invoiceEvents) {
      throw this.notImplemented(
        `Invoice events are disabled for ${this.config.provider.name}.`,
        'lookup_invoice'
      );
    }
    if (!this.config.capabilities.transactionLookup) {
      throw this.notImplemented(
        `Invoice events require transaction lookup for ${this.config.provider.name}.`,
        'lookup_invoice'
      );
    }
    await pollInvoiceEvents({
      params,
      callback,
      lookup: () => this.lookupInvoice({ paymentHash: params.paymentHash, search: params.search }),
    });
  }
}

export type GaloyNode = LightningNode & OnchainPayments;

export function createGaloyNode(config: GaloyConfig, options: NodeRequestOptions = {}): GaloyNode {
  return new GaloyNodeImplementation(config, options);
}
