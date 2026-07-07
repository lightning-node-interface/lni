import { NWCClient, Nip47Error, type Nip47GetBalanceResponse, type Nip47GetInfoResponse, type Nip47ListTransactionsResponse, type Nip47Method, type Nip47PayResponse, type Nip47Transaction } from '@getalby/sdk/nwc';
import { decode as decodeBolt11, decodeBolt11ToJson, decodeOfferToJson } from '../decode.js';
import { LniError, NwcError, type NwcErrorOperation } from '../errors.js';
import { hexToBytes } from '../internal/encoding.js';
import { toTimeoutMs } from '../internal/http.js';
import { NWC_METHOD_PERMISSIONS, normalizeNwcPermissions } from '../internal/permissions.js';
import { pollInvoiceEvents } from '../internal/polling.js';
import { sha256Hex } from '../internal/sha256.js';
import { emptyNodeInfo, emptyTransaction, matchesSearch, parseOptionalNumber } from '../internal/transform.js';
import { LnurlVerifyUnsupportedError, verifyLightningAddressPayRequest } from '../lnurl.js';
import type { CreateInvoiceParams, CreateOfferParams, InvoiceEventCallback, LightningAddressInfo, LightningNode, ListTransactionsParams, LookupInvoiceParams, NodeInfo, NodeRequestOptions, NwcConfig, Offer, OnInvoiceEventParams, PayInvoiceParams, PayInvoiceResponse, Permissions, RequestCancellationOptions, Transaction } from '../types.js';

const DEFAULT_NWC_TIMEOUT_MS = 60_000;
const NWC_PAYMENT_LOOKUP_INITIAL_DELAY_MS = 3_000;
const NWC_PAYMENT_LOOKUP_POLL_INTERVAL_MS = 3_000;

type NwcListTransaction = Partial<Omit<Nip47Transaction, 'type' | 'payment_hash'>> & {
  type?: Nip47Transaction['type'];
  payment_hash?: string;
};

type NwcListTransactionsResponse = Omit<Nip47ListTransactionsResponse, 'transactions' | 'total_count'> & {
  transactions: NwcListTransaction[];
  total_count?: number;
};

type ActiveNwcRequest = {
  cancel(reason: LniError): void;
};

function toNwcError(error: unknown, operation: NwcErrorOperation): NwcError | undefined {
  if (error instanceof NwcError) {
    return error;
  }

  if (error instanceof Nip47Error) {
    return new NwcError(error.code, error.message, {
      operation,
      cause: error,
    });
  }

  return undefined;
}

function throwNwcOrApiError(error: unknown, operation: NwcErrorOperation, fallbackPrefix: string): never {
  const nwcError = toNwcError(error, operation);
  if (nwcError) {
    throw nwcError;
  }

  if (error instanceof LniError) {
    throw new LniError(error.code, `${fallbackPrefix}: ${error.message}`, {
      status: error.status,
      body: error.body,
      cause: error,
    });
  }

  throw new LniError('Api', `${fallbackPrefix}: ${(error as Error)?.message ?? 'unknown error'}`, { cause: error });
}

function extractPubkeyFromNwcUri(uri: string): string {
  try {
    const parsed = NWCClient.parseWalletConnectUrl(uri);
    return parsed.walletPubkey ?? '';
  } catch {
    // ignore
  }

  const withoutParams = uri.split('?')[0] ?? '';
  if (withoutParams.startsWith('nostr+walletconnect://')) {
    return withoutParams.replace('nostr+walletconnect://', '');
  }

  return '';
}

function extractLightningAddressFromNwcUri(uri: string): string {
  try {
    const parsed = NWCClient.parseWalletConnectUrl(uri);
    if (parsed.lud16?.trim()) {
      return parsed.lud16.trim();
    }
  } catch {
    // fall back to URL parsing below
  }

  try {
    const normalized = uri
      .replace('nostrwalletconnect://', 'http://')
      .replace('nostr+walletconnect://', 'http://')
      .replace('nostrwalletconnect:', 'http://')
      .replace('nostr+walletconnect:', 'http://');
    return new URL(normalized).searchParams.get('lud16')?.trim() ?? '';
  } catch {
    return '';
  }
}

