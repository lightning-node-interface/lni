export type LniErrorCode =
  | 'Http'
  | 'Api'
  | 'Json'
  | 'NetworkError'
  | 'InvalidInput'
  | 'LnurlError';

export class LniError extends Error {
  public readonly code: LniErrorCode;
  public readonly status?: number;
  public readonly body?: string;

  constructor(code: LniErrorCode, message: string, options?: { status?: number; body?: string; cause?: unknown }) {
    super(message, options?.cause !== undefined ? { cause: options.cause } : undefined);
    this.name = 'LniError';
    this.code = code;
    this.status = options?.status;
    this.body = options?.body;
  }
}

export function asLniError(error: unknown, fallbackCode: LniErrorCode = 'Api'): LniError {
  if (error instanceof LniError) {
    return error;
  }

  if (error instanceof Error) {
    return new LniError(fallbackCode, error.message, { cause: error });
  }

  return new LniError(fallbackCode, 'Unknown error', { cause: error });
}
