import { LniError, NwcError, type NwcErrorCode, type NwcErrorOperation } from '../errors.js';
import { decodeBolt11ToJson, decodeOfferToJson } from '../decode.js';
import { mapProviderMessage, throwNormalizedProviderError, type ProviderErrorInfo } from '../internal/error-normalization.js';
import { requestJson, resolveFetch, toTimeoutMs } from '../internal/http.js';
import { getBlinkTokenPermissions } from '../internal/permissions.js';
import { pollInvoiceEvents } from '../internal/polling.js';
import { emptyNodeInfo, emptyTransaction, matchesSearch, satsToMsats } from '../internal/transform.js';
import { DEFAULT_ONCHAIN_FEE_GUARDRAIL, InvoiceType, type BlinkConfig, type CreateInvoiceParams, type CreateOfferParams, type InvoiceEventCallback, type LightningNode, type ListTransactionsParams, type LookupInvoiceParams, type NodeInfo, type NodeRequestOptions, type Offer, type OnInvoiceEventParams, type OnchainFeeGuardrail, type OnchainFeePayer, type OnchainFeePreference, type OnchainPayments, type OnchainTransaction, type PayInvoiceParams, type PayInvoiceResponse, type PayOnchainOptions, type PayOnchainResponse, type Permissions, type PrepareOnchainTransactionParams, type Transaction } from '../types.js';

interface GraphQLError {
  message: string;
  code?: string;
  path?: string[];
}

interface GraphQLResponse<T> {
  data?: T;
  errors?: GraphQLError[];
}

interface BlinkMeQuery {
  me: {
    defaultAccount: {
      wallets: BlinkWallet[];
    };
  };
}

interface BlinkWallet {
  id: string;
  walletCurrency: string;
  balance: number;
}

interface BlinkInvoiceCreateResponse {
  lnInvoiceCreate: {
    invoice?: {
      paymentRequest: string;
      paymentHash: string;
      satoshis: number;
    };
    errors?: GraphQLError[];
  };
}

interface BlinkFeeProbeResponse {
  lnInvoiceFeeProbe: {
    amount?: number;
    errors?: GraphQLError[];
  };
}

interface BlinkPaymentSendResponse {
  lnInvoicePaymentSend: {
    status: string;
    errors?: GraphQLError[];
  };
}

interface BlinkOnchainTxFeeResponse {
  onChainTxFee: {
    amount?: number;
  };
}

