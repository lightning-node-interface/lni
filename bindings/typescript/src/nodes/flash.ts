import { createGaloyNode, type GaloyNode } from './galoy.js';
import type {
  CreateInvoiceParams,
  CreateOfferParams,
  FlashConfig,
  GaloyPaymentOutcome,
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

function flashFeeProbeOperation(walletCurrency: string) {
  switch (walletCurrency.toUpperCase()) {
    case 'BTC':
      return { kind: 'btc', denomination: 'sats' } as const;
    case 'USD':
      return { kind: 'usd', denomination: 'usd-cents' } as const;
    default:
      return { kind: 'unsupported' } as const;
  }
}

/**
 * Flash adapter backed by the generic Galoy GraphQL implementation.
 *
 * Use `createGaloyNode` directly when a Flash deployment exposes capabilities
 * beyond these conservative Flash defaults.
 *
 * Accepted proofless statuses can resolve with an empty preimage because
 * status-only Galoy responses do not include proof data. Use
 * `payInvoiceWithStatus()` when a resolved `PENDING` payment must remain
 * distinguishable from settlement.
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
        invoiceOperations: {
          create: { kind: 'unsupported' },
          feeProbe: flashFeeProbeOperation(config.walletCurrency),
        },
        payment: {
          response: 'status-only',
          acceptedStatuses: config.acceptedStatuses ?? ['SUCCESS', 'PENDING', 'ALREADY_PAID'],
          statusMapping: {
            settled: ['SUCCESS', 'ALREADY_PAID'],
            pending: ['PENDING'],
          },
          proofUnavailableErrorCodes: ['PROOF_UNAVAILABLE'],
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

  payInvoiceWithStatus(params: PayInvoiceParams): Promise<GaloyPaymentOutcome> {
    return this.node.payInvoiceWithStatus(params);
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
