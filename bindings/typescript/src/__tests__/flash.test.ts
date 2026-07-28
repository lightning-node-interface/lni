import { describe, expect, it, vi } from 'vitest';
import { FlashNode, createNode } from '../index.js';
import type { FetchLike } from '../types.js';

const BOLT11 =
  'lnbc2500u1pvjluezsp5zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zygspp5qqqsyqcyq5rqwzqfqqqsyqcyq5rqwzqfqqqsyqcyq5rqwzqfqypqdq5xysxxatsyp3k7enxv4jsxqzpu9qrsgquk0rl77nj30yxdy8j9vdx85fkpmdla2087ne0xh8nhedh8w27kyke0lp53ut353s06fv3qfegext0eh0ymjpf39tuven09sam30g4vgpfna3rh';
const PAYMENT_HASH = '0001020304050607080900010203040506070809000102030405060708090102';

function response(body: unknown): Response {
  return new Response(JSON.stringify(body), {
    headers: { 'content-type': 'application/json' },
  });
}

function requestBody(init?: RequestInit): {
  query: string;
  variables: { input: Record<string, unknown> };
} {
  return JSON.parse(String(init?.body));
}

describe('FlashNode', () => {
  it('is a thin explicit-wallet, status-only Galoy wrapper', async () => {
    const queries: string[] = [];
    const fetchMock = vi.fn<FetchLike>(async (input, init) => {
      expect(String(input)).toBe('https://api.flashapp.me/graphql');
      const body = requestBody(init);
      queries.push(body.query);
      expect(body.variables.input.walletId).toBe('wallet-jmd');

      const headers = new Headers(init?.headers);
      expect(headers.get('x-api-key')).toBe('flash-key');
      expect(headers.get('content-type')).toBe('application/json');
      expect(headers.get('x-flash-client-capabilities')).toBe('proofless');

      if (body.query.includes('lnUsdInvoiceFeeProbe')) {
        return response({
          data: { lnUsdInvoiceFeeProbe: { amount: 50_000, errors: [] } },
        });
      }
      return response({
        data: { lnInvoicePaymentSend: { status: 'PENDING', errors: [] } },
      });
    });

    const node = createNode(
      {
        kind: 'flash',
        config: {
          apiKey: 'flash-key',
          walletId: 'wallet-jmd',
          walletCurrency: 'JMD',
          additionalHeaders: {
            'x-flash-client-capabilities': 'proofless',
            'x-api-key': 'cannot-override',
            'content-type': 'text/plain',
          },
        },
      },
      { fetch: fetchMock }
    );

    expect(node).toBeInstanceOf(FlashNode);
    await expect(node.payInvoice({ invoice: BOLT11 })).resolves.toEqual({
      paymentHash: PAYMENT_HASH,
      preimage: '',
      feeMsats: 0,
    });
    expect(queries).toHaveLength(2);
    expect(queries.join('\n')).not.toContain('query Me');
    expect(queries[0]).toContain('lnUsdInvoiceFeeProbe');
    expect(queries[1]).not.toMatch(/\btransaction\s*\{/);
  });

  it('uses configured permissions with optional Galoy capabilities disabled', async () => {
    const node = new FlashNode({
      apiKey: 'flash-key',
      baseUrl: 'https://flash.test/graphql',
      walletId: 'wallet-jmd',
      walletCurrency: 'JMD',
    });

    await expect(node.getPermissions()).resolves.toMatchObject({
      getInfo: true,
      createInvoice: true,
      payInvoice: true,
      lookupInvoice: false,
      listTransactions: false,
      onInvoiceEvents: false,
    });
  });

  it('allows the default Flash GraphQL endpoint to be overridden', async () => {
    const fetchMock = vi.fn<FetchLike>(async (input) => {
      expect(String(input)).toBe('https://flash.test/custom-graphql');
      return response({
        data: {
          lnUsdInvoiceCreate: {
            invoice: {
              paymentRequest: BOLT11,
              paymentHash: PAYMENT_HASH,
              satoshis: 1,
            },
            errors: [],
          },
        },
      });
    });
    const node = new FlashNode(
      {
        apiKey: 'flash-key',
        baseUrl: 'https://flash.test/custom-graphql',
        walletId: 'wallet-jmd',
        walletCurrency: 'JMD',
      },
      { fetch: fetchMock }
    );

    await node.createInvoice({ amountMsats: 1_000 });
  });
});