function paymentHashFromInvoice(invoice: string): string {
  if (!invoice) {
    return '';
  }

  try {
    const decoded = decodeBolt11(invoice);
    return decoded.payment_hash ?? '';
  } catch {
    return '';
  }
}

function nwcTransactionToLniTransaction(tx: Nip47Transaction | NwcListTransaction): Transaction {
  const invoice = tx.invoice ?? '';

  return emptyTransaction({
    type: tx.type === 'outgoing' ? 'outgoing' : 'incoming',
    invoice,
    description: tx.description ?? '',
    descriptionHash: tx.description_hash ?? '',
    preimage: tx.preimage ?? '',
    paymentHash: tx.payment_hash ?? paymentHashFromInvoice(invoice),
    amountMsats: parseOptionalNumber(tx.amount),
    feesPaid: parseOptionalNumber(tx.fees_paid),
    createdAt: parseOptionalNumber(tx.created_at),
    expiresAt: parseOptionalNumber(tx.expires_at),
    settledAt: parseOptionalNumber(tx.settled_at),
    payerNote: '',
    externalId: '',
  });
}

async function bufferGetAlbyLookupInvoiceErrors<T>(
  fn: () => Promise<T>,
  errorLogs: unknown[][],
): Promise<T> {
  const originalError = console.error;

  console.error = (...args: unknown[]) => {
    if (args[0] === 'Failed to request lookup_invoice') {
      errorLogs.push(args);
      return;
    }

    originalError(...args);
  };

  try {
    return await fn();
  } finally {
    console.error = originalError;
  }
}

function replayConsoleErrors(errorLogs: unknown[][]): void {
  for (const args of errorLogs) {
    console.error(...args);
  }
}

function resolveNwcTimeoutMs(timeoutSeconds: number | undefined): number | undefined {
  if (timeoutSeconds === undefined) {
    return DEFAULT_NWC_TIMEOUT_MS;
  }

  return toTimeoutMs(timeoutSeconds);
}

export class NwcNode implements LightningNode {
  private readonly client: NWCClient;
  private readonly timeoutMs: number | undefined;
  private readonly activeRequests = new Set<ActiveNwcRequest>();
  private closed = false;

  constructor(
    private readonly config: NwcConfig,
    private readonly options: NodeRequestOptions = {},
  ) {
    this.timeoutMs = resolveNwcTimeoutMs(config.httpTimeout);
    this.client = new NWCClient({
      nostrWalletConnectUrl: config.nwcUri,
    });
  }

  close(): void {
    if (this.closed) {
      return;
    }

    this.closed = true;
    this.cancelActiveRequests('NWC request canceled because the node was closed.');
    this.client.close();
  }

  async getPermissions(): Promise<Permissions> {
    const info = await this.executeNip47Request(
      'get permissions',
      'get_info',
      {},
      (result: Nip47GetInfoResponse) => Boolean(result.methods),
      () => this.client.getInfo(),
    ).catch((error) => {
      throwNwcOrApiError(error, 'get_info', 'Failed to get NWC permissions');
    });
    const methods = (info as Nip47GetInfoResponse & { methods?: string[] }).methods;
    return normalizeNwcPermissions(methods?.length ? methods : NWC_METHOD_PERMISSIONS);
  }

  async getLightningAddress(): Promise<LightningAddressInfo> {
    const lightningAddress = extractLightningAddressFromNwcUri(this.config.nwcUri);
    if (!lightningAddress) {
      throw new LniError('InvalidInput', 'NWC URI does not include a lud16 Lightning Address.');
    }

    let lnurlVerifySupported = false;
    try {
      await verifyLightningAddressPayRequest(lightningAddress, {
        fetch: this.options.fetch,
      });
      lnurlVerifySupported = true;
    } catch (error) {
      if (!(error instanceof LnurlVerifyUnsupportedError)) {
        throw error;
      }
    }

    return {
      lightningAddress,
      lnurlVerifySupported,
    };
  }

