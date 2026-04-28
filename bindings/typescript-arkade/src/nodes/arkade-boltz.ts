import type { ArkadeSwapsCreateConfig, SwapRepository } from '@arkade-os/boltz-swap';
import type { StorageConfig } from '@arkade-os/sdk';
import {
  asLniError,
  InvoiceType,
  LniError,
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
  type PayInvoiceParams,
  type PayInvoiceResponse,
  type Permissions,
  type Transaction,
} from '@sunnyln/lni';
import { pollInvoiceEvents } from '@sunnyln/lni/internal/polling';
import { emptyNodeInfo, emptyTransaction, matchesSearch, satsToMsats } from '@sunnyln/lni/internal/transform';
import type {
  ArkadeBoltzConfig,
  ArkadeBoltzNetwork,
  ArkadeBoltzSwapFilter,
  ArkadeBoltzSwapRepository,
} from '../types.js';

type ArkadeRuntimeNetwork = 'bitcoin' | 'testnet' | 'signet' | 'mutinynet' | 'regtest';

const ARKADE_BOLTZ_NODE_PERMISSIONS: Permissions = {
  getInfo: true,
  createInvoice: true,
  payInvoice: true,
  createOffer: false,
  getOffer: false,
  listOffers: false,
  payOffer: false,
  lookupInvoice: true,
  listTransactions: true,
  decode: true,
  onInvoiceEvents: true,
};

type ArkadeWalletLike = {
  networkName?: ArkadeRuntimeNetwork;
  getBalance(): Promise<{ available?: unknown }>;
};

type ArkadeLightningInvoice = {
  amountSats: number;
  expiry: number;
  description: string;
  paymentHash: string;
};

type ArkadeReverseSwap = {
  id: string;
  type: 'reverse';
  createdAt: number;
  preimage: string;
  status: string;
  request: {
    invoiceAmount: number;
    preimageHash: string;
    description?: string;
  };
  response: {
    invoice: string;
    onchainAmount: number;
  };
};

type ArkadeSubmarineSwap = {
  id: string;
  type: 'submarine';
  createdAt: number;
  preimage?: string;
  preimageHash?: string;
  status: string;
  request: {
    invoice: string;
  };
  response: {
    expectedAmount: number;
  };
};

type ArkadePendingSwap = ArkadeReverseSwap | ArkadeSubmarineSwap | { id: string; type: string; createdAt: number; status: string };

type ArkadeSwapsLike = {
  createLightningInvoice(args: { amount: number; description?: string }): Promise<{
    amount: number;
    expiry: number;
    invoice: string;
    paymentHash: string;
    preimage: string;
    pendingSwap: { createdAt: number; id?: string };
  }>;
  sendLightningPayment(args: { invoice: string }): Promise<{
    amount: number;
    preimage: string;
    txid: string;
  }>;
  getSwapHistory(): Promise<ArkadePendingSwap[]>;
};

type ArkadeBoltzContext = {
  wallet: ArkadeWalletLike;
  swaps: ArkadeSwapsLike;
  decodeInvoice(invoice: string): ArkadeLightningInvoice;
  getInvoicePaymentHash(invoice: string): string;
};

class InMemoryArkadeSwapRepository implements ArkadeBoltzSwapRepository {
  readonly version = 1 as const;
  private readonly swaps = new Map<string, unknown>();

  async saveSwap<T = unknown>(swap: T): Promise<void> {
    const id = typeof swap === 'object' && swap && 'id' in swap ? (swap as { id: string }).id : '';
    if (!id) {
      throw new Error('Arkade swap repository requires swap.id.');
    }
    this.swaps.set(id, swap);
  }

  async deleteSwap(id: string): Promise<void> {
    this.swaps.delete(id);
  }

