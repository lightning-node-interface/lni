import { LniError, NwcError, type NwcErrorCode, type NwcErrorOperation } from '../errors.js';

export interface ProviderErrorInfo {
  code?: string | number;
  status?: number;
  message?: string;
}

export interface ProviderErrorNormalizationOptions {
  provider: string;
  operation?: NwcErrorOperation;
  extractProviderError?: (error: unknown) => ProviderErrorInfo | undefined;
  mapProviderError?: (info: ProviderErrorInfo) => NwcErrorCode | undefined;
}

function mapHttpStatus(status: number | undefined): NwcErrorCode | undefined {
  switch (status) {
    case 401:
      return 'UNAUTHORIZED';
    case 403:
      return 'RESTRICTED';
    case 404:
      return 'NOT_FOUND';
    case 429:
      return 'RATE_LIMITED';
    default:
      if (status !== undefined && status >= 500) {
        return 'INTERNAL';
      }
      return undefined;
  }
}

export function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

const MAX_PROVIDER_ERROR_SEARCH_STEPS = 500;

function findProperty<T>(
  value: unknown,
  key: string,
  coerce: (value: unknown) => T | undefined
): T | undefined {
  const queue: unknown[] = [value];
  let steps = 0;

  while (queue.length > 0 && steps < MAX_PROVIDER_ERROR_SEARCH_STEPS) {
    steps += 1;
    const current = queue.shift();

    if (Array.isArray(current)) {
      queue.push(...current);
      continue;
    }

    if (!isRecord(current)) {
      continue;
    }

    const direct = coerce(current[key]);
    if (direct !== undefined) {
      return direct;
    }

    queue.push(...Object.values(current));
  }

  return undefined;
}

export function findStringProperty(value: unknown, key: string): string | undefined {
  return findProperty(value, key, (direct) => {
    return typeof direct === 'string' && direct.length > 0 ? direct : undefined;
  });
}

export function findNumberProperty(value: unknown, key: string): number | undefined {
  return findProperty(value, key, (direct) => {
    if (typeof direct === 'number' && Number.isFinite(direct)) {
      return direct;
    }

    if (typeof direct === 'string' && direct.trim().length > 0) {
      const parsed = Number.parseInt(direct, 10);
      if (Number.isFinite(parsed)) {
        return parsed;
      }
    }

    return undefined;
  });
}

export function parseProviderJsonBody(error: unknown): unknown | undefined {
  if (!(error instanceof LniError) || !error.body) {
    return undefined;
  }

  try {
    return JSON.parse(error.body);
  } catch {
    return undefined;
  }
}

export function providerInfoFromJsonErrorBody(error: unknown): ProviderErrorInfo | undefined {
  if (!(error instanceof LniError)) {
    return undefined;
  }

  const raw = parseProviderJsonBody(error);
  if (raw === undefined) {
    return error.body || error.status !== undefined
      ? { status: error.status, message: error.body }
      : undefined;
  }

  const source = isRecord(raw) && isRecord(raw.error) ? raw.error : raw;
  return {
    code: findStringProperty(source, 'code') ?? findNumberProperty(source, 'code'),
    status: findNumberProperty(source, 'status') ?? error.status,
    message: findStringProperty(source, 'message') ?? findStringProperty(source, 'reason'),
  };
}

export function mapProviderMessage(message: string | undefined): NwcErrorCode | undefined {
  const normalized = message?.toLowerCase() ?? '';
  if (!normalized) {
    return undefined;
  }

  if (normalized.includes('rate limit') || normalized.includes('too many requests')) {
    return 'RATE_LIMITED';
  }
  if (
    normalized.includes('unauthorized') ||
    normalized.includes('unauthenticated') ||
    normalized.includes('not authorized')
  ) {
    return 'UNAUTHORIZED';
  }
  if (
    normalized.includes('permission denied') ||
    normalized.includes('forbidden') ||
    normalized.includes('scope')
  ) {
    return 'RESTRICTED';
  }
  if (
    normalized.includes('insufficient') ||
    normalized.includes('balance too low') ||
    normalized.includes('not enough funds')
  ) {
    return 'INSUFFICIENT_BALANCE';
  }
  if (
    normalized.includes('quota') ||
    normalized.includes('limit exceeded') ||
    normalized.includes('amount too high')
  ) {
    return 'QUOTA_EXCEEDED';
  }
  if (
    normalized.includes('invalid invoice') ||
    normalized.includes('invoice expired') ||
    normalized.includes('expired invoice') ||
    normalized.includes('no route') ||
    normalized.includes('route not found') ||
    normalized.includes('payment failed') ||
    normalized.includes('recipient')
  ) {
    return 'PAYMENT_FAILED';
  }
  if (normalized.includes('not found')) {
    return 'NOT_FOUND';
  }

  return undefined;
}

function mapLniErrorCode(error: LniError): NwcErrorCode {
  switch (error.code) {
    case 'NetworkError':
    case 'Json':
      return 'INTERNAL';
    case 'Http':
      return mapHttpStatus(error.status) ?? 'OTHER';
    default:
      return 'OTHER';
  }
}

export function normalizeProviderError(
  error: unknown,
  options: ProviderErrorNormalizationOptions
): NwcError {
  if (error instanceof NwcError) {
    const operation = options.operation ?? error.operation;
    const provider = error.provider ?? options.provider;
    if (operation === error.operation && provider === error.provider) {
      return error;
    }

    return new NwcError(error.nwcCode, error.nwcMessage, {
      operation,
      cause: error,
      provider,
      providerCode: error.providerCode,
      providerStatus: error.providerStatus,
      providerMessage: error.providerMessage,
    });
  }

  const providerInfo = options.extractProviderError?.(error);
  const providerStatus =
    providerInfo?.status ?? (error instanceof LniError ? error.status : undefined);
  const nwcCode =
    (providerInfo ? options.mapProviderError?.(providerInfo) : undefined) ??
    mapProviderMessage(providerInfo?.message) ??
    mapHttpStatus(providerStatus) ??
    (error instanceof LniError ? mapLniErrorCode(error) : 'OTHER');

  const message =
    providerInfo?.message ?? (error instanceof Error ? error.message : 'Unknown provider error');

  return new NwcError(nwcCode, message, {
    operation: options.operation,
    cause: error,
    provider: options.provider,
    providerCode: providerInfo?.code,
    providerStatus,
    providerMessage: providerInfo?.message,
  });
}

export function throwNormalizedProviderError(
  error: unknown,
  options: ProviderErrorNormalizationOptions
): never {
  throw normalizeProviderError(error, options);
}
