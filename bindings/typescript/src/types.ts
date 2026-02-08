export type FetchLike = (input: RequestInfo | URL, init?: RequestInit) => Promise<Response>;

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
}

export interface PayInvoiceResponse {
  paymentHash: string;
  preimage: string;
  feeMsats: number;
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

export type SparkNetwork = 'mainnet' | 'regtest' | 'testnet' | 'signet' | 'local';

export interface SparkConfig {
  // 12/24-word seed phrase.
  mnemonic: string;
  // Optional passphrase applied to mnemonic->seed derivation.
  passphrase?: string;
  // Spark account index. If omitted, spark-sdk applies its default per-network.
  accountNumber?: number;
  // Spark network. Defaults to 'mainnet'.
  network?: SparkNetwork;
  // Optional override for spark-sdk runtime entrypoint loading strategy.
  // - auto: browser/Expo uses packaged no-WASM bare vendor; Node uses '@buildonspark/spark-sdk'
  // - bare: force packaged no-WASM bare vendor path
  // - native: force '@buildonspark/spark-sdk/native'
  // - default: force '@buildonspark/spark-sdk' (may load WASM depending on runtime)
  sdkEntry?: 'auto' | 'bare' | 'native' | 'default';
  // Optional max fee used by payInvoice when no fee limit is provided.
  defaultMaxFeeSats?: number;
  // Optional spark-sdk wallet options passthrough.
  sparkOptions?: Record<string, unknown>;
}

export interface LightningNode {
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
  onInvoiceEvents(params: OnInvoiceEventParams, callback: InvoiceEventCallback): Promise<void>;
}

export type BackendNodeKind =
  | 'phoenixd'
  | 'cln'
  | 'lnd'
  | 'nwc'
  | 'strike'
  | 'speed'
  | 'blink'
  | 'spark';

export type BackendNodeConfig =
  | { kind: 'phoenixd'; config: PhoenixdConfig }
  | { kind: 'cln'; config: ClnConfig }
  | { kind: 'lnd'; config: LndConfig }
  | { kind: 'nwc'; config: NwcConfig }
  | { kind: 'strike'; config: StrikeConfig }
  | { kind: 'speed'; config: SpeedConfig }
  | { kind: 'blink'; config: BlinkConfig }
  | { kind: 'spark'; config: SparkConfig };

export interface PaymentInfo {
  destinationType: 'bolt11' | 'bolt12' | 'lnurl' | 'lightning_address';
  destination: string;
  amountMsats?: number;
  minSendableMsats?: number;
  maxSendableMsats?: number;
  description?: string;
}
