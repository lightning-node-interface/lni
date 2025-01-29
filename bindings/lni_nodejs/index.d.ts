/* tslint:disable */
/* eslint-disable */

/* auto-generated by NAPI-RS */

export interface PhoenixdConfig {
  url: string
  password: string
}
export interface PhoenixdNode {
  url: string
  password: string
}
export interface Bolt11Resp {
  amountSat: number
  paymentHash: string
  serialized: string
}
export interface PhoenixdMakeInvoiceParams {
  invoiceType: InvoiceType
  amount: number
  description?: string
  descriptionHash?: string
  expiry?: number
}
export interface ListTransactionsParams {
  from: number
  until: number
  limit: number
  offset: number
  unpaid: boolean
  invoiceType: string
}
export const enum InvoiceType {
  Bolt11 = 'Bolt11',
  Bolt12 = 'Bolt12'
}
export interface TlvRecord {
  type: number
  value: string
}
export interface NodeInfo {
  alias: string
  color: string
  pubkey: string
  network: string
  blockHeight: number
  blockHash: string
}
export interface Transaction {
  type: string
  invoice: string
  description: string
  descriptionHash: string
  preimage: string
  paymentHash: string
  amount: number
  feesPaid: number
  createdAt: number
  expiresAt: number
  settledAt: number
}
export interface NodeConnectionInfo {
  pubkey: string
  address: string
  port: number
}
export interface Channel {
  localBalance: number
  localSpendableBalance: number
  remoteBalance: number
  id: string
  remotePubkey: string
  fundingTxId: string
  fundingTxVout: number
  active: boolean
  public: boolean
  internalChannel: string
  confirmations: number
  confirmationsRequired: number
  forwardingFeeBaseMsat: number
  unspendablePunishmentReserve: number
  counterpartyUnspendablePunishmentReserve: number
  error: string
  isOutbound: boolean
}
export interface NodeStatus {
  isReady: boolean
  internalNodeStatus: string
}
export interface ConnectPeerRequest {
  pubkey: string
  address: string
  port: number
}
export interface OpenChannelRequest {
  pubkey: string
  amountSats: number
  public: boolean
}
export interface OpenChannelResponse {
  fundingTxId: string
}
export interface CloseChannelRequest {
  channelId: string
  nodeId: string
  force: boolean
}
export interface UpdateChannelRequest {
  channelId: string
  nodeId: string
  forwardingFeeBaseMsat: number
  maxDustHtlcExposureFromFeeRateMultiplier: number
}
export interface CloseChannelResponse {
  
}
export interface PendingBalanceDetails {
  channelId: string
  nodeId: string
  amount: number
  fundingTxId: string
  fundingTxVout: number
}
export interface OnchainBalanceResponse {
  spendable: number
  total: number
  reserved: number
  pendingBalancesFromChannelClosures: number
  pendingBalancesDetails: Array<PendingBalanceDetails>
  internalBalances: string
}
export interface PeerDetails {
  nodeId: string
  address: string
  isPersisted: boolean
  isConnected: boolean
}
export interface LightningBalanceResponse {
  totalSpendable: number
  totalReceivable: number
  nextMaxSpendable: number
  nextMaxReceivable: number
  nextMaxSpendableMpp: number
  nextMaxReceivableMpp: number
}
export interface PayInvoiceResponse {
  preimage: string
  fee: number
}
export interface PayKeysendResponse {
  fee: number
}
export interface BalancesResponse {
  onchain: OnchainBalanceResponse
  lightning: LightningBalanceResponse
}
export interface PaymentFailedEventProperties {
  transaction: Transaction
  reason: string
}
export declare class PhoenixdNode {
  constructor(config: PhoenixdConfig)
  getUrl(): string
  getPassword(): string
  getConfig(): PhoenixdConfig
  getInfo(): Promise<NodeInfo>
  makeInvoice(params: PhoenixdMakeInvoiceParams): Promise<Transaction>
  lookupInvoice(paymentHash: string): Promise<Transaction>
  listTransactions(params: ListTransactionsParams): Promise<Array<Transaction>>
}
