import { createGaloyNode, type GaloyNode } from './galoy.js';
import type {
  BlinkConfig,
  CreateInvoiceParams,
  CreateOfferParams,
  InvoiceEventCallback,
  LightningNode,
  ListTransactionsParams,
  LookupInvoiceParams,
  NodeInfo,
  NodeRequestOptions,
  Offer,
  OnInvoiceEventParams,
  OnchainPayments,
  OnchainTransaction,
  PayInvoiceParams,
  PayInvoiceResponse,
  PayOnchainOptions,
  PayOnchainResponse,
  Permissions,
  PrepareOnchainTransactionParams,
  Transaction,
} from '../types.js';

/**
 * Backward-compatible Blink adapter.
 *
 * @deprecated Use `createGaloyNode` or `createNode({ kind: 'galoy', ... })`.
 */
export class BlinkNode implements LightningNode, OnchainPayments {
  private readonly node: GaloyNode;

  constructor(config: BlinkConfig, options: NodeRequestOptions = {}) {
    this.node = createGaloyNode(
      {
        apiKey: config.apiKey,
        baseUrl: config.baseUrl ?? 'https://api.blink.sv/graphql',
        provider: {
          id: 'blink',
          name: 'Blink',
        },
        wallet: {
          mode: 'currency',
          currency: 'BTC',
        },
        invoiceOperations: {
          create: { kind: 'btc', denomination: 'sats' },
          feeProbe: { kind: 'btc', denomination: 'sats' },
        },
        payment: {
          response: 'transaction-with-preimage',
          acceptedStatuses: ['SUCCESS'],
        },
        capabilities: {
          transactionLookup: true,
          transactionHistory: true,
          invoiceEvents: true,
          onchain: true,
        },
        permissions: 'jwt-introspection',
        httpTimeout: config.httpTimeout,
      },
      options
    );
  }

  getPermissions(): Promise<Permissions> {
    return this.node.getPermissions();
  }

  getInfo(): Promise<NodeInfo> {
    return this.node.getInfo();
  }

  createInvoice(params: CreateInvoiceParams): Promise<Transaction> {
    return this.node.createInvoice(params);
  }

  payInvoice(params: PayInvoiceParams): Promise<PayInvoiceResponse> {
    return this.node.payInvoice(params);
  }

  prepareOnchainTransaction(params: PrepareOnchainTransactionParams): Promise<OnchainTransaction> {
    return this.node.prepareOnchainTransaction(params);
  }

  payOnchain(
    transaction: OnchainTransaction,
    options?: PayOnchainOptions
  ): Promise<PayOnchainResponse> {
    return this.node.payOnchain(transaction, options);
  }

  createOffer(params: CreateOfferParams): Promise<Offer> {
    return this.node.createOffer(params);
  }

  getOffer(search?: string): Promise<Offer> {
    return this.node.getOffer(search);
  }

  listOffers(search?: string): Promise<Offer[]> {
    return this.node.listOffers(search);
  }

  payOffer(offer: string, amountMsats: number, payerNote?: string): Promise<PayInvoiceResponse> {
    return this.node.payOffer(offer, amountMsats, payerNote);
  }

  lookupInvoice(params: LookupInvoiceParams): Promise<Transaction> {
    return this.node.lookupInvoice(params);
  }

  listTransactions(params: ListTransactionsParams): Promise<Transaction[]> {
    return this.node.listTransactions(params);
  }

  decode(str: string): Promise<string> {
    return this.node.decode(str);
  }

  decodeOffer(offer: string): Promise<string> {
    return this.node.decodeOffer(offer);
  }

  onInvoiceEvents(params: OnInvoiceEventParams, callback: InvoiceEventCallback): Promise<void> {
    return this.node.onInvoiceEvents(params, callback);
  }
}
