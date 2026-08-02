import { InvoiceType, LniError } from '@sunnyln/lni';
import type {
  CreateInvoiceParams,
  CreateOfferParams,
  InvoiceEventCallback,
  LightningNode,
  ListTransactionsParams,
  LookupInvoiceParams,
  NodeInfo,
  Offer,
  OnInvoiceEventParams,
  PayInvoiceParams,
  PayInvoiceResponse,
  Permissions,
  Transaction,
  TransactionType,
} from '@sunnyln/lni';

import {
  InvoiceType as NativeInvoiceType,
  LexeNode,
} from './generated/react_native_lexe';
import type {
  CreateInvoiceParams as NativeCreateInvoiceParams,
  CreateOfferParams as NativeCreateOfferParams,
  HumanBitcoinAddress as NativeHumanBitcoinAddress,
  LexeNodeLike as NativeLexeNodeLike,
  ListTransactionsParams as NativeListTransactionsParams,
  NodeInfo as NativeNodeInfo,
  Offer as NativeOffer,
  OnInvoiceEventCallback as NativeInvoiceEventCallback,
  OnInvoiceEventParams as NativeOnInvoiceEventParams,
  PayInvoiceParams as NativePayInvoiceParams,
  PayInvoiceResponse as NativePayInvoiceResponse,
  Permissions as NativePermissions,
  Transaction as NativeTransaction,
} from './generated/react_native_lexe';

export interface LexeLniNodeConfig {
  clientCredentials: string;
  dataDir?: string;
  network?: string;
}

export type LexeHumanBitcoinAddress = {
  humanBitcoinAddress: string;
  lightningAddress: string;
  offer: string;
  updatable: boolean;
};

type NativeLexeNode = NativeLexeNodeLike & {
  uniffiDestroy(): void;
};

function invalidInteger(field: string, value: number): LniError {
  return new LniError(
    'InvalidInput',
    `${field} must be a safe integer; received ${String(value)}`
  );
}

function toBigInt(value: number, field: string): bigint {
  if (!Number.isSafeInteger(value)) {
    throw invalidInteger(field, value);
  }
  return BigInt(value);
}

function toOptionalBigInt(
  value: number | undefined,
  field: string
): bigint | undefined {
  return value === undefined ? undefined : toBigInt(value, field);
}

function toFiniteNumber(value: number | undefined, field: string) {
  if (value !== undefined && !Number.isFinite(value)) {
    throw new LniError(
      'InvalidInput',
      `${field} must be a finite number; received ${String(value)}`
    );
  }
  return value;
}

function toSafeNumber(value: bigint, field: string): number {
  const numberValue = Number(value);
  if (!Number.isSafeInteger(numberValue) || BigInt(numberValue) !== value) {
    throw new LniError(
      'InvalidInput',
      `${field} is outside JavaScript's safe-integer range: ${value.toString()}`
    );
  }
  return numberValue;
}

function toOptionalSafeNumber(
  value: bigint | undefined,
  field: string
): number | undefined {
  return value === undefined ? undefined : toSafeNumber(value, field);
}

function toNativeInvoiceType(
  invoiceType: InvoiceType | undefined
): NativeInvoiceType | undefined {
  switch (invoiceType) {
    case undefined:
      return undefined;
    case InvoiceType.Bolt11:
      return NativeInvoiceType.Bolt11;
    case InvoiceType.Bolt12:
      return NativeInvoiceType.Bolt12;
    default:
      throw new LniError(
        'InvalidInput',
        `Unsupported invoice type: ${String(invoiceType)}`
      );
  }
}

function toTransactionType(type: string): TransactionType {
  if (type === 'incoming' || type === 'outgoing') {
    return type;
  }
  throw new LniError(
    'InvalidInput',
    `Unsupported Lexe transaction type: ${type}`
  );
}

function toTransaction(transaction: NativeTransaction): Transaction {
  return {
    type: toTransactionType(transaction.type),
    invoice: transaction.invoice,
    description: transaction.description,
    descriptionHash: transaction.descriptionHash,
    preimage: transaction.preimage,
    paymentHash: transaction.paymentHash,
    amountMsats: toSafeNumber(transaction.amountMsats, 'amountMsats'),
    feesPaid: toSafeNumber(transaction.feesPaid, 'feesPaid'),
    createdAt: toSafeNumber(transaction.createdAt, 'createdAt'),
    expiresAt: toSafeNumber(transaction.expiresAt, 'expiresAt'),
    settledAt: toSafeNumber(transaction.settledAt, 'settledAt'),
    payerNote: transaction.payerNote,
    externalId: transaction.externalId,
  };
}

function toOffer(offer: NativeOffer): Offer {
  return {
    offerId: offer.offerId,
    bolt12: offer.bolt12,
    label: offer.label,
    active: offer.active,
    singleUse: offer.singleUse,
    used: offer.used,
    amountMsats: toOptionalSafeNumber(offer.amountMsats, 'amountMsats'),
  };
}