  async getInfo(): Promise<NodeInfo> {
    const balance = await this.executeNip47Request(
      'get balance',
      'get_balance',
      {},
      (result: Nip47GetBalanceResponse) => result.balance !== undefined,
      () => this.client.getBalance(),
    ).catch((error) => {
      throwNwcOrApiError(error, 'get_balance', 'Failed to get balance');
    });

    const pubkeyFallback = extractPubkeyFromNwcUri(this.config.nwcUri);

    try {
      const info = await this.executeNip47Request(
        'get info',
        'get_info',
        {},
        (result: Nip47GetInfoResponse) => Boolean(result.methods),
        () => this.client.getInfo(),
      );
      return this.mapInfoWithBalance(info, balance, pubkeyFallback);
    } catch {
      return emptyNodeInfo({
        alias: 'NWC Node',
        pubkey: pubkeyFallback,
        network: 'mainnet',
        sendBalanceMsat: parseOptionalNumber(balance.balance),
      });
    }
  }

  private mapInfoWithBalance(
    info: Nip47GetInfoResponse,
    balance: Nip47GetBalanceResponse,
    pubkeyFallback: string,
  ): NodeInfo {
    return emptyNodeInfo({
      alias: info.alias ?? 'NWC Node',
      color: info.color ?? '',
      pubkey: info.pubkey ?? pubkeyFallback,
      network: info.network ?? 'mainnet',
      blockHeight: parseOptionalNumber(info.block_height),
      blockHash: info.block_hash ?? '',
      sendBalanceMsat: parseOptionalNumber(balance.balance),
    });
  }

  async createInvoice(params: CreateInvoiceParams): Promise<Transaction> {
    const request = {
      amount: params.amountMsats ?? 0,
      description: params.description,
      description_hash: params.descriptionHash,
      expiry: params.expiry,
    };
    const tx = await this.executeNip47Request(
      'create invoice',
      'make_invoice',
      request,
      (result: Nip47Transaction) => Boolean(result.invoice),
      () => this.client.makeInvoice(request),
    ).catch((error) => {
      throwNwcOrApiError(error, 'make_invoice', 'Failed to create invoice');
    });

    return nwcTransactionToLniTransaction(tx);
  }

  async payInvoice(params: PayInvoiceParams, options: RequestCancellationOptions = {}): Promise<PayInvoiceResponse> {
    const request = {
      invoice: params.invoice,
      amount: params.amountMsats,
    };
    const response = await this.payInvoiceWithLookupFallback(request, options).catch((error) => {
      throwNwcOrApiError(error, 'pay_invoice', 'Failed to pay invoice');
    });

    return this.mapPayInvoiceResponse(response);
  }

  private async mapPayInvoiceResponse(response: Nip47PayResponse): Promise<PayInvoiceResponse> {
    let paymentHash = '';
    if (response.preimage) {
      let preimageBytes: Uint8Array;
      try {
        preimageBytes = hexToBytes(response.preimage);
      } catch (error) {
        throw new LniError('InvalidInput', `Invalid preimage hex: ${(error as Error).message}`);
      }

      paymentHash = await sha256Hex(preimageBytes);
    }

    return {
      paymentHash,
      preimage: response.preimage,
      feeMsats: parseOptionalNumber(response.fees_paid),
    };
  }

  private async payInvoiceWithLookupFallback(
    request: { invoice: string; amount?: number },
    options: RequestCancellationOptions,
  ): Promise<Nip47PayResponse> {
    const directController = new AbortController();
    const pollController = new AbortController();
    const abortInternalRequests = () => {
      directController.abort();
      pollController.abort();
    };

    if (options.signal?.aborted) {
      abortInternalRequests();
    } else {
      options.signal?.addEventListener('abort', abortInternalRequests, { once: true });
    }

    const directRequest = this.executeNip47Request(
      'pay invoice',
      'pay_invoice',
      request,
      (result: Nip47PayResponse) => Boolean(result),
      () => this.client.payInvoice(request),
      { signal: directController.signal },
    );
    void directRequest.catch(() => undefined);

    const paymentHash = paymentHashFromInvoice(request.invoice);
    if (!paymentHash) {
      try {
        return await directRequest;
      } finally {
        options.signal?.removeEventListener('abort', abortInternalRequests);
      }
    }

    const lookupFallback = this.pollPaidInvoiceResult(
      paymentHash,
      request.invoice,
      pollController.signal,
      this.timeoutMs ?? DEFAULT_NWC_TIMEOUT_MS,
    );
    void lookupFallback.catch(() => undefined);

    try {
      return await Promise.race([directRequest, lookupFallback]);
    } finally {
      abortInternalRequests();
      options.signal?.removeEventListener('abort', abortInternalRequests);
    }
  }

