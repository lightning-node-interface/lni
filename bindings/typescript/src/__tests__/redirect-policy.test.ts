import { afterEach, describe, expect, it, vi } from 'vitest';
import { ClnNode } from '../nodes/cln.js';
import { createGaloyNode } from '../nodes/galoy.js';
import { LndNode } from '../nodes/lnd.js';
import { PhoenixdNode } from '../nodes/phoenixd.js';
import { SpeedNode } from '../nodes/speed.js';
import type { FetchLike, LightningNode, NodeRequestOptions } from '../types.js';

const originalNavigatorDescriptor = Object.getOwnPropertyDescriptor(globalThis, 'navigator');
const originalFetchDescriptor = Object.getOwnPropertyDescriptor(globalThis, 'fetch');

function useReactNativeRuntime(): void {
  Object.defineProperty(globalThis, 'navigator', {
    configurable: true,
    value: { product: 'ReactNative' },
  });
}

const additionalNodeFactories: Array<{
  name: string;
  create: (options: NodeRequestOptions) => LightningNode;
}> = [
  {
    name: 'Galoy',
    create: (options) =>
      createGaloyNode(
        {
          apiKey: 'fake-api-key',
          baseUrl: 'https://galoy.test/graphql',
          provider: { id: 'galoy', name: 'Galoy' },
          wallet: { mode: 'currency', currency: 'BTC' },
          invoiceOperations: {
            create: { kind: 'unsupported' },
            feeProbe: { kind: 'unsupported' },
          },
          payment: { response: 'status-only', acceptedStatuses: ['SUCCESS'] },
          capabilities: {
            transactionLookup: false,
            transactionHistory: false,
            invoiceEvents: false,
            onchain: false,
          },
          permissions: 'configured',
        },
        options
      ),
  },
  {
    name: 'Phoenixd',
    create: (options) =>
      new PhoenixdNode({ url: 'https://phoenixd.test', password: 'fake-password' }, options),
  },
  {
    name: 'Speed',
    create: (options) =>
      new SpeedNode({ baseUrl: 'https://speed.test', apiKey: 'fake-api-key' }, options),
  },
];

afterEach(() => {
  if (originalNavigatorDescriptor) {
    Object.defineProperty(globalThis, 'navigator', originalNavigatorDescriptor);
  } else {
    delete (globalThis as { navigator?: Navigator }).navigator;
  }

  if (originalFetchDescriptor) {
    Object.defineProperty(globalThis, 'fetch', originalFetchDescriptor);
  } else {
    delete (globalThis as { fetch?: typeof fetch }).fetch;
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

  it('does not reject a React Native runtime at construction', () => {
    useReactNativeRuntime();
    const fetchMock = vi.fn<FetchLike>();

    expect(
      () => new ClnNode({ url: 'https://cln.test', rune: 'fake-rune' }, { fetch: fetchMock })
    ).not.toThrow();
  });

  it('uses a redirect-capable global fetch in React Native without a capability flag', async () => {
    useReactNativeRuntime();
    const inits: RequestInit[] = [];
    const fetchMock = vi.fn<FetchLike>(async (_input, init) => {
      inits.push(init ?? {});
      return new Response('request failed', { status: 500 });
    });
    Object.defineProperty(globalThis, 'fetch', {
      configurable: true,
      writable: true,
      value: fetchMock,
    });
    const node = new ClnNode({ url: 'https://cln.test', rune: 'fake-rune' });

    await node.getInfo().catch(() => undefined);

    expect(inits.length).toBeGreaterThan(0);
    expect(inits.every((init) => init.redirect === 'error')).toBe(true);
  });

  it.each(additionalNodeFactories)(
    'uses redirect:error for $name in React Native without a capability flag',
    async ({ create }) => {
      useReactNativeRuntime();
      const inits: RequestInit[] = [];
      const fetchMock = vi.fn<FetchLike>(async (_input, init) => {
        inits.push(init ?? {});
        return new Response('request failed', { status: 500 });
      });
      const node = create({ fetch: fetchMock });

      await node.getInfo().catch(() => undefined);

      expect(inits.length).toBeGreaterThan(0);
      expect(inits.every((init) => init.redirect === 'error')).toBe(true);
    }
  );
});
