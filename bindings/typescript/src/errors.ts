export type LniErrorCode =
  | 'Http'
  | 'Api'
  | 'Json'
  | 'NetworkError'
  | 'InvalidInput'
  | 'FeeError'
  | 'LnurlError'
  | 'NwcError';

export type NwcStandardErrorCode =
  | 'RATE_LIMITED'
  | 'NOT_IMPLEMENTED'
  | 'INSUFFICIENT_BALANCE'
  | 'PAYMENT_FAILED'
  | 'NOT_FOUND'
  | 'QUOTA_EXCEEDED'
  | 'RESTRICTED'
  | 'UNAUTHORIZED'
  | 'INTERNAL'
  | 'UNSUPPORTED_ENCRYPTION'
  | 'OTHER';

export type NwcErrorCode = NwcStandardErrorCode | (string & {});

export type NwcErrorOperation =
  | 'get_info'
  | 'get_balance'
  | 'make_invoice'
  | 'pay_invoice'
  | 'lookup_invoice'
  | 'list_transactions';

export class LniError extends Error {
  public readonly code: LniErrorCode;
  public readonly status?: number;
  public readonly body?: string;

  constructor(
    code: LniErrorCode,
    message: string,
    options?: { status?: number; body?: string; cause?: unknown }
  ) {
    super(message, options?.cause !== undefined ? { cause: options.cause } : undefined);
    this.name = 'LniError';
    this.code = code;
    this.status = options?.status;
    this.body = options?.body;
  }
}

/**
 * A valid quoted or provider fee exceeded a caller-configured fee limit.
 *
 * This is a local LNI guardrail error: the payment was not executed. Invalid
 * fee-limit configuration and quotes whose fee cannot be determined safely
 * continue to use `LniError` with the `InvalidInput` code.
 */
export class FeeError extends LniError {
  constructor(message: string, options?: { cause?: unknown }) {
    super('FeeError', message, options?.cause !== undefined ? { cause: options.cause } : undefined);
    this.name = 'FeeError';
  }
}

export class NwcError extends LniError {
  public readonly nwcCode: NwcErrorCode;
  public readonly nwcMessage: string;
  public readonly operation?: NwcErrorOperation;
  public readonly provider?: string;
  public readonly providerCode?: string | number;
  public readonly providerStatus?: number;
  public readonly providerMessage?: string;

  constructor(
    nwcCode: NwcErrorCode,
    message: string,
    options?: {
      operation?: NwcErrorOperation;
      cause?: unknown;
      provider?: string;
      providerCode?: string | number;
      providerStatus?: number;
      providerMessage?: string;
    }
  ) {
    super(
      'NwcError',
      message,
      options?.cause !== undefined
        ? { cause: options.cause, status: options.providerStatus }
        : { status: options?.providerStatus }
    );
    this.name = 'NwcError';
    this.nwcCode = nwcCode;
    this.nwcMessage = message;
    this.operation = options?.operation;
    this.provider = options?.provider;
    this.providerCode = options?.providerCode;
    this.providerStatus = options?.providerStatus;
    this.providerMessage = options?.providerMessage;
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