function toNodeInfo(info: NativeNodeInfo): NodeInfo {
  return {
    alias: info.alias,
    color: info.color,
    pubkey: info.pubkey,
    network: info.network,
    blockHeight: toSafeNumber(info.blockHeight, 'blockHeight'),
    blockHash: info.blockHash,
    sendBalanceMsat: toSafeNumber(info.sendBalanceMsat, 'sendBalanceMsat'),
    receiveBalanceMsat: toSafeNumber(
      info.receiveBalanceMsat,
      'receiveBalanceMsat'
    ),
    feeCreditBalanceMsat: toSafeNumber(
      info.feeCreditBalanceMsat,
      'feeCreditBalanceMsat'
    ),
    unsettledSendBalanceMsat: toSafeNumber(
      info.unsettledSendBalanceMsat,
      'unsettledSendBalanceMsat'
    ),
    unsettledReceiveBalanceMsat: toSafeNumber(
      info.unsettledReceiveBalanceMsat,
      'unsettledReceiveBalanceMsat'
    ),
    pendingOpenSendBalance: toSafeNumber(
      info.pendingOpenSendBalance,
      'pendingOpenSendBalance'
    ),
    pendingOpenReceiveBalance: toSafeNumber(
      info.pendingOpenReceiveBalance,
      'pendingOpenReceiveBalance'
    ),
  };
}

function toHumanBitcoinAddress(
  address: NativeHumanBitcoinAddress
): LexeHumanBitcoinAddress {
  return {
    humanBitcoinAddress: address.humanBitcoinAddress,
    lightningAddress: address.lightningAddress,
    offer: address.offer,
    updatable: address.updatable,
  };
}

function toPermissions(permissions: NativePermissions): Permissions {
  return {
    getInfo: permissions.getInfo,
    createInvoice: permissions.createInvoice,
    payInvoice: permissions.payInvoice,
    createOffer: permissions.createOffer,
    getOffer: permissions.getOffer,
    listOffers: permissions.listOffers,
    payOffer: permissions.payOffer,
    lookupInvoice: permissions.lookupInvoice,
    listTransactions: permissions.listTransactions,
    decode: permissions.decode,
    onInvoiceEvents: permissions.onInvoiceEvents,
  };
}

function toPayInvoiceResponse(
  response: NativePayInvoiceResponse
): PayInvoiceResponse {
  return {
    paymentHash: response.paymentHash,
    preimage: response.preimage,
    feeMsats: toSafeNumber(response.feeMsats, 'feeMsats'),
  };
}

function toNativeCreateInvoiceParams(
  params: CreateInvoiceParams
): NativeCreateInvoiceParams {
  return {
    invoiceType: toNativeInvoiceType(params.invoiceType),
    amountMsats: toOptionalBigInt(params.amountMsats, 'amountMsats'),
    offer: params.offer,
    description: params.description,
    descriptionHash: params.descriptionHash,
    expiry: toOptionalBigInt(params.expiry, 'expiry'),
    rPreimage: params.rPreimage,
    isBlinded: params.isBlinded,
    isKeysend: params.isKeysend,
    isAmp: params.isAmp,
    isPrivate: params.isPrivate,
  };
}

function toNativePayInvoiceParams(
  params: PayInvoiceParams
): NativePayInvoiceParams {
  return {
    invoice: params.invoice,
    feeLimitMsat: toOptionalBigInt(params.feeLimitMsat, 'feeLimitMsat'),
    feeLimitPercentage: toFiniteNumber(
      params.feeLimitPercentage,
      'feeLimitPercentage'
    ),
    timeoutSeconds: toOptionalBigInt(params.timeoutSeconds, 'timeoutSeconds'),
    amountMsats: toOptionalBigInt(params.amountMsats, 'amountMsats'),
    maxParts: toOptionalBigInt(params.maxParts, 'maxParts'),
    firstHopPubkey: params.firstHopPubkey,
    lastHopPubkey: params.lastHopPubkey,
    allowSelfPayment: params.allowSelfPayment,
    isAmp: params.isAmp,
  };
}

function toNativeCreateOfferParams(
  params: CreateOfferParams
): NativeCreateOfferParams {
  return {
    description: params.description,
    amountMsats: toOptionalBigInt(params.amountMsats, 'amountMsats'),
  };
}

function toNativeListTransactionsParams(
  params: ListTransactionsParams
): NativeListTransactionsParams {
  return {
    from: toBigInt(params.from, 'from'),
    limit: toBigInt(params.limit, 'limit'),
    paymentHash: params.paymentHash,
    search: params.search,
    createdAfter: toOptionalBigInt(params.createdAfter, 'createdAfter'),
    createdBefore: toOptionalBigInt(params.createdBefore, 'createdBefore'),
  };
}

function toNativeInvoiceEventParams(
  params: OnInvoiceEventParams
): NativeOnInvoiceEventParams {
  return {
    paymentHash: params.paymentHash,
    search: params.search,
    pollingDelaySec: toBigInt(params.pollingDelaySec, 'pollingDelaySec'),
    maxPollingSec: toBigInt(params.maxPollingSec, 'maxPollingSec'),
  };
}

