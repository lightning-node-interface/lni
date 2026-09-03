export type FetchLike = (input: RequestInfo | URL, init?: RequestInit) => Promise<Response>;

/** Simple async key-value store for caching. */
export interface StorageProvider {
  get(key: string): Promise<string | null>;
  set(key: string, value: string): Promise<void>;
  remove(key: string): Promise<void>;
}

export type InvoiceEventStatus = 'success' | 'pending' | 'failure';
export type InvoiceEventCallback = (status: InvoiceEventStatus, transaction?: Transaction) => void;

export enum InvoiceType {
  Bolt11 = 'Bolt11',
  Bolt12 = 'Bolt12',
}

export interface NodeInfo {
  alias: string;
  color: string;
  pubkey: string;
  network: string;
  blockHeight: number;
  blockHash: string;
  sendBalanceMsat: number;
  receiveBalanceMsat: number;
  feeCreditBalanceMsat: number;
  unsettledSendBalanceMsat: number;
  unsettledReceiveBalanceMsat: number;
  pendingOpenSendBalance: number;
  pendingOpenReceiveBalance: number;
}

export type TransactionType = 'incoming' | 'outgoing';

export type SettlementType = 'lightning' | 'onchain' | 'intraledger' | 'unknown';

export type SettlementState = 'pending' | 'completed' | 'failed' | 'unknown';

export interface Transaction {
  type: TransactionType;
  invoice: string;
  description: string;
  descriptionHash: string;
  preimage: string;
  paymentHash: string;
  amountMsats: number;
  feesPaid: number;
  createdAt: number;
  expiresAt: number;
  settledAt: number;
  payerNote?: string;
  externalId?: string;
  settlementType?: SettlementType;
  settlementState?: SettlementState;
  txid?: string;
}

export interface PayInvoiceResponse {
  paymentHash: string;
  preimage: string;
  feeMsats: number;
}

export type OnchainFeeSpeed = 'fast' | 'normal' | 'slow' | 'free';

export type OnchainFeePreference =
  | { type: 'default' }
  | { type: 'speed'; speed: OnchainFeeSpeed }
  | { type: 'targetConf'; blocks: number }
  | { type: 'satsPerVbyte'; satsPerVbyte: number }
  | { type: 'backend'; value: string };

export type OnchainFeePayer = 'sender' | 'recipient';

export interface PrepareOnchainTransactionParams {
  address: string;
  amountSats: number;
  fee?: OnchainFeePreference;
  feePayer?: OnchainFeePayer;
  description?: string;
  idempotencyKey?: string;
}

export interface OnchainTransaction {
  id?: string;
  address: string;
  amountSats: number;
  feeSats?: number;
  totalAmountSats?: number;
  recipientAmountSats?: number;
  feePayer: OnchainFeePayer;
  fee: OnchainFeePreference;
  expiresAt?: number;
  estimatedDeliverySeconds?: number;
  raw?: unknown;
}

export interface OnchainFeeGuardrail {
  maxFeeSats?: number;
  maxFeePercent?: number;
}

export const DEFAULT_ONCHAIN_MAX_FEE_SATS = 25_000;
export const DEFAULT_ONCHAIN_MAX_FEE_PERCENT = 25;
export const DEFAULT_ONCHAIN_FEE_GUARDRAIL: Required<OnchainFeeGuardrail> = {
  maxFeeSats: DEFAULT_ONCHAIN_MAX_FEE_SATS,
  maxFeePercent: DEFAULT_ONCHAIN_MAX_FEE_PERCENT,
};

export interface PayOnchainOptions {
  feeGuardrail?: OnchainFeeGuardrail;
  dangerouslyDisableFeeGuardrail?: boolean;
}

export interface PayOnchainResponse {
  paymentId?: string;
  txid?: string;
  state: 'pending' | 'completed' | 'failed' | string;
  address: string;
  amountSats: number;
  feeSats?: number;
  totalAmountSats?: number;
  recipientAmountSats?: number;
  createdAt?: number;
  raw?: unknown;
}

export interface Offer {
  offerId: string;
  bolt12: string;
  label?: string;
  active?: boolean;
  singleUse?: boolean;
  used?: boolean;
  amountMsats?: number;
}

