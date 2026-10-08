import { LexeConnectSession } from './generated/react_native_lexe';
import type {
  LexeConnectSessionLike,
  LexeConnectResponse,
} from './generated/react_native_lexe';

/** Requester configuration. Omit destinations to use Lexe's encrypted mailbox. */
export interface LexeConnectConfig {
  redirectUri?: string;
  postUrl?: string;
  mailboxUrl?: string;
  scopes?: string[];
  permissions?: string[];
  /** Account the user will recognize on Lexe's approval screen. */
  account?: string;
  metadata?: string;
  requesterName?: string;
  requesterIcon?: string;
  label?: string;
  /** Suggested credential expiry in Unix milliseconds; editable in Lexe. */
  expiresAtMs?: number;
  /** Local request lifetime, 1–300 seconds. Defaults to 300. */
  requestTtlSecs?: number;
}

type ResponseContext = { account?: string; metadata?: string };

export type LexeConnectResult =
  | (ResponseContext & {
      status: 'approved';
      clientCredentials: string;
      scopes: string[];
      permissions: string[];
      expiresAtMs?: number;
    })
  | (ResponseContext & {
      status: 'rejected' | 'error';
      error: 'user_rejected' | 'other';
      message?: string;
    });

type NativeSession = LexeConnectSessionLike & { uniffiDestroy(): void };

function safeInteger(value: number, name: string): number {
  if (!Number.isSafeInteger(value) || value < 0) {
    throw new Error(`${name} must be a non-negative safe integer`);
  }
  return value;
}

function result(response: LexeConnectResponse): LexeConnectResult {
  const context = { account: response.account, metadata: response.metadata };
  if (response.clientCredentials !== undefined) {
    return {
      ...context,
      status: 'approved',
      clientCredentials: response.clientCredentials,
      scopes: response.scopes,
      permissions: response.permissions,
      expiresAtMs:
        response.expiresAtMs === undefined
          ? undefined
          : safeInteger(Number(response.expiresAtMs), 'expiresAtMs'),
    };
  }
  return {
    ...context,
    status: response.error === 'user_rejected' ? 'rejected' : 'error',
    error: response.error === 'user_rejected' ? 'user_rejected' : 'other',
    message: response.errorMessage,
  };
}

function abortError(): Error {
  const error = new Error('LexeConnect cancelled');
  error.name = 'AbortError';
  return error;
}

function delay(ms: number, signal: AbortSignal): Promise<void> {
  return new Promise((resolve, reject) => {
    if (signal.aborted) {
      reject(abortError());
      return;
    }
    const abort = () => {
      clearTimeout(timer);
      signal.removeEventListener('abort', abort);
      reject(abortError());
    };
    const timer = setTimeout(() => {
      signal.removeEventListener('abort', abort);
      resolve();
    }, ms);
    signal.addEventListener('abort', abort, { once: true });
  });
}

/**
 * One connection attempt backed by Rust HPKE and response verification.
 * Create before opening Lexe; keep this instance until its callback arrives.
 * Keys stay in native memory. Dispose on screen unmount; do not log links or
 * results. Callbacks cannot be recovered after the app process is terminated.
 */
export class LexeConnect {
  private readonly native: NativeSession;
  private disposed = false;
  private waiter?: AbortController;

  constructor(config: LexeConnectConfig);
  /** @internal Native injection for adapter tests. */
  constructor(config: LexeConnectConfig, native: NativeSession);
  constructor(config: LexeConnectConfig, native?: NativeSession) {
    const expiresAtMs =
      config.expiresAtMs === undefined
        ? undefined
        : BigInt(safeInteger(config.expiresAtMs, 'expiresAtMs'));
    const requestTtlSecs = config.requestTtlSecs;
    if (
      requestTtlSecs !== undefined &&
      (!Number.isInteger(requestTtlSecs) ||
        requestTtlSecs < 1 ||
        requestTtlSecs > 300)
    ) {
      throw new Error('requestTtlSecs must be between 1 and 300');
    }
    this.native =
      native ??
      new LexeConnectSession({
        ...config,
        scopes: config.scopes ?? [],
        permissions: config.permissions ?? [],
        expiresAtMs,
        requestTtlSecs,
      });
  }

  private assertLive(): void {
    if (this.disposed) throw new Error('LexeConnect disposed');
  }

  /** Open with Linking.openURL, or encode as a QR code. Treat as sensitive. */
  connectionString(): string {
    this.assertLive();
    return this.native.connectionString();
  }

  acceptRedirect(url: string): LexeConnectResult {
    this.assertLive();
    return result(this.native.acceptRedirect(url));
  }

  /** Raw encrypted POST/mailbox bytes, not base64 text. */
  acceptBody(body: Uint8Array): LexeConnectResult {
    this.assertLive();
    // Copy only this view, not unrelated bytes in its underlying buffer.
    return result(this.native.acceptBody(Uint8Array.from(body).buffer));
  }

  /** Poll once; undefined means no response yet. */
  async pollMailbox(
    signal?: AbortSignal
  ): Promise<LexeConnectResult | undefined> {
    this.assertLive();
    const response = await this.native.pollMailbox(
      signal ? { signal } : undefined
    );
    return response === undefined ? undefined : result(response);
  }

  /**
   * Poll until approval, rejection, expiration, or a transport error. Aborting
   * cancels the session and discards its keys. Transport errors allow retry with
   * the same session while it remains live. Only one waiter is allowed at a time.
   */
  async waitForResponse(
    options: {
      signal?: AbortSignal;
      pollIntervalMs?: number;
    } = {}
  ): Promise<LexeConnectResult> {
    this.assertLive();
    if (this.waiter) throw new Error('LexeConnect is already waiting');
    const interval = options.pollIntervalMs ?? 1000;
    if (!Number.isInteger(interval) || interval < 250 || interval > 5000) {
      throw new Error('pollIntervalMs must be between 250 and 5000');
    }
    const controller = new AbortController();
    this.waiter = controller;
    const abort = () => this.cancel();
    options.signal?.addEventListener('abort', abort, { once: true });
    try {
      if (options.signal?.aborted) this.cancel();
      while (true) {
        if (controller.signal.aborted) throw abortError();
        const response = await this.pollMailbox(controller.signal);
        if (controller.signal.aborted) throw abortError();
        if (response !== undefined) return response;
        await delay(interval, controller.signal);
      }
    } finally {
      options.signal?.removeEventListener('abort', abort);
      this.waiter = undefined;
    }
  }

  cancel(): void {
    if (this.disposed) return;
    this.waiter?.abort();
    this.native.cancel();
  }

  /** Cancel and release the native handle. Safe to call repeatedly. */
  dispose(): void {
    if (this.disposed) return;
    this.cancel();
    this.native.uniffiDestroy();
    this.disposed = true;
  }
}
