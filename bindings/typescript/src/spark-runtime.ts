import { Buffer as NodeBuffer } from 'buffer';
import { LniError } from './errors.js';
import type { FetchLike } from './types.js';

type FetchRuntime = typeof globalThis & {
  Buffer?: typeof NodeBuffer;
  btoa?: (value: string) => string;
  atob?: (value: string) => string;
  fetch?: FetchLike;
};

export interface SparkRuntimeOptions {
  fetch?: FetchLike;
  apiKey?: string;
  apiKeyHeader?: string;
  apiKeySameOriginOnly?: boolean;
  setGlobalFetch?: boolean;
  installBuffer?: boolean;
  installBase64?: boolean;
  useStreamCompat?: boolean;
}

export interface SparkRuntimeHandle {
  fetch: FetchLike;
  restore: () => void;
}

function getRequestUrl(input: RequestInfo | URL): URL | null {
  try {
    if (typeof input === 'string') {
      if (typeof window !== 'undefined' && window.location?.href) {
        return new URL(input, window.location.href);
      }
      return new URL(input);
    }
    if (input instanceof URL) {
      return input;
    }
    if (typeof input === 'object' && input && 'url' in input && typeof input.url === 'string') {
      return new URL(input.url);
    }
  } catch {
    return null;
  }
  return null;
}

function shouldAttachApiKeyHeader(
  input: RequestInfo | URL,
  sameOriginOnly: boolean,
): boolean {
  if (!sameOriginOnly) {
    return true;
  }

  if (typeof window === 'undefined' || !window.location?.origin) {
    return true;
  }

  const requestUrl = getRequestUrl(input);
  if (!requestUrl) {
    return false;
  }

  return requestUrl.origin === window.location.origin;
}

function createFallbackReaderResponse(response: Response): Response {
  let reader:
    | {
        read(): Promise<{ done: boolean; value?: Uint8Array }>;
        cancel(): Promise<void>;
      }
    | null = null;

  const fallbackBody = {
    getReader() {
      if (reader) {
        return reader;
      }

      let offset = 0;
      let bytesPromise: Promise<Uint8Array> | null = null;

      const loadBytes = async (): Promise<Uint8Array> => {
        if (!bytesPromise) {
          bytesPromise = response.arrayBuffer().then((buffer) => new Uint8Array(buffer));
        }
        return bytesPromise;
      };

      reader = {
        async read() {
          const bytes = await loadBytes();
          if (offset >= bytes.length) {
            return { done: true };
          }
          const chunk = bytes.subarray(offset);
          offset = bytes.length;
          return { done: false, value: chunk };
        },
        async cancel() {
          offset = Number.MAX_SAFE_INTEGER;
        },
      };

      return reader;
    },
  };

  return new Proxy(response, {
    get(target, property, receiver) {
      if (property === 'body') {
        return fallbackBody as unknown;
      }
      return Reflect.get(target, property, receiver);
    },
  });
}

export function createStreamCompatibleFetch(baseFetch: FetchLike): FetchLike {
  return (async (input: RequestInfo | URL, init?: RequestInit): Promise<Response> => {
    const response = await baseFetch(input, init);
    const body = (response as { body?: { getReader?: () => unknown } }).body;

    if (body && typeof body.getReader === 'function') {
      return response;
    }

    return createFallbackReaderResponse(response);
  }) as FetchLike;
}

export function withHeaderFetch(
  baseFetch: FetchLike,
  headerName: string,
  headerValue: string,
  options: {
    sameOriginOnly?: boolean;
  } = {},
): FetchLike {
  const trimmedValue = headerValue.trim();
  if (!trimmedValue) {
    return baseFetch;
  }

  return (input: RequestInfo | URL, init: RequestInit = {}) => {
    const sameOriginOnly = options.sameOriginOnly === true;
    if (!shouldAttachApiKeyHeader(input, sameOriginOnly)) {
      return baseFetch(input, init);
    }

    const headers = new Headers(init.headers ?? {});
    headers.set(headerName, trimmedValue);
    return baseFetch(input, {
      ...init,
      headers,
    });
  };
}

export function installSparkRuntime(options: SparkRuntimeOptions = {}): SparkRuntimeHandle {
  const runtime = globalThis as FetchRuntime;
  const previousFetch = runtime.fetch;

  let baseFetch = options.fetch ?? runtime.fetch;
  if (!baseFetch) {
    throw new LniError('InvalidInput', 'Spark runtime requires a fetch implementation.');
  }

  if (options.installBuffer !== false && typeof runtime.Buffer !== 'function') {
    runtime.Buffer = NodeBuffer;
  }

  if (options.installBase64 !== false && typeof runtime.Buffer === 'function') {
    if (typeof runtime.btoa !== 'function') {
      runtime.btoa = (value: string) => runtime.Buffer!.from(value, 'binary').toString('base64');
    }
    if (typeof runtime.atob !== 'function') {
      runtime.atob = (value: string) => runtime.Buffer!.from(value, 'base64').toString('binary');
    }
  }

  if (options.useStreamCompat !== false) {
    baseFetch = createStreamCompatibleFetch(baseFetch);
  }

  if (options.apiKey?.trim()) {
    baseFetch = withHeaderFetch(
      baseFetch,
      options.apiKeyHeader?.trim() || 'x-api-key',
      options.apiKey,
      {
        sameOriginOnly: options.apiKeySameOriginOnly !== false,
      },
    );
  }

  if (options.setGlobalFetch !== false) {
    runtime.fetch = baseFetch;
  }

  return {
    fetch: baseFetch,
    restore() {
      if (options.setGlobalFetch !== false) {
        runtime.fetch = previousFetch;
      }
    },
  };
}