  async getAllSwaps<T = unknown>(filter?: ArkadeBoltzSwapFilter): Promise<T[]> {
    let rows = Array.from(this.swaps.values()) as Array<Record<string, unknown>>;

    if (filter?.id) {
      const ids = new Set(Array.isArray(filter.id) ? filter.id : [filter.id]);
      rows = rows.filter((row) => typeof row.id === 'string' && ids.has(row.id));
    }

    if (filter?.status) {
      const statuses = new Set(Array.isArray(filter.status) ? filter.status : [filter.status]);
      rows = rows.filter((row) => typeof row.status === 'string' && statuses.has(row.status));
    }

    if (filter?.type) {
      const types = new Set(Array.isArray(filter.type) ? filter.type : [filter.type]);
      rows = rows.filter((row) => typeof row.type === 'string' && types.has(row.type));
    }

    if (filter?.orderBy === 'createdAt') {
      rows = rows.sort((a, b) => {
        const left = typeof a.createdAt === 'number' ? a.createdAt : 0;
        const right = typeof b.createdAt === 'number' ? b.createdAt : 0;
        return filter.orderDirection === 'asc' ? left - right : right - left;
      });
    }

    return rows as T[];
  }

  async clear(): Promise<void> {
    this.swaps.clear();
  }
}

function toArkadeNetwork(network?: ArkadeBoltzNetwork): ArkadeRuntimeNetwork {
  switch (network ?? 'mainnet') {
    case 'mainnet':
    case 'bitcoin':
      return 'bitcoin';
    case 'testnet':
      return 'testnet';
    case 'signet':
      return 'signet';
    case 'mutinynet':
      return 'mutinynet';
    case 'regtest':
      return 'regtest';
    default:
      return 'bitcoin';
  }
}

function toLniNetwork(network?: string): string {
  return network === 'bitcoin' ? 'mainnet' : (network ?? 'mainnet');
}

function isIncomingSuccess(status: string): boolean {
  return status === 'invoice.settled';
}

function isOutgoingSuccess(status: string): boolean {
  return status === 'invoice.paid' || status === 'transaction.claimed';
}

function isFailed(status: string): boolean {
  return status === 'invoice.expired'
    || status === 'invoice.failedToPay'
    || status === 'swap.expired'
    || status === 'transaction.failed'
    || status === 'transaction.lockupFailed'
    || status === 'transaction.refunded';
}

function isReverseSwap(swap: ArkadePendingSwap): swap is ArkadeReverseSwap {
  return swap.type === 'reverse' && 'request' in swap && 'response' in swap && 'preimage' in swap;
}

function isSubmarineSwap(swap: ArkadePendingSwap): swap is ArkadeSubmarineSwap {
  return swap.type === 'submarine' && 'request' in swap && 'response' in swap;
}

function toNumber(value: unknown): number {
  if (typeof value === 'number') {
    return Number.isFinite(value) ? value : 0;
  }
  if (typeof value === 'bigint') {
    return Number(value);
  }
  if (typeof value === 'string') {
    const parsed = Number(value);
    return Number.isFinite(parsed) ? parsed : 0;
  }
  return 0;
}

export class ArkadeBoltzNode implements LightningNode {
  private contextPromise?: Promise<ArkadeBoltzContext>;

  constructor(
    private readonly config: ArkadeBoltzConfig,
    _options: NodeRequestOptions = {},
  ) {}

  private async getContext(): Promise<ArkadeBoltzContext> {
    if (!this.contextPromise) {
      this.contextPromise = this.initContext();
    }
    return this.contextPromise;
  }

  private async initContext(): Promise<ArkadeBoltzContext> {
    const [{ InMemoryContractRepository, InMemoryWalletRepository, MnemonicIdentity, Wallet }, boltz] = await Promise.all([
      import('@arkade-os/sdk'),
      import('@arkade-os/boltz-swap'),
    ]);

    const identity = MnemonicIdentity.fromMnemonic(this.config.mnemonic, {
      isMainnet: toArkadeNetwork(this.config.network) === 'bitcoin',
      passphrase: this.config.passphrase,
    });

    const wallet = await Wallet.create({
      identity,
      arkServerUrl: this.config.arkServerUrl,
      indexerUrl: this.config.indexerUrl,
      esploraUrl: this.config.esploraUrl,
      arkServerPublicKey: this.config.arkServerPublicKey,
      storage: (this.config.walletStorage ?? {
        walletRepository: new InMemoryWalletRepository(),
        contractRepository: new InMemoryContractRepository(),
      }) as StorageConfig,
    });

    const network = toArkadeNetwork(this.config.network ?? (wallet as ArkadeWalletLike).networkName);
    const swapProvider = new boltz.BoltzSwapProvider({
      network,
      apiUrl: this.config.swapApiUrl,
      referralId: this.config.referralId,
    });

    const swaps = await boltz.ArkadeSwaps.create({
      wallet,
      swapProvider,
      swapManager: this.config.swapManager,
      swapRepository: (this.config.swapRepository ?? new InMemoryArkadeSwapRepository()) as SwapRepository,
    } satisfies ArkadeSwapsCreateConfig);

    return {
      wallet: wallet as ArkadeWalletLike,
      swaps,
      decodeInvoice: boltz.decodeInvoice,
      getInvoicePaymentHash: boltz.getInvoicePaymentHash,
    };
  }

