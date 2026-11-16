import type {
  OnchainTransaction,
  OnInvoiceEventParams,
  PayOnchainOptions,
  PayOnchainResponse,
  PrepareOnchainTransactionParams,
  Transaction,
} from './generated'

/** Hand-written public contract; napi-rs does not generate declarations for interfaces. */
export interface OnchainPayments {
  prepareOnchainTransaction(params: PrepareOnchainTransactionParams): Promise<OnchainTransaction>
  payOnchain(
    transaction: OnchainTransaction,
    options?: PayOnchainOptions | undefined | null
  ): Promise<PayOnchainResponse>
}

declare module './generated' {
  interface LexeNode {
    /**
     * Resolves after a `success` or `failure` event, or without a further
     * callback when `close()` ends the monitor.
     */
    onInvoiceEvents(
      params: OnInvoiceEventParams,
      callback: (status: 'success' | 'pending' | 'failure', transaction?: Transaction) => void
    ): Promise<void>
  }
}
