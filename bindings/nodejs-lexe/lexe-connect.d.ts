import type { LexeConnectOptions } from './generated'

export type LexeConnectResult =
  | { status: 'approved', clientCredentials: string, scopes: string[], permissions: string[], expiresAtMs?: number, account?: string, metadata?: string }
  | { status: 'rejected' | 'error', error: 'user_rejected' | 'other', message?: string, account?: string, metadata?: string }

/** Any `AbortSignal`, or an equivalent object on Node versions without one. */
export type LexeAbortSignal = Pick<AbortSignal, 'aborted' | 'addEventListener' | 'removeEventListener'>

/** One LexeConnect attempt; retain this instance until its response arrives. */
export declare class LexeConnect {
  constructor(config: LexeConnectOptions)
  connectionString(): string
  acceptRedirect(url: string): LexeConnectResult
  acceptBody(body: Uint8Array): LexeConnectResult
  /**
   * Polls once; `undefined` means no response yet. Concurrent calls share one
   * native request. Aborting `signal` rejects only this call with `AbortError`:
   * the request continues, and a verified response it receives is returned by
   * the next `pollMailbox()` or `waitForResponse()`. Use `cancel()` to end the attempt.
   */
  pollMailbox(signal?: LexeAbortSignal): Promise<LexeConnectResult | undefined>
  /** Polls until a response arrives. Aborting `signal` cancels the attempt. */
  waitForResponse(options?: { signal?: LexeAbortSignal, pollIntervalMs?: number }): Promise<LexeConnectResult>
  /**
   * Ends the attempt. A response verified before cancellation took effect is
   * still returned by the next `pollMailbox()`.
   */
  cancel(): void
  /** Cancels and releases the session, discarding any retained response. */
  dispose(): void
}