  private reverseSwapToTransaction(swap: ArkadeReverseSwap): Transaction {
    const amountMsats = satsToMsats(swap.request.invoiceAmount);
    const receivedMsats = satsToMsats(swap.response.onchainAmount);
    const settled = isIncomingSuccess(swap.status);

    return emptyTransaction({
      type: 'incoming',
      invoice: swap.response.invoice,
      description: swap.request.description ?? '',
      descriptionHash: '',
      preimage: settled ? swap.preimage : '',
      paymentHash: swap.request.preimageHash,
      amountMsats,
      feesPaid: Math.max(0, amountMsats - receivedMsats),
      createdAt: swap.createdAt,
      settledAt: settled ? swap.createdAt : 0,
      externalId: swap.id,
    });
  }

  private submarineSwapToTransaction(swap: ArkadeSubmarineSwap, decoded: ArkadeLightningInvoice): Transaction {
    const amountMsats = satsToMsats(decoded.amountSats);
    const expectedMsats = satsToMsats(swap.response.expectedAmount);
    const settled = isOutgoingSuccess(swap.status);
    const failed = isFailed(swap.status);

    return emptyTransaction({
      type: 'outgoing',
      invoice: swap.request.invoice,
      description: decoded.description,
      descriptionHash: '',
      preimage: settled ? (swap.preimage ?? '') : '',
      paymentHash: swap.preimageHash ?? decoded.paymentHash,
      amountMsats,
      feesPaid: Math.max(0, expectedMsats - amountMsats),
      createdAt: swap.createdAt,
      expiresAt: decoded.expiry,
      settledAt: settled || failed ? swap.createdAt : 0,
      externalId: swap.id,
    });
  }

  private mapSwapHistory(swaps: ArkadePendingSwap[], decodeInvoice: ArkadeBoltzContext['decodeInvoice']): Transaction[] {
    const txs: Transaction[] = [];

    for (const swap of swaps) {
      if (isReverseSwap(swap)) {
        txs.push(this.reverseSwapToTransaction(swap));
        continue;
      }

      if (!isSubmarineSwap(swap)) {
        continue;
      }

      try {
        txs.push(this.submarineSwapToTransaction(swap, decodeInvoice(swap.request.invoice)));
      } catch {
        txs.push(emptyTransaction({
          type: 'outgoing',
          invoice: swap.request.invoice,
          paymentHash: swap.preimageHash ?? '',
          amountMsats: 0,
          feesPaid: satsToMsats(swap.response.expectedAmount),
          createdAt: swap.createdAt,
          settledAt: isOutgoingSuccess(swap.status) || isFailed(swap.status) ? swap.createdAt : 0,
          externalId: swap.id,
        }));
      }
    }

    return txs;
  }

  async getPermissions(): Promise<Permissions> {
    return { ...ARKADE_BOLTZ_NODE_PERMISSIONS };
  }

  async getInfo(): Promise<NodeInfo> {
    try {
      const { wallet } = await this.getContext();
        const balance = await wallet.getBalance();

        return emptyNodeInfo({
          alias: 'Arkade Boltz Node',
          network: toLniNetwork(wallet.networkName ?? this.config.network),
          sendBalanceMsat: satsToMsats(toNumber(balance.available)),
        });
    } catch (error) {
      throw asLniError(error);
    }
  }