  private async pollPaidInvoiceResult(
    paymentHash: string,
    invoice: string,
    signal: AbortSignal,
    timeoutMs: number,
  ): Promise<Nip47PayResponse> {
    const deadlineAt = Date.now() + timeoutMs;
    await this.delay(
      Math.min(NWC_PAYMENT_LOOKUP_INITIAL_DELAY_MS, this.remainingLookupTimeoutMs(deadlineAt, timeoutMs)),
      signal,
      'pay invoice lookup fallback',
    );

    while (!signal.aborted) {
      try {
        const lookup = this.lookupInvoiceWithOptions(
          {
            paymentHash,
            search: invoice,
          },
          { signal },
          { suppressBufferedLookupErrors: true },
        );
        void lookup.catch(() => undefined);
        const tx = await this.withLookupDeadline(
          lookup,
          this.remainingLookupTimeoutMs(deadlineAt, timeoutMs),
          timeoutMs,
          signal,
        );

        if (tx.type === 'outgoing' && tx.paymentHash === paymentHash && tx.settledAt > 0 && tx.preimage) {
          return {
            preimage: tx.preimage,
            fees_paid: tx.feesPaid,
          };
        }
      } catch (error) {
        if (this.isLookupTimeoutError(error)) {
          throw error;
        }

        if (signal.aborted || (error instanceof LniError && error.code === 'Canceled')) {
          throw error;
        }
      }

      await this.delay(
        Math.min(NWC_PAYMENT_LOOKUP_POLL_INTERVAL_MS, this.remainingLookupTimeoutMs(deadlineAt, timeoutMs)),
        signal,
        'pay invoice lookup fallback',
      );
    }

    throw this.createCancellationError('pay invoice lookup fallback', 'canceled because the request was aborted.');
  }

  private remainingLookupTimeoutMs(deadlineAt: number, timeoutMs: number): number {
    const remainingMs = deadlineAt - Date.now();
    if (remainingMs <= 0) {
      throw this.createPaymentLookupTimeoutError(timeoutMs);
    }

    return remainingMs;
  }

  private async withLookupDeadline<T>(
    promise: Promise<T>,
    timeoutMs: number,
    totalTimeoutMs: number,
    signal: AbortSignal,
  ): Promise<T> {
    let timeoutId: ReturnType<typeof setTimeout> | undefined;
    let onAbort: (() => void) | undefined;
    const timeout = new Promise<never>((_resolve, reject) => {
      timeoutId = setTimeout(() => {
        reject(this.createPaymentLookupTimeoutError(totalTimeoutMs));
      }, timeoutMs);
      onAbort = () => {
        reject(this.createCancellationError('pay invoice lookup fallback', 'canceled because the request was aborted.'));
      };
      signal.addEventListener('abort', onAbort, { once: true });
    });

    try {
      return await Promise.race([promise, timeout]);
    } finally {
      if (timeoutId) {
        clearTimeout(timeoutId);
      }
      if (onAbort) {
        signal.removeEventListener('abort', onAbort);
      }
    }
  }

  private createPaymentLookupTimeoutError(timeoutMs: number): LniError {
    return new LniError(
      'NetworkError',
      `NWC pay invoice lookup fallback timed out after ${timeoutMs / 1000}s. Check that the wallet exposes a settled outgoing transaction with preimage via lookup_invoice or list_transactions.`,
    );
  }

  private isLookupTimeoutError(error: unknown): boolean {
    return (
      error instanceof LniError &&
      error.code === 'NetworkError' &&
      error.message.startsWith('NWC pay invoice lookup fallback timed out')
    );
  }