export interface CreateInvoiceParams {
  invoiceType?: InvoiceType;
  amountMsats?: number;
  offer?: string;
  description?: string;
  descriptionHash?: string;
  expiry?: number;
  rPreimage?: string;
  isBlinded?: boolean;
  isKeysend?: boolean;
  isAmp?: boolean;
  isPrivate?: boolean;
}

export interface CreateOfferParams {
  description?: string;
  amountMsats?: number;
}

export interface PayInvoiceParams {
  invoice: string;
  feeLimitMsat?: number;
  feeLimitPercentage?: number;
  timeoutSeconds?: number;
  amountMsats?: number;
  maxParts?: number;
  firstHopPubkey?: string;
  lastHopPubkey?: string;
  allowSelfPayment?: boolean;
  isAmp?: boolean;
}

export interface LookupInvoiceParams {
  paymentHash?: string;
  search?: string;
}

export interface ListTransactionsParams {
  from: number;
  limit: number;
  // Exact payment hash match.
  paymentHash?: string;
  // Case-insensitive partial match across common transaction text fields.
  search?: string;
  // Unix seconds — adapters that don't support this ignore it.
  createdAfter?: number;
  // Unix seconds — adapters that don't support this ignore it.
  createdBefore?: number;
}

export interface OnInvoiceEventParams {
  paymentHash?: string;
  search?: string;
  pollingDelaySec: number;
  maxPollingSec: number;
}

export interface NodeRequestOptions {
  fetch?: FetchLike;
}

export interface PhoenixdConfig {
  url: string;
  password: string;
  socks5Proxy?: string;
  acceptInvalidCerts?: boolean;
  httpTimeout?: number;
}

export interface ClnConfig {
  url: string;
  rune: string;
  socks5Proxy?: string;
  acceptInvalidCerts?: boolean;
  httpTimeout?: number;
}

export interface LndConfig {
  url: string;
  macaroon: string;
  socks5Proxy?: string;
  acceptInvalidCerts?: boolean;
  httpTimeout?: number;
}

export interface NwcConfig {
  nwcUri: string;
  socks5Proxy?: string;
  acceptInvalidCerts?: boolean;
  httpTimeout?: number;
}

export interface LightningAddressInfo {
  lightningAddress: string;
  lnurlVerifySupported: boolean;
}

export interface StrikeConfig {
  baseUrl?: string;
  apiKey: string;
  socks5Proxy?: string;
  acceptInvalidCerts?: boolean;
  httpTimeout?: number;
}

export interface SpeedConfig {
  baseUrl?: string;
  apiKey: string;
  socks5Proxy?: string;
  acceptInvalidCerts?: boolean;
  httpTimeout?: number;
}

export interface BlinkConfig {
  baseUrl?: string;
  apiKey: string;
  socks5Proxy?: string;
  acceptInvalidCerts?: boolean;
  httpTimeout?: number;
}

export interface FlashConfig {
  apiKey: string;
  baseUrl?: string;
  walletId: string;
  /**
   * Flash payment fee probing is currently documented for BTC and USD.
   * Other currencies remain unsupported until their operation family and fee
   * denomination are confirmed.
   */
  walletCurrency: string;
  additionalHeaders?: Record<string, string>;
  acceptedStatuses?: readonly string[];
  httpTimeout?: number;
}

export type GaloyWalletConfig =
  | {
      mode: 'explicit';
      id: string;
      currency: string;
    }
  | {
      mode: 'currency';
      currency: string;
    };

export interface GaloyPaymentConfig {
  response: 'transaction-with-preimage' | 'status-only';
  acceptedStatuses: readonly string[];
  /**
   * Optional semantic mapping used to distinguish accepted-but-pending
   * payments from settled payments.
   */
  statusMapping?: {
    settled: readonly string[];
    pending: readonly string[];
  };
  /**
   * Provider error codes that specifically mean payment proof is unavailable.
   * These are nonfatal only when the payment status is otherwise accepted and
   * no preimage was returned.
   */
  proofUnavailableErrorCodes?: readonly string[];
}

export type GaloyPaymentState = 'settled' | 'pending' | 'accepted';

/**
 * Galoy-specific payment details available without changing the shared
 * LightningNode PayInvoiceResponse contract.
 */
