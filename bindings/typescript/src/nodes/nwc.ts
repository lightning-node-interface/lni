import { NWCClient, type Nip47GetBalanceResponse, type Nip47GetInfoResponse, type Nip47ListTransactionsResponse, type Nip47Transaction } from '@getalby/sdk/nwc';
import { LniError } from '../errors.js';
import { bytesToHex, hexToBytes } from '../internal/encoding.js';
import { pollInvoiceEvents } from '../internal/polling.js';
import { emptyNodeInfo, emptyTransaction, parseOptionalNumber } from '../internal/transform.js';
import type { CreateInvoiceParams, CreateOfferParams, InvoiceEventCallback, LightningNode, ListTransactionsParams, LookupInvoiceParams, NodeInfo, NodeRequestOptions, NwcConfig, Offer, OnInvoiceEventParams, PayInvoiceParams, PayInvoiceResponse, Transaction } from '../types.js';

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

async function sha256Hex(bytes: Uint8Array): Promise<string> {
  if (!globalThis.crypto?.subtle) {
    throw new LniError('Api', 'Web Crypto API is required to hash NWC preimages.');
  }

  const digestInput = new Uint8Array(bytes.length);
  digestInput.set(bytes);
  const digest = await globalThis.crypto.subtle.digest('SHA-256', digestInput);
  return bytesToHex(new Uint8Array(digest));
}

function nwcTransactionToLniTransaction(tx: Nip47Transaction): Transaction {
  return emptyTransaction({
    type: tx.type,
    invoice: tx.invoice ?? '',
    description: tx.description ?? '',
    descriptionHash: tx.description_hash ?? '',
    preimage: tx.preimage ?? '',
    paymentHash: tx.payment_hash ?? '',
    amountMsats: parseOptionalNumber(tx.amount),
    feesPaid: parseOptionalNumber(tx.fees_paid),
    createdAt: parseOptionalNumber(tx.created_at),
    expiresAt: parseOptionalNumber(tx.expires_at),
    settledAt: parseOptionalNumber(tx.settled_at),
    payerNote: '',
    externalId: '',
  });
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
    const paymentHash = params.paymentHash;
    const invoice = params.search;

    if (!paymentHash && !invoice) {
      throw new LniError('InvalidInput', 'lookupInvoice requires paymentHash or search (invoice) for NwcNode.');
    }

    const tx = await this.client
      .lookupInvoice({
        payment_hash: paymentHash,
        invoice,
      })
      .catch((error) => {
        throw new LniError('Api', `Failed to lookup invoice: ${(error as Error)?.message ?? 'unknown error'}`);
      });

    return nwcTransactionToLniTransaction(tx);
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

    return this.filterTransactions(response, params);
  }

  private filterTransactions(
    response: Nip47ListTransactionsResponse,
    params: ListTransactionsParams,
  ): Transaction[] {
    const mapped = response.transactions.map((tx) => nwcTransactionToLniTransaction(tx));

    return mapped.filter((tx) => {
      if (params.paymentHash && tx.paymentHash !== params.paymentHash) {
        return false;
      }

      if (params.search) {
        const search = params.search.toLowerCase();
        return (
          tx.paymentHash.toLowerCase().includes(search) ||
          tx.invoice.toLowerCase().includes(search) ||
          tx.description.toLowerCase().includes(search)
        );
      }

      return true;
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