  private async delay(ms: number, signal: AbortSignal, operation: string): Promise<void> {
    if (signal.aborted) {
      throw this.createCancellationError(operation, 'canceled because the request was aborted.');
    }

    await new Promise<void>((resolve, reject) => {
      const timeoutId = setTimeout(() => {
        signal.removeEventListener('abort', onAbort);
        resolve();
      }, ms);
      const onAbort = () => {
        clearTimeout(timeoutId);
        reject(this.createCancellationError(operation, 'canceled because the request was aborted.'));
      };
      signal.addEventListener('abort', onAbort, { once: true });
    });
  }

  async createOffer(_params: CreateOfferParams): Promise<Offer> {
    throw new LniError('Api', 'NWC does not support offers (BOLT12) yet.');
  }

  async getOffer(_search?: string): Promise<Offer> {
    throw new LniError('Api', 'NWC does not support offers (BOLT12) yet.');
  }

  async listOffers(_search?: string): Promise<Offer[]> {
    throw new LniError('Api', 'NWC does not support offers (BOLT12) yet.');
  }

  async payOffer(_offer: string, _amountMsats: number, _payerNote?: string): Promise<PayInvoiceResponse> {
    throw new LniError('Api', 'NWC does not support offers (BOLT12) yet.');
  }

  async lookupInvoice(params: LookupInvoiceParams): Promise<Transaction> {
    return this.lookupInvoiceWithOptions(params);
  }

  private async lookupInvoiceWithOptions(
    params: LookupInvoiceParams,
    options: RequestCancellationOptions = {},
    lookupOptions: { suppressBufferedLookupErrors?: boolean } = {},
  ): Promise<Transaction> {
    const paymentHash = params.paymentHash ?? paymentHashFromInvoice(params.search ?? '');
    const invoice = params.search;

    if (!paymentHash && !invoice) {
      throw new LniError('InvalidInput', 'lookupInvoice requires paymentHash or search (invoice) for NwcNode.');
    }

    const errorLogs: unknown[][] = [];
    try {
      const tx = await bufferGetAlbyLookupInvoiceErrors(
        () =>
          this.executeNip47Request(
            'lookup invoice',
            'lookup_invoice',
            {
              payment_hash: paymentHash || undefined,
              invoice: paymentHash ? undefined : invoice,
            },
            (result: Nip47Transaction) => Boolean(result.invoice),
            () =>
              this.client.lookupInvoice({
                payment_hash: paymentHash || undefined,
                invoice: paymentHash ? undefined : invoice,
              }),
            options,
          ),
        errorLogs,
      );

      return nwcTransactionToLniTransaction(tx);
    } catch (error) {
      if (error instanceof LniError && error.code === 'Canceled') {
        throw error;
      }

      if (error instanceof LniError && error.code === 'NetworkError') {
        if (!lookupOptions.suppressBufferedLookupErrors) {
          replayConsoleErrors(errorLogs);
        }
        throwNwcOrApiError(error, 'lookup_invoice', 'Failed to lookup invoice');
      }

      const fallback = await this.lookupInvoiceFromTransactions(paymentHash, invoice, options);
      if (fallback) {
        return fallback;
      }

      if (!lookupOptions.suppressBufferedLookupErrors) {
        replayConsoleErrors(errorLogs);
      }
      throwNwcOrApiError(error, 'lookup_invoice', 'Failed to lookup invoice');
    }
  }

  private async lookupInvoiceFromTransactions(
    paymentHash: string,
    invoice: string | undefined,
    options: RequestCancellationOptions = {},
  ): Promise<Transaction | undefined> {
    const pageSize = 100;
    let offset = 0;

    try {
      while (true) {
        const request = {
          from: 0,
          limit: pageSize,
          offset,
        };
        const response = await this.executeNip47Request(
          'list transactions',
          'list_transactions',
          request,
          (result: NwcListTransactionsResponse) => Boolean(result.transactions),
          () =>
            this.client.listTransactions({
              from: 0,
              limit: pageSize,
              offset,
            }),
          options,
        );
        const page = response as NwcListTransactionsResponse;
        const transactions = page.transactions.map((tx) => nwcTransactionToLniTransaction(tx));

        const found = transactions.find((tx) => {
          if (paymentHash && tx.paymentHash === paymentHash) {
            return true;
          }

          return Boolean(invoice && tx.invoice === invoice);
        });

        if (found) {
          return found;
        }

        if (page.transactions.length < pageSize) {
          return undefined;
        }

        offset += page.transactions.length;
      }
    } catch {
      return undefined;
    }
  }