function nestedErrorMessage(error: unknown): string | undefined {
  if (typeof error !== 'object' || error === null || !('inner' in error)) {
    return undefined;
  }
  const inner = error.inner;
  if (typeof inner !== 'object' || inner === null || !('message' in inner)) {
    return undefined;
  }
  return typeof inner.message === 'string' && inner.message.length > 0
    ? inner.message
    : undefined;
}

function toLniError(error: unknown): LniError {
  if (error instanceof LniError) {
    return error;
  }

  const message =
    nestedErrorMessage(error) ??
    (error instanceof Error && error.message.length > 0
      ? error.message
      : 'Unknown Lexe error');
  return new LniError('Api', message, { cause: error });
}

/** Adapts the native Lexe binding to the shared `LightningNode` interface. */
export class LexeLniNode implements LightningNode {
  readonly #nativeNode: NativeLexeNode;
  #closed = false;

  constructor(config: LexeLniNodeConfig);
  /** @internal Native-node injection is only for package adapter tests. */
  constructor(config: LexeLniNodeConfig, nativeNode: NativeLexeNode);
  constructor(config: LexeLniNodeConfig, nativeNode?: NativeLexeNode) {
    try {
      this.#nativeNode = nativeNode ?? new LexeNode(config);
    } catch (error) {
      throw toLniError(error);
    }
  }

  async getPermissions(): Promise<Permissions> {
    return toPermissions(
      await this.#call(() => this.#nativeNode.getPermissions())
    );
  }

  async getInfo(): Promise<NodeInfo> {
    return toNodeInfo(await this.#call(() => this.#nativeNode.getInfo()));
  }

  async getHumanBitcoinAddress(): Promise<LexeHumanBitcoinAddress> {
    return toHumanBitcoinAddress(
      await this.#call(() => this.#nativeNode.getHumanBitcoinAddress())
    );
  }

  async createInvoice(params: CreateInvoiceParams): Promise<Transaction> {
    return toTransaction(
      await this.#call(() =>
        this.#nativeNode.createInvoice(toNativeCreateInvoiceParams(params))
      )
    );
  }

  async payInvoice(params: PayInvoiceParams): Promise<PayInvoiceResponse> {
    return toPayInvoiceResponse(
      await this.#call(() =>
        this.#nativeNode.payInvoice(toNativePayInvoiceParams(params))
      )
    );
  }

  async createOffer(params: CreateOfferParams): Promise<Offer> {
    return toOffer(
      await this.#call(() =>
        this.#nativeNode.createOffer(toNativeCreateOfferParams(params))
      )
    );
  }

  async getOffer(search?: string): Promise<Offer> {
    return toOffer(await this.#call(() => this.#nativeNode.getOffer(search)));
  }

  async listOffers(search?: string): Promise<Offer[]> {
    return (await this.#call(() => this.#nativeNode.listOffers(search))).map(
      toOffer
    );
  }

  async payOffer(
    offer: string,
    amountMsats: number,
    payerNote?: string
  ): Promise<PayInvoiceResponse> {
    return toPayInvoiceResponse(
      await this.#call(() =>
        this.#nativeNode.payOffer(
          offer,
          toBigInt(amountMsats, 'amountMsats'),
          payerNote
        )
      )
    );
  }

  async lookupInvoice(params: LookupInvoiceParams): Promise<Transaction> {
    return toTransaction(
      await this.#call(() => this.#nativeNode.lookupInvoice(params))
    );
  }

  async listTransactions(
    params: ListTransactionsParams
  ): Promise<Transaction[]> {
    return (
      await this.#call(() =>
        this.#nativeNode.listTransactions(
          toNativeListTransactionsParams(params)
        )
      )
    ).map(toTransaction);
  }

  async decode(value: string): Promise<string> {
    return this.#call(() => this.#nativeNode.decode(value));
  }

  async decodeOffer(offer: string): Promise<string> {
    return this.#call(() => this.#nativeNode.decodeOffer(offer));
  }

  async onInvoiceEvents(
    params: OnInvoiceEventParams,
    callback: InvoiceEventCallback
  ): Promise<void> {
    const nativeCallback: NativeInvoiceEventCallback = {
      success: (transaction) =>
        callback(
          'success',
          transaction === undefined ? undefined : toTransaction(transaction)
        ),
      pending: (transaction) =>
        callback(
          'pending',
          transaction === undefined ? undefined : toTransaction(transaction)
        ),
      failure: (transaction) =>
        callback(
          'failure',
          transaction === undefined ? undefined : toTransaction(transaction)
        ),
    };

    await this.#call(() =>
      this.#nativeNode.onInvoiceEvents(
        toNativeInvoiceEventParams(params),
        nativeCallback
      )
    );
  }

  close(): void {
    if (this.#closed) {
      return;
    }
    this.#closed = true;
    try {
      this.#nativeNode.uniffiDestroy();
    } catch (error) {
      throw toLniError(error);
    }
  }

  async #call<T>(operation: () => Promise<T>): Promise<T> {
    if (this.#closed) {
      throw new LniError('Api', 'Lexe node is closed');
    }
    try {
      return await operation();
    } catch (error) {
      throw toLniError(error);
    }
  }
}
