import { afterEach, describe, expect, it, vi } from 'vitest';
import { createStreamCompatibleFetch, installSparkRuntime, withHeaderFetch } from '../spark-runtime.js';
import type { FetchLike } from '../types.js';

const globalRuntime = globalThis as typeof globalThis & { fetch?: typeof fetch };

const initialFetch = globalRuntime.fetch;

afterEach(() => {
  globalRuntime.fetch = initialFetch;
});

describe('spark-runtime helpers', () => {
  it('adds reader fallback when response.body is missing', async () => {
    const response = new Response('hello');
    Object.defineProperty(response, 'body', {
      value: undefined,
      configurable: true,
    });

    const fetchMock = vi.fn<FetchLike>(async () => response);
    const wrapped = createStreamCompatibleFetch(fetchMock as unknown as typeof fetch);

    const wrappedResponse = await wrapped('https://example.com');
    const body = (wrappedResponse as unknown as { body?: { getReader?: () => unknown } }).body;

    expect(body).toBeDefined();
    expect(typeof body?.getReader).toBe('function');

    const reader = (body as { getReader: () => { read: () => Promise<{ done: boolean; value?: Uint8Array }> } }).getReader();

    const chunk = await reader.read();
    expect(chunk.done).toBe(false);
    expect(new TextDecoder().decode(chunk.value)).toBe('hello');

    const done = await reader.read();
    expect(done.done).toBe(true);
  });

  it('injects header via withHeaderFetch', async () => {
    const fetchMock = vi.fn<FetchLike>(async () => new Response('{}'));
    const wrapped = withHeaderFetch(
      fetchMock as unknown as typeof fetch,
      'x-api-key',
      'demo-key',
    );

    await wrapped('https://example.com');

    const init = (fetchMock.mock.calls[0]?.[1] ?? {}) as RequestInit;
    const headers = new Headers(init.headers);
    expect(headers.get('x-api-key')).toBe('demo-key');
  });

  it('installs and restores global fetch', async () => {
    const fetchMock = vi.fn<FetchLike>(async () => new Response('{}'));
    const handle = installSparkRuntime({
      fetch: fetchMock as unknown as typeof fetch,
      apiKey: 'abc',
      useStreamCompat: false,
    });

    expect(globalRuntime.fetch).toBe(handle.fetch);

    expect(globalRuntime.fetch).toBeDefined();
    await globalRuntime.fetch!('https://example.com');
    const init = (fetchMock.mock.calls[0]?.[1] ?? {}) as RequestInit;
    const headers = new Headers(init.headers);
    expect(headers.get('x-api-key')).toBe('abc');

    handle.restore();
    expect(globalRuntime.fetch).toBe(initialFetch);
  });
});
