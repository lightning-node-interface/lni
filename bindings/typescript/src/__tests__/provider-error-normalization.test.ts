import { describe, expect, it, vi } from 'vitest';
import { ClnNode } from '../nodes/cln.js';
import { BlinkNode } from '../nodes/blink.js';
import { LndNode } from '../nodes/lnd.js';
import { PhoenixdNode } from '../nodes/phoenixd.js';
import { SpeedNode } from '../nodes/speed.js';
import type { FetchLike } from '../types.js';

function jsonResponse(body: unknown, init?: ResponseInit): Response {
  return new Response(JSON.stringify(body), {
    ...init,
    headers: {
      'content-type': 'application/json',
      ...(init?.headers ?? {}),
    },
  });
}

describe('provider error normalization', () => {
  it('maps CLN pay route failures to payment failed', async () => {
    const fetchMock = vi.fn<FetchLike>(async () =>
      jsonResponse(
        {
          error: {
            code: 205,
            message: 'Unable to find a route',
          },
        },
        { status: 500 },
      ),
    );
    const node = new ClnNode({ url: 'https://cln.test', rune: 'test-rune' }, { fetch: fetchMock });

    await expect(node.payInvoice({ invoice: 'lnbc1testinvoice' })).rejects.toMatchObject({
      name: 'NwcError',
      nwcCode: 'PAYMENT_FAILED',
      operation: 'pay_invoice',
      provider: 'cln',
      providerCode: 205,
    });
  });

  it('maps Blink top-level authorization errors to unauthorized', async () => {
    const fetchMock = vi.fn<FetchLike>(async () =>
      jsonResponse({
        errors: [
          {
            code: 'UNAUTHORIZED',
            message: 'Not authorized',
            path: ['me'],
          },
        ],
      }),
    );
    const node = new BlinkNode({ apiKey: 'bad-token', baseUrl: 'https://blink.test/graphql' }, { fetch: fetchMock });

    await expect(node.getInfo()).rejects.toMatchObject({
      name: 'NwcError',
      nwcCode: 'UNAUTHORIZED',
      operation: 'get_info',
      provider: 'blink',
      providerCode: 'UNAUTHORIZED',
    });
  });

  it('maps LND router insufficient-balance failures to insufficient balance', async () => {
    const fetchMock = vi.fn<FetchLike>(async () =>
      jsonResponse({
        result: {
          payment_hash: '',
          payment_preimage: '',
          fee_msat: '0',
          status: 'FAILED',
          failure_reason: 'FAILURE_REASON_INSUFFICIENT_BALANCE',
        },
      }),
    );
    const node = new LndNode({ url: 'https://lnd.test', macaroon: '00' }, { fetch: fetchMock });

    await expect(node.payInvoice({ invoice: 'lnbc1testinvoice' })).rejects.toMatchObject({
      name: 'NwcError',
      nwcCode: 'INSUFFICIENT_BALANCE',
      operation: 'pay_invoice',
      provider: 'lnd',
      providerCode: 'FAILURE_REASON_INSUFFICIENT_BALANCE',
    });
  });

  it('maps Phoenixd lookup 404s to not found', async () => {
    const fetchMock = vi.fn<FetchLike>(async () => new Response('Not found', { status: 404 }));
    const node = new PhoenixdNode({ url: 'https://phoenixd.test', password: 'secret' }, { fetch: fetchMock });

    await expect(node.lookupInvoice({ paymentHash: 'hash-1' })).rejects.toMatchObject({
      name: 'NwcError',
      nwcCode: 'NOT_FOUND',
      operation: 'lookup_invoice',
      provider: 'phoenixd',
      providerStatus: 404,
    });
  });

  it('maps Speed failed sends to payment failed', async () => {
    const fetchMock = vi.fn<FetchLike>(async () =>
      jsonResponse({
        id: 'send-1',
        status: 'failed',
        target_amount: 1,
        withdraw_method: 'lightning',
        withdraw_request: 'lnbc1testinvoice',
        created: 1,
        speed_fee: { amount: 0 },
      }),
    );
    const node = new SpeedNode({ apiKey: 'sk_test', baseUrl: 'https://speed.test' }, { fetch: fetchMock });

    await expect(node.payInvoice({ invoice: 'lnbc1testinvoice', amountMsats: 1000 })).rejects.toMatchObject({
      name: 'NwcError',
      nwcCode: 'PAYMENT_FAILED',
      operation: 'pay_invoice',
      provider: 'speed',
      providerCode: 'failed',
    });
  });
});
