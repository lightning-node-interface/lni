// Only the Lexe binding's public API is re-exported from generated core types.
export {
  LexeNode,
  LexeConnectSession,
  InvoiceType,
  SettlementType,
  SettlementState,
  OnchainFeeSpeed,
  OnchainFeePayer,
  OnchainFeePreferenceType,
} from './generated'
export type {
  LexeReceivingCapabilities,
  LexeAddressCheck,
  LexeClientInfo,
  LexeHumanBitcoinAddress,
  Permissions,
  TlvRecord,
  NodeInfo,
  Transaction,
  PayInvoiceResponse,
  OnchainFeePreference,
  PrepareOnchainTransactionParams,
  OnchainTransaction,
  OnchainFeeGuardrail,
  PayOnchainOptions,
  PayOnchainResponse,
  ListTransactionsParams,
  LookupInvoiceParams,
  CreateInvoiceParams,
  Offer,
  CreateOfferParams,
  PayInvoiceParams,
  OnInvoiceEventParams,
  LexeConnectOptions,
  LexeConnectResponse,
} from './generated'
export { LexeConnect, type LexeAbortSignal, type LexeConnectResult } from './lexe-connect'
export * from './contracts'