  async listTransactions(params: ListTransactionsParams): Promise<Transaction[]> {
    const request = {
      from: params.from > 0 ? params.from : undefined,
      limit: params.limit > 0 ? params.limit : undefined,
    };
    const response = await this.executeNip47Request(
      'list transactions',
      'list_transactions',
      request,
      (result: NwcListTransactionsResponse) => Boolean(result.transactions),
      () => this.client.listTransactions(request),
    ).catch((error) => {
      throwNwcOrApiError(error, 'list_transactions', 'Failed to list transactions');
    });

    return this.filterTransactions(response as NwcListTransactionsResponse, params);
  }

  private filterTransactions(
    response: NwcListTransactionsResponse,
    params: ListTransactionsParams,
  ): Transaction[] {
    const mapped = response.transactions.map((tx) => nwcTransactionToLniTransaction(tx));

    return mapped.filter((tx) => {
      if (params.paymentHash && tx.paymentHash !== params.paymentHash) {
        return false;
      }

      return matchesSearch(tx, params.search);
    });
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

  private cancelActiveRequests(message: string): void {
    for (const request of [...this.activeRequests]) {
      request.cancel(new LniError('Canceled', message));
    }
  }

  private createCancellationError(operation: string, message: string): LniError {
    return new LniError('Canceled', `NWC ${operation} ${message}`);
  }

  private async withTimeout<T>(
    operation: string,
    fn: () => Promise<T>,
    options: RequestCancellationOptions = {},
  ): Promise<T> {
    if (this.closed) {
      throw this.createCancellationError(operation, 'canceled because the node is closed.');
    }

    if (options.signal?.aborted) {
      throw this.createCancellationError(operation, 'canceled because the request was aborted.');
    }

    const promise = Promise.resolve().then(fn);
    const timeoutMs = this.timeoutMs;
    if (!timeoutMs && !options.signal) {
      return promise;
    }

    let timeoutId: ReturnType<typeof setTimeout> | undefined;
    let settled = false;
    let rejectWait: (error: LniError) => void = () => undefined;
    const activeRequest: ActiveNwcRequest = {
      cancel: (reason) => {
        if (settled) {
          return;
        }

        settled = true;
        if (timeoutId) {
          clearTimeout(timeoutId);
        }
        options.signal?.removeEventListener('abort', onAbort);
        this.activeRequests.delete(activeRequest);
        rejectWait(reason);
      },
    };
    const onAbort = () => activeRequest.cancel(this.createCancellationError(operation, 'canceled because the request was aborted.'));
    const wait = new Promise<never>((_resolve, reject) => {
      rejectWait = reject;
      if (timeoutMs) {
        timeoutId = setTimeout(() => {
          activeRequest.cancel(
            new LniError(
              'NetworkError',
              `NWC ${operation} timed out after ${timeoutMs / 1000}s. Check that the relay websocket is reachable and the wallet connection still exists.`,
            ),
          );
        }, timeoutMs);
      }
    });
    this.activeRequests.add(activeRequest);
    options.signal?.addEventListener('abort', onAbort, { once: true });
    void promise.catch(() => undefined);

    try {
      return await Promise.race([promise, wait]);
    } finally {
      settled = true;
      if (timeoutId) {
        clearTimeout(timeoutId);
      }
      options.signal?.removeEventListener('abort', onAbort);
      this.activeRequests.delete(activeRequest);
    }
  }

  private async executeNip47Request<T>(
    operation: string,
    _method: Nip47Method,
    _params: unknown,
    _validator: (result: T) => boolean,
    fallback: () => Promise<T>,
    options: RequestCancellationOptions = {},
  ): Promise<T> {
    return this.withTimeout(operation, fallback, options);
  }
}
