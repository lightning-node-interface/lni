import { LniError } from '../errors.js';
import type { FetchLike } from '../types.js';

export type QueryValue = string | number | boolean | null | undefined;

export interface RequestArgs {
  method?: string;
  headers?: HeadersInit;
  query?: Record<string, QueryValue>;
  json?: unknown;
  form?: Record<string, QueryValue>;
  body?: BodyInit | null;
  timeoutMs?: number;
  signal?: AbortSignal;
}

export function resolveFetch(customFetch?: FetchLike): FetchLike {
  if (customFetch) {
    return customFetch;
  }

  if (typeof globalThis.fetch === 'function') {
    return globalThis.fetch.bind(globalThis);
  }

  throw new LniError('InvalidInput', 'No fetch implementation found. Pass fetch via NodeRequestOptions.');
}

export function buildUrl(baseUrl: string, path: string, query?: Record<string, QueryValue>): string {
  const normalizedBase = baseUrl.endsWith('/') ? baseUrl : `${baseUrl}/`;
  const normalizedPath = path.startsWith('/') ? path.slice(1) : path;
  const url = new URL(normalizedPath, normalizedBase);

  if (query) {
    for (const [key, value] of Object.entries(query)) {
      if (value === undefined || value === null || value === '') {
        continue;
      }
      url.searchParams.set(key, String(value));
    }
  }

  return url.toString();
}

function encodeForm(form: Record<string, QueryValue>): URLSearchParams {
  const params = new URLSearchParams();
  for (const [key, value] of Object.entries(form)) {
    if (value === undefined || value === null) {
      continue;
    }
    params.set(key, String(value));
  }
  return params;
}

function withTimeout(signal: AbortSignal | undefined, timeoutMs: number | undefined): AbortSignal | undefined {
  if (!timeoutMs || timeoutMs <= 0) {
    return signal;
  }

  const controller = new AbortController();
  const timeoutId = setTimeout(() => controller.abort(), timeoutMs);

  if (signal) {
    if (signal.aborted) {
      clearTimeout(timeoutId);
      controller.abort();
    } else {
      signal.addEventListener(
        'abort',
        () => {
          clearTimeout(timeoutId);
          controller.abort();
        },
        { once: true },
      );
    }
  }

  controller.signal.addEventListener(
    'abort',
    () => {
      clearTimeout(timeoutId);
    },
    { once: true },
  );

  return controller.signal;
}

export async function requestText(fetchFn: FetchLike, url: string, args: RequestArgs = {}): Promise<string> {
  const headers = new Headers(args.headers);
  let body: BodyInit | null | undefined = args.body;

  if (args.json !== undefined) {
    if (!headers.has('content-type')) {
      headers.set('content-type', 'application/json');
    }
    body = JSON.stringify(args.json);
  } else if (args.form) {
    if (!headers.has('content-type')) {
      headers.set('content-type', 'application/x-www-form-urlencoded');
    }
    body = encodeForm(args.form).toString();
  }

  const signal = withTimeout(args.signal, args.timeoutMs);

  let response: Response;
  try {
    response = await fetchFn(url, {
      method: args.method ?? (body ? 'POST' : 'GET'),
      headers,
      body,
      signal,
    });
  } catch (error) {
    throw new LniError('NetworkError', `Network request failed: ${(error as Error)?.message ?? 'unknown error'}`, {
      cause: error,
    });
  }

  const text = await response.text();

  if (!response.ok) {
    throw new LniError('Http', `HTTP ${response.status}: ${text || response.statusText}`, {
      status: response.status,
      body: text,
    });
  }

  return text;
}

export async function requestJson<T>(fetchFn: FetchLike, url: string, args: RequestArgs = {}): Promise<T> {
  const text = await requestText(fetchFn, url, args);

  if (!text) {
    return {} as T;
  }

  try {
    return JSON.parse(text) as T;
  } catch (error) {
    throw new LniError('Json', `Failed to parse JSON response: ${(error as Error)?.message ?? 'unknown error'}`, {
      body: text,
      cause: error,
    });
  }
}

export async function requestMaybeJson<T>(fetchFn: FetchLike, url: string, args: RequestArgs = {}): Promise<T | string> {
  const text = await requestText(fetchFn, url, args);

  if (!text) {
    return '';
  }

  try {
    return JSON.parse(text) as T;
  } catch {
    return text;
  }
}

export function toTimeoutMs(timeoutSeconds?: number): number | undefined {
  if (!timeoutSeconds || timeoutSeconds <= 0) {
    return undefined;
  }
  return timeoutSeconds * 1000;
}