interface BlinkOnchainPaymentSendResponse {
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

interface BlinkTransactionsQuery {
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

type BlinkTransactionNode = BlinkTransactionsQuery['me']['defaultAccount']['transactions']['edges'][number]['node'];

interface BlinkTransactionsPage {
  transactions: Transaction[];
  nextCursor: string | null;
}

function defaultOnchainFee(): OnchainFeePreference {
  return { type: 'default' };
}

function resolveBlinkFeeSpeed(fee: OnchainFeePreference): 'FAST' | 'MEDIUM' | 'SLOW' {
  if (fee.type === 'default') {
    return 'FAST';
  }

  if (fee.type !== 'speed') {
    throw new LniError('InvalidInput', `Blink payOnchain does not support ${fee.type} fee preferences.`);
  }

  switch (fee.speed) {
    case 'fast':
      return 'FAST';
    case 'normal':
      return 'MEDIUM';
    case 'slow':
      return 'SLOW';
    case 'free':
      throw new LniError('InvalidInput', 'Blink payOnchain does not support free on-chain fee speed.');
  }
}

function resolveBlinkFeePayer(feePayer?: OnchainFeePayer): OnchainFeePayer {
  if (feePayer === 'recipient') {
    throw new LniError('InvalidInput', 'Blink payOnchain only supports sender-paid on-chain fees.');
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

function blinkTransactionAmountToSats(amount: number | undefined, currency: string | undefined): number | undefined {
  return currency === 'BTC' && amount !== undefined ? Math.abs(amount) : undefined;
}

function blinkTransactionMemo(transaction: OnchainTransaction): string | undefined {
  const raw = transaction.raw;
  if (typeof raw !== 'object' || raw === null || Array.isArray(raw)) {
    return undefined;
  }

  const memo = (raw as { memo?: unknown }).memo;
  return typeof memo === 'string' && memo.length > 0 ? memo : undefined;
}

function blinkProviderInfoFromErrors(errors: GraphQLError[]): ProviderErrorInfo {
  const firstActionable = errors.find((error) => mapBlinkCode(error.code) !== undefined);
  const firstCoded = errors.find((error) => error.code !== undefined);
  const selected = firstActionable ?? firstCoded ?? errors[0];
  return {
    code: selected?.code,
    message: errors.map((error) => error.message).join(', '),
  };
}

function mapBlinkCode(code: unknown): NwcErrorCode | undefined {
  const normalized = String(code ?? '').toUpperCase();

  if (normalized.includes('AUTHENTICATION') || normalized.includes('UNAUTHORIZED')) {
    return 'UNAUTHORIZED';
  }
  if (normalized.includes('FORBIDDEN') || normalized.includes('PERMISSION') || normalized.includes('SCOPE')) {
    return 'RESTRICTED';
  }
  if (normalized.includes('INSUFFICIENT') || normalized.includes('BALANCE')) {
    return 'INSUFFICIENT_BALANCE';
  }
  if (normalized.includes('LIMIT') || normalized.includes('QUOTA')) {
    return 'QUOTA_EXCEEDED';
  }
  if (normalized.includes('INVALID_INVOICE') || normalized.includes('NO_ROUTE') || normalized.includes('PAYMENT')) {
    return 'PAYMENT_FAILED';
  }

  return undefined;
}

function mapBlinkProviderError(info: ProviderErrorInfo): NwcErrorCode | undefined {
  const code = mapBlinkCode(info.code);
  if (code) {
    return code;
  }

  return mapProviderMessage(info.message);
}

function blinkNwcError(
  code: NwcErrorCode,
  message: string,
  operation: NwcErrorOperation,
  info?: ProviderErrorInfo,
): NwcError {
  return new NwcError(code, message, {
    operation,
    provider: 'blink',
    providerCode: info?.code,
    providerStatus: info?.status,
    providerMessage: info?.message ?? message,
  });
}

function blinkErrorsToNwcError(errors: GraphQLError[], operation: NwcErrorOperation, fallbackCode: NwcErrorCode = 'OTHER'): NwcError {
  const info = blinkProviderInfoFromErrors(errors);
  return blinkNwcError(mapBlinkProviderError(info) ?? fallbackCode, info.message ?? 'Blink GraphQL error', operation, info);
}

function throwBlinkProviderError(error: unknown, operation: NwcErrorOperation): never {
  throwNormalizedProviderError(error, {
    provider: 'blink',
    operation,
    mapProviderError: mapBlinkProviderError,
  });
}

export class BlinkNode implements LightningNode, OnchainPayments {
  private readonly fetchFn;
  private readonly timeoutMs?: number;
  private readonly baseUrl: string;
  private cachedWalletId?: string;
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

  constructor(private readonly config: BlinkConfig, options: NodeRequestOptions = {}) {
    this.fetchFn = resolveFetch(options.fetch);
    this.timeoutMs = toTimeoutMs(config.httpTimeout);
    this.baseUrl = config.baseUrl ?? 'https://api.blink.sv/graphql';
  }

  private headers(extra?: HeadersInit): HeadersInit {
    return {
      'x-api-key': this.config.apiKey,
      'content-type': 'application/json',
      ...(extra ?? {}),
    };
  }

  private async gql<T>(
    query: string,
    variables: Record<string, unknown> | undefined,
    operation: NwcErrorOperation,
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
      throwBlinkProviderError(error, operation);
    }

    if (payload.errors?.length) {
      throw blinkErrorsToNwcError(payload.errors, operation);
    }

    if (!payload.data) {
      throw blinkNwcError('INTERNAL', 'No data in Blink GraphQL response.', operation);
    }

    return payload.data;
  }

  private async getBtcWallet(): Promise<BlinkWallet> {
    const response = await this.gql<BlinkMeQuery>(BlinkNode.ME_QUERY, undefined, 'get_info');
    const wallet = response.me.defaultAccount.wallets.find((item) => item.walletCurrency === 'BTC');

    if (!wallet) {
      throw blinkNwcError('NOT_FOUND', 'No BTC wallet found in Blink account.', 'get_info');
    }

    this.cachedWalletId = wallet.id;
    return wallet;
  }

  private async getBtcWalletId(): Promise<string> {
    if (this.cachedWalletId) {
      return this.cachedWalletId;
    }

    const wallet = await this.getBtcWallet();
    this.cachedWalletId = wallet.id;
    return wallet.id;
  }

  async getPermissions(): Promise<Permissions> {
    const permissions = getBlinkTokenPermissions(this.config.apiKey);
    if (!permissions) {
      throw new LniError(
        'InvalidInput',
        'Blink API keys cannot be introspected. Use a JWT-style token with scopes or manually test permissions against Blink GraphQL operations.',
      );
    }

    return permissions;
  }

  async getInfo(): Promise<NodeInfo> {
    const wallet = await this.getBtcWallet();
    const sats = wallet.balance;

    return emptyNodeInfo({
      alias: 'Blink Node',
      network: 'mainnet',
      sendBalanceMsat: satsToMsats(sats),
      receiveBalanceMsat: satsToMsats(sats),
    });
  }

  async createInvoice(params: CreateInvoiceParams): Promise<Transaction> {
    if ((params.invoiceType ?? InvoiceType.Bolt11) !== InvoiceType.Bolt11) {
      throw blinkNwcError('NOT_IMPLEMENTED', 'Bolt12 is not implemented for BlinkNode.', 'make_invoice');
    }

    const walletId = await this.getBtcWalletId();

    const query = `
      mutation LnInvoiceCreate($input: LnInvoiceCreateInput!) {
        lnInvoiceCreate(input: $input) {
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

    const response = await this.gql<BlinkInvoiceCreateResponse>(query, {
      input: {
        amount: Math.floor((params.amountMsats ?? 0) / 1000),
        walletId,
        memo: params.description,
      },
    }, 'make_invoice');

    if (response.lnInvoiceCreate.errors?.length) {
      throw blinkErrorsToNwcError(response.lnInvoiceCreate.errors, 'make_invoice');
    }

    const invoice = response.lnInvoiceCreate.invoice;
    if (!invoice) {
      throw blinkNwcError('INTERNAL', 'No invoice returned from Blink invoice creation.', 'make_invoice');
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
    const walletId = await this.getBtcWalletId();

    const feeProbe = await this.gql<BlinkFeeProbeResponse>(
      `
      mutation lnInvoiceFeeProbe($input: LnInvoiceFeeProbeInput!) {
        lnInvoiceFeeProbe(input: $input) {
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
          walletId,
        },
      },
      'pay_invoice',
    );

    if (feeProbe.lnInvoiceFeeProbe.errors?.length) {
      throw blinkErrorsToNwcError(feeProbe.lnInvoiceFeeProbe.errors, 'pay_invoice', 'PAYMENT_FAILED');
    }

    const payment = await this.gql<BlinkPaymentSendResponse>(
      `
      mutation LnInvoicePaymentSend($input: LnInvoicePaymentInput!) {
        lnInvoicePaymentSend(input: $input) {
          status
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
          paymentRequest: params.invoice,
          walletId,
        },
      },
      'pay_invoice',
    );

    if (payment.lnInvoicePaymentSend.errors?.length) {
      throw blinkErrorsToNwcError(payment.lnInvoicePaymentSend.errors, 'pay_invoice', 'PAYMENT_FAILED');
    }

    if (payment.lnInvoicePaymentSend.status !== 'SUCCESS') {
      const status = payment.lnInvoicePaymentSend.status;
      throw blinkNwcError(
        status === 'FAILED' ? 'PAYMENT_FAILED' : 'OTHER',
        `Blink payment failed with status ${status}`,
        'pay_invoice',
        { code: status, message: status },
      );
    }

    return {
      paymentHash: '',
      preimage: '',
      feeMsats: satsToMsats(feeProbe.lnInvoiceFeeProbe.amount ?? 0),
    };
  }

  async prepareOnchainTransaction(params: PrepareOnchainTransactionParams): Promise<OnchainTransaction> {
    const amountSats = params.amountSats;
    assertValidOnchainAmount(amountSats);

    const fee = params.fee ?? defaultOnchainFee();
    const feePayer = resolveBlinkFeePayer(params.feePayer);
    const speed = resolveBlinkFeeSpeed(fee);
    const walletId = await this.getBtcWalletId();

    const response = await this.gql<BlinkOnchainTxFeeResponse>(
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
      'pay_invoice',
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

  async payOnchain(transaction: OnchainTransaction, options?: PayOnchainOptions): Promise<PayOnchainResponse> {
    assertValidOnchainAmount(transaction.amountSats);
    resolveBlinkFeePayer(transaction.feePayer);
    const speed = resolveBlinkFeeSpeed(transaction.fee);
    assertOnchainFeeGuardrail(transaction, options);

    const walletId = await this.getBtcWalletId();
    const memo = blinkTransactionMemo(transaction);
    const payment = await this.gql<BlinkOnchainPaymentSendResponse>(
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
      'pay_invoice',
    );

    if (payment.onChainPaymentSend.errors?.length) {
      throw blinkErrorsToNwcError(payment.onChainPaymentSend.errors, 'pay_invoice', 'PAYMENT_FAILED');
    }

    const paymentTransaction = payment.onChainPaymentSend.transaction;
    const feeSats =
      blinkTransactionAmountToSats(paymentTransaction?.settlementFee, paymentTransaction?.settlementCurrency)
        ?? transaction.feeSats;
    const amountSats =
      blinkTransactionAmountToSats(paymentTransaction?.settlementAmount, paymentTransaction?.settlementCurrency)
        ?? transaction.amountSats;

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
    throw blinkNwcError('NOT_IMPLEMENTED', 'Bolt12 is not implemented for BlinkNode.', 'make_invoice');
  }

  async getOffer(_search?: string): Promise<Offer> {
    throw blinkNwcError('NOT_IMPLEMENTED', 'Bolt12 is not implemented for BlinkNode.', 'lookup_invoice');
  }

  async listOffers(_search?: string): Promise<Offer[]> {
    throw blinkNwcError('NOT_IMPLEMENTED', 'Bolt12 is not implemented for BlinkNode.', 'list_transactions');
  }

  async payOffer(_offer: string, _amountMsats: number, _payerNote?: string): Promise<PayInvoiceResponse> {
    throw blinkNwcError('NOT_IMPLEMENTED', 'Bolt12 is not implemented for BlinkNode.', 'pay_invoice');
  }

  private mapTransaction(node: BlinkTransactionNode): Transaction {
    const paymentHash =
      node.initiationVia?.__typename === 'InitiationViaLn'
        ? (node.initiationVia.paymentHash ?? '')
        : '';
    const preimage =
      node.settlementVia?.__typename === 'SettlementViaLn' ? (node.settlementVia.preImage ?? '') : '';

    const amountMsats = node.settlementCurrency === 'BTC' ? satsToMsats(Math.abs(node.settlementAmount ?? 0)) : 0;
    const feeMsats = node.settlementCurrency === 'BTC' ? satsToMsats(Math.abs(node.settlementFee ?? 0)) : 0;

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
  }): Promise<BlinkTransactionsPage> {
    const response: BlinkTransactionsQuery = await this.gql<BlinkTransactionsQuery>(BlinkNode.TRANSACTIONS_QUERY, {
      first: Math.max(args.first, 1),
      after: args.after ?? null,
    }, args.paymentHash ? 'lookup_invoice' : 'list_transactions');

    const page: BlinkTransactionsQuery['me']['defaultAccount']['transactions'] =
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

    const nextCursor: string | null = page.pageInfo.endCursor ?? edges[edges.length - 1]?.cursor ?? null;
    return {
      transactions,
      nextCursor: nextCursor && nextCursor !== args.after ? nextCursor : null,
    };
  }

  async lookupInvoice(params: LookupInvoiceParams): Promise<Transaction> {
    if (!params.paymentHash) {
      throw new LniError('InvalidInput', 'lookupInvoice requires paymentHash for BlinkNode.');
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

    throw blinkNwcError('NOT_FOUND', `Transaction not found for payment hash: ${params.paymentHash}`, 'lookup_invoice');
  }

  async listTransactions(params: ListTransactionsParams): Promise<Transaction[]> {
    const limit =
      params.limit > 0
        ? params.limit
        : Math.min(BlinkNode.MAX_TRANSACTION_FETCH, BlinkNode.DEFAULT_PAGE_SIZE * 10);
    const from = Math.max(params.from, 0);
    const pageSize = Math.max(Math.min(limit, BlinkNode.DEFAULT_PAGE_SIZE), 1);

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

  async onInvoiceEvents(params: OnInvoiceEventParams, callback: InvoiceEventCallback): Promise<void> {
    await pollInvoiceEvents({
      params,
      callback,
      lookup: () => this.lookupInvoice({ paymentHash: params.paymentHash, search: params.search }),
    });
  }
}
