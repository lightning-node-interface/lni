import { afterEach, describe, expect, it, vi } from 'vitest';
import { ClnNode } from '../nodes/cln.js';
import { LndNode } from '../nodes/lnd.js';
import type { FetchLike } from '../types.js';

const originalNavigatorDescriptor = Object.getOwnPropertyDescriptor(globalThis, 'navigator');

function useReactNativeRuntime(): void {
  Object.defineProperty(globalThis, 'navigator', {
    configurable: true,
    value: { product: 'ReactNative' },
  });
}

afterEach(() => {
  if (originalNavigatorDescriptor) {
    Object.defineProperty(globalThis, 'navigator', originalNavigatorDescriptor);
  } else {
    delete (globalThis as { navigator?: Navigator }).navigator;
  }
});

describe('HTTP redirect policy', () => {
  it('disables redirects for CLN requests carrying a rune', async () => {
    const inits: RequestInit[] = [];
    const fetchMock = vi.fn<FetchLike>(async (_input, init) => {
      inits.push(init ?? {});
      return new Response('redirect blocked', { status: 500 });
    });
    const node = new ClnNode({ url: 'https://cln.test', rune: 'fake-rune' }, { fetch: fetchMock });

    await node.getInfo().catch(() => undefined);

    expect(inits.length).toBeGreaterThan(0);
    expect(inits.every((init) => init.redirect === 'error')).toBe(true);
  });

  it('disables redirects for LND requests carrying a macaroon', async () => {
    const inits: RequestInit[] = [];
    const fetchMock = vi.fn<FetchLike>(async (_input, init) => {
      inits.push(init ?? {});
      return new Response('redirect blocked', { status: 500 });
    });
    const node = new LndNode(
      { url: 'https://lnd.test', macaroon: 'fake-macaroon' },
      { fetch: fetchMock }
    );

    await node.getInfo().catch(() => undefined);

    expect(inits.length).toBeGreaterThan(0);
    expect(inits.every((init) => init.redirect === 'error')).toBe(true);
  });

  it('rejects legacy React Native fetch before sending credentials', () => {
    useReactNativeRuntime();
    const fetchMock = vi.fn<FetchLike>();

    expect(
      () => new ClnNode({ url: 'https://cln.test', rune: 'fake-rune' }, { fetch: fetchMock })
    ).toThrow(/legacy fetch cannot reject redirects/);
    expect(fetchMock).not.toHaveBeenCalled();
  });

  it('accepts an explicitly redirect-capable React Native fetch', async () => {
    useReactNativeRuntime();
    const inits: RequestInit[] = [];
    const fetchMock = vi.fn<FetchLike>(async (_input, init) => {
      inits.push(init ?? {});
      return new Response('request failed', { status: 500 });
    });
    const node = new ClnNode(
      { url: 'https://cln.test', rune: 'fake-rune' },
      { fetch: fetchMock, fetchSupportsRedirectError: true }
    );

    await node.getInfo().catch(() => undefined);

    expect(inits.length).toBeGreaterThan(0);
    expect(inits.every((init) => init.redirect === 'error')).toBe(true);
  });
});