  async createInvoice(params: CreateInvoiceParams): Promise<Transaction> {
    if ((params.invoiceType ?? InvoiceType.Bolt11) !== InvoiceType.Bolt11) {
      throw new LniError('Api', 'Bolt12 is not implemented for ArkadeBoltzNode.');
    }
    if (params.amountMsats === undefined) {
      throw new LniError('InvalidInput', 'ArkadeBoltzNode createInvoice requires amountMsats.');
    }

    try {
      const { swaps } = await this.getContext();
      const amountSats = Math.floor(params.amountMsats / 1000);
      if (amountSats <= 0) {
        throw new LniError('InvalidInput', 'ArkadeBoltzNode createInvoice requires amountMsats > 0.');
      }

      const response = await swaps.createLightningInvoice({
        amount: amountSats,
        description: params.description,
      });

      return emptyTransaction({
        type: 'incoming',
        invoice: response.invoice,
        description: params.description ?? '',
        descriptionHash: params.descriptionHash ?? '',
        preimage: response.preimage,
        paymentHash: response.paymentHash,
        amountMsats: params.amountMsats,
        feesPaid: Math.max(0, params.amountMsats - satsToMsats(response.amount)),
        createdAt: response.pendingSwap.createdAt ?? 0,
        expiresAt: response.expiry,
        settledAt: 0,
        externalId: response.pendingSwap.id,
      });
    } catch (error) {
      throw asLniError(error);
    }
  }

  async payInvoice(params: PayInvoiceParams): Promise<PayInvoiceResponse> {
    try {
      const { decodeInvoice, getInvoicePaymentHash, swaps } = await this.getContext();
      const decoded = decodeInvoice(params.invoice);
      if (decoded.amountSats <= 0) {
        throw new LniError('InvalidInput', 'ArkadeBoltzNode does not support amountless invoices.');
      }

      const response = await swaps.sendLightningPayment({
        invoice: params.invoice,
      });

      return {
        paymentHash: getInvoicePaymentHash(params.invoice),
        preimage: response.preimage,
        feeMsats: Math.max(0, satsToMsats(response.amount - decoded.amountSats)),
      };
    } catch (error) {
      throw asLniError(error);
    }
  }

  async createOffer(_params: CreateOfferParams): Promise<Offer> {
    throw new LniError('Api', 'Bolt12 is not implemented for ArkadeBoltzNode.');
  }

  async getOffer(_search?: string): Promise<Offer> {
    throw new LniError('Api', 'Bolt12 is not implemented for ArkadeBoltzNode.');
  }

  async listOffers(_search?: string): Promise<Offer[]> {
    throw new LniError('Api', 'Bolt12 is not implemented for ArkadeBoltzNode.');
  }

  async payOffer(_offer: string, _amountMsats: number, _payerNote?: string): Promise<PayInvoiceResponse> {
    throw new LniError('Api', 'Bolt12 is not implemented for ArkadeBoltzNode.');
  }

  async lookupInvoice(params: LookupInvoiceParams): Promise<Transaction> {
    try {
      const { decodeInvoice, swaps } = await this.getContext();
      const txs = this.mapSwapHistory(await swaps.getSwapHistory(), decodeInvoice);
      const match = txs.find((tx) => {
        if (params.paymentHash && tx.paymentHash !== params.paymentHash) {
          return false;
        }
        return matchesSearch(tx, params.search);
      });

      if (!match) {
        throw new LniError('Api', 'No arkade-boltz transaction found matching lookup parameters.');
      }

      return match;
    } catch (error) {
      throw asLniError(error);
    }
  }

  async listTransactions(params: ListTransactionsParams): Promise<Transaction[]> {
    try {
      const { decodeInvoice, swaps } = await this.getContext();
      const txs = this.mapSwapHistory(await swaps.getSwapHistory(), decodeInvoice)
        .filter((tx) => {
          if (params.paymentHash && tx.paymentHash !== params.paymentHash) {
            return false;
          }
          if (!matchesSearch(tx, params.search)) {
            return false;
          }
          if (params.createdAfter && tx.createdAt < params.createdAfter) {
            return false;
          }
          if (params.createdBefore && tx.createdAt > params.createdBefore) {
            return false;
          }
          return true;
        })
        .sort((a, b) => b.createdAt - a.createdAt);

      const start = Math.max(0, params.from || 0);
      const end = params.limit > 0 ? start + params.limit : undefined;
      return txs.slice(start, end);
    } catch (error) {
      throw asLniError(error);
    }
  }

  async decode(str: string): Promise<string> {
    return str;
  }

  async onInvoiceEvents(params: OnInvoiceEventParams, callback: InvoiceEventCallback): Promise<void> {
    await pollInvoiceEvents({
      params,
      callback,
      lookup: () => this.lookupInvoice({ paymentHash: params.paymentHash, search: params.search }),
    });
  }
}
