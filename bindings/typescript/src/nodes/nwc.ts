import { NWCClient, type Nip47GetBalanceResponse, type Nip47GetInfoResponse, type Nip47ListTransactionsResponse, type Nip47Transaction } from '@getalby/sdk/nwc';
import { decode as decodeBolt11 } from 'light-bolt11-decoder';
import { LniError } from '../errors.js';
import { bytesToHex, hexToBytes } from '../internal/encoding.js';
import { NWC_METHOD_PERMISSIONS, normalizeNwcPermissions } from '../internal/permissions.js';
import { pollInvoiceEvents } from '../internal/polling.js';
import { emptyNodeInfo, emptyTransaction, matchesSearch, parseOptionalNumber } from '../internal/transform.js';
import type { CreateInvoiceParams, CreateOfferParams, InvoiceEventCallback, LightningNode, ListTransactionsParams, LookupInvoiceParams, NodeInfo, NodeRequestOptions, NwcConfig, Offer, OnInvoiceEventParams, PayInvoiceParams, PayInvoiceResponse, Permissions, Transaction } from '../types.js';

type NwcListTransaction = Partial<Omit<Nip47Transaction, 'type' | 'payment_hash'>> & {
  type?: Nip47Transaction['type'];
  payment_hash?: string;
};

type NwcListTransactionsResponse = Omit<Nip47ListTransactionsResponse, 'transactions' | 'total_count'> & {
  transactions: NwcListTransaction[];
  total_count?: number;
};

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

function paymentHashFromInvoice(invoice: string): string {
  if (!invoice) {
    return '';
  }

  try {
    const decoded = decodeBolt11(invoice);
    const section = decoded.sections.find((item) => item.name === 'payment_hash');
    return section?.name === 'payment_hash' ? section.value : '';
  } catch {
    return '';
  }
}

async function sha256Hex(bytes: Uint8Array): Promise<string> {
  if (!globalThis.crypto?.subtle) {
    throw new LniError('Api', 'Web Crypto API is required to hash NWC preimages.');
  }

  const digest = await globalThis.crypto.subtle.digest('SHA-256', bytes as BufferSource);
  return bytesToHex(new Uint8Array(digest));
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

export class NwcNode implements LightningNode {
  private readonly client: NWCClient;

  constructor(private readonly config: NwcConfig, _options: NodeRequestOptions = {}) {
    this.client = new NWCClient({
      nostrWalletConnectUrl: config.nwcUri,
    });
  }

  close(): void {
    this.client.close();
  }

  async getPermissions(): Promise<Permissions> {
    const info = await this.client.getInfo().catch((error) => {
      throw new LniError('Api', `Failed to get NWC permissions: ${(error as Error)?.message ?? 'unknown error'}`);
    });
    const methods = (info as Nip47GetInfoResponse & { methods?: string[] }).methods;
    return normalizeNwcPermissions(methods?.length ? methods : NWC_METHOD_PERMISSIONS);
  }

  async getInfo(): Promise<NodeInfo> {
    const balance = await this.client.getBalance().catch((error) => {
      throw new LniError('Api', `Failed to get balance: ${(error as Error)?.message ?? 'unknown error'}`);
    });

    const pubkeyFallback = extractPubkeyFromNwcUri(this.config.nwcUri);

    try {
      const info = await this.client.getInfo();
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
    const tx = await this.client
      .makeInvoice({
        amount: params.amountMsats ?? 0,
        description: params.description,
        description_hash: params.descriptionHash,
        expiry: params.expiry,
      })
      .catch((error) => {
        throw new LniError('Api', `Failed to create invoice: ${(error as Error)?.message ?? 'unknown error'}`);
      });

    return nwcTransactionToLniTransaction(tx);
  }

  async payInvoice(params: PayInvoiceParams): Promise<PayInvoiceResponse> {
    const response = await this.client
      .payInvoice({
        invoice: params.invoice,
        amount: params.amountMsats,
      })
      .catch((error) => {
        throw new LniError('Api', `Failed to pay invoice: ${(error as Error)?.message ?? 'unknown error'}`);
      });

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
    const paymentHash = params.paymentHash ?? paymentHashFromInvoice(params.search ?? '');
    const invoice = params.search;

    if (!paymentHash && !invoice) {
      throw new LniError('InvalidInput', 'lookupInvoice requires paymentHash or search (invoice) for NwcNode.');
    }

    const errorLogs: unknown[][] = [];
    try {
      const tx = await bufferGetAlbyLookupInvoiceErrors(
        () =>
          this.client.lookupInvoice({
            payment_hash: paymentHash || undefined,
            invoice: paymentHash ? undefined : invoice,
          }),
        errorLogs,
      );

      return nwcTransactionToLniTransaction(tx);
    } catch (error) {
      const fallback = await this.lookupInvoiceFromTransactions(paymentHash, invoice);
      if (fallback) {
        return fallback;
      }

      replayConsoleErrors(errorLogs);
      throw new LniError('Api', `Failed to lookup invoice: ${(error as Error)?.message ?? 'unknown error'}`);
    }
  }

  private async lookupInvoiceFromTransactions(
    paymentHash: string,
    invoice: string | undefined,
  ): Promise<Transaction | undefined> {
    const pageSize = 100;
    let offset = 0;

    try {
      while (true) {
        const response = await this.client.listTransactions({
          from: 0,
          limit: pageSize,
          offset,
        });
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
    const response = await this.client
      .listTransactions({
        from: params.from > 0 ? params.from : undefined,
        limit: params.limit > 0 ? params.limit : undefined,
      })
      .catch((error) => {
        throw new LniError('Api', `Failed to list transactions: ${(error as Error)?.message ?? 'unknown error'}`);
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
    return str;
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
