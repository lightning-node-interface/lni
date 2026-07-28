import { createGaloyNode, type GaloyNode } from './galoy.js';
import type {
  CreateInvoiceParams,
  CreateOfferParams,
  FlashConfig,
  InvoiceEventCallback,
  LightningNode,
  ListTransactionsParams,
  LookupInvoiceParams,
  NodeInfo,
  NodeRequestOptions,
  Offer,
  OnInvoiceEventParams,
  PayInvoiceParams,
  PayInvoiceResponse,
  Permissions,
  Transaction,
} from '../types.js';

export const DEFAULT_FLASH_GRAPHQL_URL = 'https://api.flashapp.me/graphql';

/**
 * Flash adapter backed by the generic Galoy GraphQL implementation.
 *
 * Use `createGaloyNode` directly when a Flash deployment exposes capabilities
 * beyond these conservative Flash defaults.
 */
export class FlashNode implements LightningNode {
  private readonly node: GaloyNode;

  constructor(config: FlashConfig, options: NodeRequestOptions = {}) {
    this.node = createGaloyNode(
      {
        apiKey: config.apiKey,
        baseUrl: config.baseUrl ?? DEFAULT_FLASH_GRAPHQL_URL,
        provider: {
          id: 'flash',
          name: 'Flash',
        },
        wallet: {
          mode: 'explicit',
          id: config.walletId,
          currency: config.walletCurrency,
        },
        payment: {
          response: 'status-only',
          acceptedStatuses: config.acceptedStatuses ?? ['SUCCESS', 'PENDING', 'ALREADY_PAID'],
        },
        capabilities: {
          transactionLookup: false,
          transactionHistory: false,
          invoiceEvents: false,
          onchain: false,
        },
        permissions: 'configured',
        additionalHeaders: config.additionalHeaders,
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