export interface GaloyPaymentOutcome {
  payment: PayInvoiceResponse;
  state: GaloyPaymentState;
  providerStatus: string;
}

export type GaloyInvoiceOperation =
  /** Galoy's BTC operation family with satoshi-denominated amounts. */
  | {
      kind: 'btc';
      denomination: 'sats';
    }
  /** Galoy's USD operation family with USD-cent-denominated amounts. */
  | {
      kind: 'usd';
      denomination: 'usd-cents';
    }
  /** No safe GraphQL operation is known for this action. */
  | {
      kind: 'unsupported';
    };

export interface GaloyInvoiceOperationsConfig {
  create: GaloyInvoiceOperation;
  feeProbe: GaloyInvoiceOperation;
}

export interface GaloyCapabilities {
  transactionLookup: boolean;
  transactionHistory: boolean;
  invoiceEvents: boolean;
  onchain: boolean;
}

export interface GaloyConfig {
  apiKey: string;
  baseUrl: string;
  provider: {
    id: string;
    name: string;
  };
  wallet: GaloyWalletConfig;
  invoiceOperations: GaloyInvoiceOperationsConfig;
  payment: GaloyPaymentConfig;
  capabilities: GaloyCapabilities;
  permissions: 'jwt-introspection' | 'configured';
  additionalHeaders?: Record<string, string>;
  httpTimeout?: number;
}

export interface Permissions {
  getInfo: boolean;
  createInvoice: boolean;
  payInvoice: boolean;
  createOffer: boolean;
  getOffer: boolean;
  listOffers: boolean;
  payOffer: boolean;
  lookupInvoice: boolean;
  listTransactions: boolean;
  decode: boolean;
  onInvoiceEvents: boolean;
}

export interface LightningNode {
  getPermissions(): Promise<Permissions>;
  getInfo(): Promise<NodeInfo>;
  createInvoice(params: CreateInvoiceParams): Promise<Transaction>;
  payInvoice(params: PayInvoiceParams): Promise<PayInvoiceResponse>;
  createOffer(params: CreateOfferParams): Promise<Offer>;
  getOffer(search?: string): Promise<Offer>;
  listOffers(search?: string): Promise<Offer[]>;
  payOffer(offer: string, amountMsats: number, payerNote?: string): Promise<PayInvoiceResponse>;
  lookupInvoice(params: LookupInvoiceParams): Promise<Transaction>;
  listTransactions(params: ListTransactionsParams): Promise<Transaction[]>;
  decode(str: string): Promise<string>;
  decodeOffer(offer: string): Promise<string>;
  onInvoiceEvents(params: OnInvoiceEventParams, callback: InvoiceEventCallback): Promise<void>;
}

export interface OnchainPayments {
  prepareOnchainTransaction(params: PrepareOnchainTransactionParams): Promise<OnchainTransaction>;
  payOnchain(
    transaction: OnchainTransaction,
    options?: PayOnchainOptions
  ): Promise<PayOnchainResponse>;
}

export type BackendNodeKind =
  | 'phoenixd'
  | 'cln'
  | 'lnd'
  | 'nwc'
  | 'strike'
  | 'speed'
  | 'galoy'
  | 'flash'
  /** @deprecated Use `galoy` with Blink defaults instead. */
  | 'blink';

export type BackendNodeConfig =
  | { kind: 'phoenixd'; config: PhoenixdConfig }
  | { kind: 'cln'; config: ClnConfig }
  | { kind: 'lnd'; config: LndConfig }
  | { kind: 'nwc'; config: NwcConfig }
  | { kind: 'strike'; config: StrikeConfig }
  | { kind: 'speed'; config: SpeedConfig }
  | { kind: 'galoy'; config: GaloyConfig }
  | { kind: 'flash'; config: FlashConfig }
  /** @deprecated Use `galoy` with Blink defaults instead. */
  | { kind: 'blink'; config: BlinkConfig };

export interface PaymentInfo {
  destinationType: 'bolt11' | 'bolt12' | 'lnurl' | 'lightning_address';
  destination: string;
  amountMsats?: number;
  minSendableMsats?: number;
  maxSendableMsats?: number;
  description?: string;
}
