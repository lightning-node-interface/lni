import { describe, expect, it, vi } from 'vitest';
import { NwcError } from '../errors.js';
import { StrikeNode } from '../nodes/strike.js';
import type { FetchLike } from '../types.js';

const BOLT11 =
  'lnbc2500u1pvjluezsp5zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zygspp5qqqsyqcyq5rqwzqfqqqsyqcyq5rqwzqfqqqsyqcyq5rqwzqfqypqdq5xysxxatsyp3k7enxv4jsxqzpu9qrsgquk0rl77nj30yxdy8j9vdx85fkpmdla2087ne0xh8nhedh8w27kyke0lp53ut353s06fv3qfegext0eh0ymjpf39tuven09sam30g4vgpfna3rh';

function jsonResponse(body: unknown, init?: ResponseInit): Response {
  return new Response(JSON.stringify(body), {
    ...init,
    headers: {
      'content-type': 'application/json',
      ...(init?.headers ?? {}),
    },
  });
}

describe('StrikeNode error normalization', () => {
  it('maps Strike insufficient-balance payment quote errors to NwcError', async () => {
    const fetchMock = vi.fn<FetchLike>(async (input) => {
      const url = String(input);

      if (url === 'https://api.strike.test/v1/payment-quotes/lightning') {
        return jsonResponse(
          {
            traceId: 'trace-1',
            data: {
              status: '422',
              code: 'BALANCE_TOO_LOW',
              message: 'Insufficient funds',
            },
          },
          { status: 422 }
        );
      }

      return new Response('not found', { status: 404 });
    });

    const node = new StrikeNode(
      { apiKey: 'test-token', baseUrl: 'https://api.strike.test/v1' },
      { fetch: fetchMock }
    );

    const payment = node.payInvoice({ invoice: 'lnbc1testinvoice' });

    await expect(payment).rejects.toMatchObject({
      name: 'NwcError',
      code: 'NwcError',
      nwcCode: 'INSUFFICIENT_BALANCE',
      operation: 'pay_invoice',
      provider: 'strike',
      providerCode: 'BALANCE_TOO_LOW',
      providerStatus: 422,
      providerMessage: 'Insufficient funds',
    });
    await expect(payment).rejects.toBeInstanceOf(NwcError);
  });

  it('maps unsupported Bolt12 flows to not implemented NWC errors', async () => {
    const node = new StrikeNode(
      { apiKey: 'test-token', baseUrl: 'https://api.strike.test/v1' },
      { fetch: vi.fn<FetchLike>() }
    );

    await expect(node.createOffer({})).rejects.toMatchObject({
      name: 'NwcError',
      nwcCode: 'NOT_IMPLEMENTED',
      operation: 'make_invoice',
      provider: 'strike',
    });
  });

  it('maps Strike invalid invoice errors to payment failures', async () => {
    const fetchMock = vi.fn<FetchLike>(async (input) => {
      const url = String(input);

      if (url === 'https://api.strike.test/v1/payment-quotes/lightning') {
        return jsonResponse(
          {
            data: {
              status: 422,
              code: 'INVALID_LN_INVOICE',
              message: 'Invalid lightning invoice.',
            },
          },
          { status: 422 }
        );
      }

      return new Response('not found', { status: 404 });
    });

    const node = new StrikeNode(
      { apiKey: 'test-token', baseUrl: 'https://api.strike.test/v1' },
      { fetch: fetchMock }
    );

    await expect(node.payInvoice({ invoice: 'not-an-invoice' })).rejects.toMatchObject({
      nwcCode: 'PAYMENT_FAILED',
      operation: 'pay_invoice',
      provider: 'strike',
      providerCode: 'INVALID_LN_INVOICE',
      providerStatus: 422,
    });
  });

  it('falls back from Strike HTTP status when the error body is not structured JSON', async () => {
    const fetchMock = vi.fn<FetchLike>(async (input) => {
      const url = String(input);

      if (url === 'https://api.strike.test/v1/balances') {
        return new Response('Invalid or unspecified identity.', { status: 401 });
      }

      return new Response('not found', { status: 404 });
    });

    const node = new StrikeNode(
      { apiKey: 'bad-token', baseUrl: 'https://api.strike.test/v1' },
      { fetch: fetchMock }
    );

    await expect(node.getInfo()).rejects.toMatchObject({
      name: 'NwcError',
      nwcCode: 'UNAUTHORIZED',
      operation: 'get_info',
      provider: 'strike',
      providerStatus: 401,
    });
  });
});

describe('StrikeNode Lightning payments', () => {
  it('preserves a preimage returned by execute when payment.read is unavailable', async () => {
    const fetchMock = vi.fn<FetchLike>(async (input) => {
      const url = String(input);

      if (url === 'https://api.strike.test/v1/payment-quotes/lightning') {
        return jsonResponse({ paymentQuoteId: 'quote-1' });
      }

      if (url === 'https://api.strike.test/v1/payment-quotes/quote-1/execute') {
        return jsonResponse({
          paymentId: 'payment-1',
          lightning: {
            preImage: 'execute-preimage',
            networkFee: { amount: '0.00000001', currency: 'BTC' },
          },
        });
      }

      return new Response('forbidden', { status: 403 });
    });

    const node = new StrikeNode(
      { apiKey: 'test-token', baseUrl: 'https://api.strike.test/v1' },
      { fetch: fetchMock }
    );

    await expect(node.payInvoice({ invoice: BOLT11 })).resolves.toEqual({
      paymentHash: '0001020304050607080900010203040506070809000102030405060708090102',
      preimage: 'execute-preimage',
      feeMsats: 1_000,
    });
    expect(fetchMock).toHaveBeenCalledTimes(2);
  });

  it('returns the settled preimage from the outgoing payment record', async () => {
    vi.useFakeTimers();
    let paymentReads = 0;

    try {
      const fetchMock = vi.fn<FetchLike>(async (input) => {
        const url = String(input);

        if (url === 'https://api.strike.test/v1/payment-quotes/lightning') {
          return jsonResponse({ paymentQuoteId: 'quote-1' });
        }

        if (url === 'https://api.strike.test/v1/payment-quotes/quote-1/execute') {
          return jsonResponse({ paymentId: 'payment-1' });
        }

        if (url === 'https://api.strike.test/v1/payments/payment-1') {
          paymentReads += 1;
          return jsonResponse({
            id: 'payment-1',
            state: 'COMPLETED',
            created: '2026-07-16T12:00:00Z',
            amount: { amount: '0.00002500', currency: 'BTC' },
            lightning: {
              paymentHash: 'provider-payment-hash',
              preImage: paymentReads > 1 ? 'settled-preimage' : undefined,
              networkFee: { amount: '0.00000001', currency: 'BTC' },
            },
          });
        }

        return new Response('not found', { status: 404 });
      });

      const node = new StrikeNode(
        { apiKey: 'test-token', baseUrl: 'https://api.strike.test/v1' },
        { fetch: fetchMock }
      );

      const paymentPromise = node.payInvoice({ invoice: BOLT11 });
      await vi.runAllTimersAsync();

      await expect(paymentPromise).resolves.toEqual({
        paymentHash: 'provider-payment-hash',
        preimage: 'settled-preimage',
        feeMsats: 1_000,
      });
      expect(paymentReads).toBe(2);
    } finally {
      vi.useRealTimers();
    }
  });

  it('retries transient outgoing payment record failures', async () => {
    vi.useFakeTimers();
    let paymentReads = 0;

    try {
      const fetchMock = vi.fn<FetchLike>(async (input) => {
        const url = String(input);

        if (url === 'https://api.strike.test/v1/payment-quotes/lightning') {
          return jsonResponse({ paymentQuoteId: 'quote-1' });
        }

        if (url === 'https://api.strike.test/v1/payment-quotes/quote-1/execute') {
          return jsonResponse({ paymentId: 'payment-1' });
        }

        if (url === 'https://api.strike.test/v1/payments/payment-1') {
          paymentReads += 1;
          if (paymentReads === 1) {
            return new Response('not found', { status: 404 });
          }
          if (paymentReads === 2) {
            return new Response('unavailable', { status: 503 });
          }
          if (paymentReads === 3) {
            return new Response('{not-json', { status: 200 });
          }
          return jsonResponse({
            id: 'payment-1',
            state: 'COMPLETED',
            created: '2026-07-16T12:00:00Z',
            amount: { amount: '0.00002500', currency: 'BTC' },
            lightning: {
              paymentHash: 'provider-payment-hash',
              preImage: 'settled-preimage',
              networkFee: { amount: '0.00000001', currency: 'BTC' },
            },
          });
        }

        return new Response('not found', { status: 404 });
      });

      const node = new StrikeNode(
        { apiKey: 'test-token', baseUrl: 'https://api.strike.test/v1' },
        { fetch: fetchMock }
      );

      const paymentPromise = node.payInvoice({ invoice: BOLT11 });
      await vi.runAllTimersAsync();

      await expect(paymentPromise).resolves.toEqual({
        paymentHash: 'provider-payment-hash',
        preimage: 'settled-preimage',
        feeMsats: 1_000,
      });
      expect(paymentReads).toBe(4);
    } finally {
      vi.useRealTimers();
    }
  });

  it('rejects when a pending payment remains indeterminate after polling', async () => {
    vi.useFakeTimers();
    const started = performance.now();

    try {
      const fetchMock = vi.fn<FetchLike>(async (input) => {
        const url = String(input);

        if (url === 'https://api.strike.test/v1/payment-quotes/lightning') {
          return jsonResponse({ paymentQuoteId: 'quote-1' });
        }

        if (url === 'https://api.strike.test/v1/payment-quotes/quote-1/execute') {
          return jsonResponse({ paymentId: 'payment-1', state: 'PENDING' });
        }

        return new Response('not found', { status: 404 });
      });

      const node = new StrikeNode(
        { apiKey: 'test-token', baseUrl: 'https://api.strike.test/v1' },
        { fetch: fetchMock }
      );

      const paymentPromise = node.payInvoice({ invoice: BOLT11 });
      const rejection = expect(paymentPromise).rejects.toMatchObject({
        name: 'NwcError',
        nwcCode: 'OTHER',
        operation: 'pay_invoice',
        provider: 'strike',
        providerCode: 'PENDING',
        providerMessage: JSON.stringify({ paymentId: 'payment-1', state: 'PENDING' }),
        message: expect.stringContaining('indeterminate'),
      });
      await vi.runAllTimersAsync();

      await rejection;
      expect(performance.now() - started).toBe(60_000);
    } finally {
      vi.useRealTimers();
    }
  });

  it.each([
    ['just before deadline', 999, 'success', 1],
    ['pending', 1000, 'OTHER', 2],
    ['stalled lookup', 1000, 'OTHER', 2],
    ['stalled body', 1000, 'OTHER', 2],
    ['failed', 400, 'PAYMENT_FAILED', 1],
    ['completed without proof', 1000, 'OTHER', 2],
    ['immediate proof', 0, 'success', 0],
  ] as const)('bounds settlement: %s', async (scenario, elapsed, outcome, reads) => {
    vi.useFakeTimers();
    let paymentReads = 0;
    let lookupSignal: AbortSignal | null | undefined;
    const state =
      scenario === 'failed' ? 'FAILED' : scenario === 'pending' ? 'PENDING' : 'COMPLETED';
    try {
      const started = performance.now();
      const fetchMock = vi.fn<FetchLike>(async (input, init) => {
        const url = String(input);
        if (url.endsWith('/payment-quotes/lightning'))
          return jsonResponse({ paymentQuoteId: 'quote-1' });
        if (url.endsWith('/execute'))
          return jsonResponse({
            paymentId: 'payment-1',
            state: 'PENDING',
            lightning: scenario === 'immediate proof' ? { preImage: 'test-proof' } : undefined,
          });
        paymentReads++;
        lookupSignal = init?.signal;
        if (scenario === 'just before deadline') {
          await new Promise((resolve) => setTimeout(resolve, 599));
          return jsonResponse({ id: 'payment-1', state, lightning: { preImage: 'test-proof' } });
        }
        if (scenario === 'stalled lookup' && paymentReads === 2) {
          return new Promise<Response>(() => {}); // Deliberately ignores abort.
        }
        if (scenario === 'stalled body' && paymentReads === 2) {
          const response = jsonResponse({});
          vi.spyOn(response, 'text').mockImplementation(() => new Promise(() => {}));
          return response;
        }
        return jsonResponse({ id: 'payment-1', state });
      });
      const node = new StrikeNode(
        {
          apiKey: 'test-token',
          baseUrl: 'https://api.strike.test/v1',
          paymentSettlementTimeout: 1,
          httpTimeout: 30,
        },
        { fetch: fetchMock }
      );
      const result = node.payInvoice({ invoice: BOLT11 });
      const assertion =
        outcome === 'success'
          ? expect(result).resolves.toMatchObject({ preimage: 'test-proof' })
          : expect(result).rejects.toMatchObject({
              nwcCode: outcome,
              providerCode: state,
              providerMessage: JSON.stringify({ paymentId: 'payment-1', state }),
            });
      await vi.runAllTimersAsync();
      await assertion;
      expect(performance.now() - started).toBe(elapsed);
      expect(paymentReads).toBe(reads);
      expect(vi.getTimerCount()).toBe(0);
      if (scenario.startsWith('stalled')) expect(lookupSignal?.aborted).toBe(true);
      await vi.advanceTimersByTimeAsync(2000);
      expect(paymentReads).toBe(reads);
    } finally {
      vi.useRealTimers();
    }
  });

  it.each(['proof', 'failed', 'completed without proof'] as const)(
    'preserves %s when a complete read wins at the deadline',
    async (scenario) => {
      vi.useFakeTimers();
      const started = performance.now();
      const state = scenario === 'failed' ? 'FAILED' : 'COMPLETED';
      try {
        const fetchMock = vi.fn<FetchLike>(async (input) => {
          if (String(input).endsWith('/lightning'))
            return jsonResponse({ paymentQuoteId: 'quote-1' });
          if (String(input).endsWith('/execute'))
            return jsonResponse({ paymentId: 'payment-1', state: 'PENDING' });
          const response = jsonResponse({
            id: 'payment-1',
            state,
            lightning: scenario === 'proof' ? { preImage: 'test-proof' } : undefined,
          });
          const text = await response.text();
          vi.spyOn(response, 'text').mockImplementation(async () => {
            // The body is ready; the deadline is reached before the caller resumes.
            // Timer callbacks have not won the race, so this record must survive.
            vi.spyOn(performance, 'now').mockReturnValue(started + 1000);
            return text;
          });
          return response;
        });
        const node = new StrikeNode(
          { apiKey: 'test-token', paymentSettlementTimeout: 1 },
          { fetch: fetchMock }
        );
        const result = node.payInvoice({ invoice: BOLT11 });
        const assertion =
          scenario === 'proof'
            ? expect(result).resolves.toMatchObject({ preimage: 'test-proof' })
            : expect(result).rejects.toMatchObject({
                nwcCode: scenario === 'failed' ? 'PAYMENT_FAILED' : 'OTHER',
                providerCode: state,
                providerMessage: JSON.stringify({ paymentId: 'payment-1', state }),
              });
        await vi.runAllTimersAsync();
        await assertion;
        expect(fetchMock).toHaveBeenCalledTimes(3);
        expect(vi.getTimerCount()).toBe(0);
      } finally {
        vi.restoreAllMocks();
        vi.useRealTimers();
      }
    }
  );

  it.each(['COMPLETED', 'FAILED'])(
    'keeps an unknown outcome when timeout beats a late %s record',
    async (state) => {
      vi.useFakeTimers();
      let finishRead: ((response: Response) => void) | undefined;
      let signal: AbortSignal | null | undefined;
      try {
        const fetchMock = vi.fn<FetchLike>(async (input, init) => {
          if (String(input).endsWith('/lightning'))
            return jsonResponse({ paymentQuoteId: 'quote-1' });
          if (String(input).endsWith('/execute'))
            return jsonResponse({ paymentId: 'payment-1', state: 'PENDING' });
          signal = init?.signal;
          return new Promise<Response>((resolve) => {
            finishRead = resolve;
          });
        });
        const node = new StrikeNode(
          { apiKey: 'test-token', paymentSettlementTimeout: 1 },
          { fetch: fetchMock }
        );
        const result = node.payInvoice({ invoice: BOLT11 });
        const assertion = expect(result).rejects.toMatchObject({
          nwcCode: 'OTHER',
          providerCode: 'PENDING',
        });
        await vi.advanceTimersByTimeAsync(1000);
        await assertion;
        expect(signal?.aborted).toBe(true);
        expect(finishRead).toBeDefined();
        finishRead!(
          jsonResponse({
            id: 'payment-1',
            state,
            lightning: state === 'COMPLETED' ? { preImage: 'test-proof' } : undefined,
          })
        );
        await vi.runAllTimersAsync();
        await expect(result).rejects.toMatchObject({ nwcCode: 'OTHER', providerCode: 'PENDING' });
        expect(fetchMock).toHaveBeenCalledTimes(3);
        expect(vi.getTimerCount()).toBe(0);
      } finally {
        vi.useRealTimers();
      }
    }
  );

  it.each([0, 1])('keeps request and settlement budgets separate (%s seconds)', async (budget) => {
    vi.useFakeTimers();
    const signals: AbortSignal[] = [];
    try {
      const started = performance.now();
      const fetchMock = vi.fn<FetchLike>(async (input, init) => {
        if (String(input).endsWith('/lightning'))
          return jsonResponse({ paymentQuoteId: 'quote-1' });
        if (String(input).endsWith('/execute'))
          return jsonResponse({ paymentId: 'payment-1', state: 'PENDING' });
        signals.push(init!.signal!);
        return new Promise<Response>(() => {});
      });
      const node = new StrikeNode(
        { apiKey: 'test-token', httpTimeout: 0.1, paymentSettlementTimeout: budget },
        { fetch: fetchMock }
      );
      const assertion = expect(node.payInvoice({ invoice: BOLT11 })).rejects.toMatchObject({
        nwcCode: 'OTHER',
        providerCode: 'PENDING',
        providerMessage: JSON.stringify({ paymentId: 'payment-1', state: 'PENDING' }),
      });
      await vi.runAllTimersAsync();
      await assertion;
      expect(performance.now() - started).toBe(budget * 1000);
      expect(signals).toHaveLength(budget === 0 ? 0 : 2);
      expect(signals.every((signal) => signal.aborted)).toBe(true);
      expect(vi.getTimerCount()).toBe(0);
    } finally {
      vi.useRealTimers();
    }
  });

  it('accepts the five-minute maximum settlement budget', () => {
    expect(
      () => new StrikeNode({ apiKey: 'test-token', paymentSettlementTimeout: 300 })
    ).not.toThrow();
  });

  it.each([-1, Infinity, NaN, 300.001, 301])(
    'rejects invalid settlement budget %s before payment',
    (value) => {
      expect(
        () => new StrikeNode({ apiKey: 'test-token', paymentSettlementTimeout: value })
      ).toThrow('paymentSettlementTimeout');
    }
  );

  it('keeps polling a pending payment until it settles', async () => {
    vi.useFakeTimers();
    let paymentReads = 0;

    try {
      const fetchMock = vi.fn<FetchLike>(async (input) => {
        const url = String(input);

        if (url === 'https://api.strike.test/v1/payment-quotes/lightning') {
          return jsonResponse({ paymentQuoteId: 'quote-1' });
        }

        if (url === 'https://api.strike.test/v1/payment-quotes/quote-1/execute') {
          return jsonResponse({ paymentId: 'payment-1', state: 'PENDING' });
        }

        if (url === 'https://api.strike.test/v1/payments/payment-1') {
          paymentReads += 1;
          // Settles ~8s in: well past the old five-attempt cap.
          const settled = paymentReads >= 20;
          return jsonResponse({
            id: 'payment-1',
            state: settled ? 'COMPLETED' : 'PENDING',
            created: '2026-07-16T12:00:00Z',
            amount: { amount: '0.00002500', currency: 'BTC' },
            lightning: {
              paymentHash: 'provider-payment-hash',
              preImage: settled ? 'settled-preimage' : undefined,
              networkFee: { amount: '0.00000001', currency: 'BTC' },
            },
          });
        }

        return new Response('not found', { status: 404 });
      });

      const node = new StrikeNode(
        { apiKey: 'test-token', baseUrl: 'https://api.strike.test/v1' },
        { fetch: fetchMock }
      );

      const paymentPromise = node.payInvoice({ invoice: BOLT11 });
      await vi.runAllTimersAsync();

      await expect(paymentPromise).resolves.toEqual({
        paymentHash: 'provider-payment-hash',
        preimage: 'settled-preimage',
        feeMsats: 1_000,
      });
      expect(paymentReads).toBe(20);
    } finally {
      vi.useRealTimers();
    }
  });

  it('stops polling as soon as the outgoing payment record fails', async () => {
    vi.useFakeTimers();
    let paymentReads = 0;

    try {
      const fetchMock = vi.fn<FetchLike>(async (input) => {
        const url = String(input);

        if (url === 'https://api.strike.test/v1/payment-quotes/lightning') {
          return jsonResponse({ paymentQuoteId: 'quote-1' });
        }

        if (url === 'https://api.strike.test/v1/payment-quotes/quote-1/execute') {
          return jsonResponse({ paymentId: 'payment-1', state: 'PENDING' });
        }

        if (url === 'https://api.strike.test/v1/payments/payment-1') {
          paymentReads += 1;
          return jsonResponse({
            id: 'payment-1',
            state: paymentReads >= 3 ? 'FAILED' : 'PENDING',
            created: '2026-07-16T12:00:00Z',
            amount: { amount: '0.00002500', currency: 'BTC' },
          });
        }

        return new Response('not found', { status: 404 });
      });

      const node = new StrikeNode(
        { apiKey: 'test-token', baseUrl: 'https://api.strike.test/v1' },
        { fetch: fetchMock }
      );

      const paymentPromise = node.payInvoice({ invoice: BOLT11 });
      const rejection = expect(paymentPromise).rejects.toMatchObject({
        name: 'NwcError',
        nwcCode: 'PAYMENT_FAILED',
        providerCode: 'FAILED',
        providerMessage: JSON.stringify({ paymentId: 'payment-1', state: 'FAILED' }),
      });
      await vi.runAllTimersAsync();

      await rejection;
      expect(paymentReads).toBe(3);
    } finally {
      vi.useRealTimers();
    }
  });

  it('rejects when Strike reports a failed payment without a preimage', async () => {
    vi.useFakeTimers();

    try {
      const fetchMock = vi.fn<FetchLike>(async (input) => {
        const url = String(input);

        if (url === 'https://api.strike.test/v1/payment-quotes/lightning') {
          return jsonResponse({ paymentQuoteId: 'quote-1' });
        }

        if (url === 'https://api.strike.test/v1/payment-quotes/quote-1/execute') {
          return jsonResponse({ paymentId: 'payment-1', state: 'FAILED' });
        }

        return new Response('not found', { status: 404 });
      });

      const node = new StrikeNode(
        { apiKey: 'test-token', baseUrl: 'https://api.strike.test/v1' },
        { fetch: fetchMock }
      );

      const paymentPromise = node.payInvoice({ invoice: BOLT11 });
      const rejection = expect(paymentPromise).rejects.toMatchObject({
        name: 'NwcError',
        nwcCode: 'PAYMENT_FAILED',
        operation: 'pay_invoice',
        provider: 'strike',
        message: expect.stringContaining('failed'),
      });
      await vi.runAllTimersAsync();

      await rejection;
    } finally {
      vi.useRealTimers();
    }
  });
});

describe('StrikeNode on-chain payments', () => {
  it('prepares an on-chain transaction using Strike tiers and fee policy', async () => {
    const fetchMock = vi.fn<FetchLike>(async (input, init) => {
      const url = String(input);
      const body = init?.body ? JSON.parse(String(init.body)) : undefined;

      if (url === 'https://api.strike.test/v1/payment-quotes/onchain/tiers') {
        expect(body).toEqual({
          btcAddress: 'bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh',
          amount: {
            amount: '0.00010000',
            currency: 'BTC',
          },
        });

        return jsonResponse([
          {
            id: 'tier_fast',
            estimatedDeliveryDurationInMin: 20,
            estimatedFee: { amount: '0.00002000', currency: 'BTC' },
          },
          {
            id: 'tier_standard',
            estimatedDeliveryDurationInMin: 60,
            estimatedFee: { amount: '0.00001000', currency: 'BTC' },
          },
        ]);
      }

      if (url === 'https://api.strike.test/v1/payment-quotes/onchain') {
        expect(Object.fromEntries(new Headers(init?.headers).entries())).toMatchObject({
          authorization: 'Bearer test-token',
          'content-type': 'application/json',
          'idempotency-key': '00000000-0000-4000-8000-000000000001',
        });
        expect(body).toEqual({
          btcAddress: 'bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh',
          sourceCurrency: 'BTC',
          description: 'cold storage',
          amount: {
            amount: '0.00010000',
            currency: 'BTC',
            feePolicy: 'INCLUSIVE',
          },
          onchainTierId: 'tier_standard',
        });

        return jsonResponse({
          paymentQuoteId: 'quote-1',
          estimatedDeliveryDurationInMin: 60,
          validUntil: '2026-05-29T12:34:56Z',
          amount: { amount: '0.00010000', currency: 'BTC' },
          totalFee: { amount: '0.00001000', currency: 'BTC' },
          totalAmount: { amount: '0.00011000', currency: 'BTC' },
        });
      }

      return new Response('not found', { status: 404 });
    });

    const node = new StrikeNode(
      { apiKey: 'test-token', baseUrl: 'https://api.strike.test/v1' },
      { fetch: fetchMock }
    );

    const transaction = await node.prepareOnchainTransaction({
      address: 'bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh',
      amountSats: 10_000,
      fee: { type: 'speed', speed: 'normal' },
      feePayer: 'recipient',
      description: 'cold storage',
      idempotencyKey: '00000000-0000-4000-8000-000000000001',
    });

    expect(transaction).toMatchObject({
      id: 'quote-1',
      address: 'bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh',
      amountSats: 10_000,
      feeSats: 1_000,
      totalAmountSats: 11_000,
      recipientAmountSats: 10_000,
      feePayer: 'recipient',
      fee: { type: 'speed', speed: 'normal' },
      estimatedDeliverySeconds: 3600,
    });
    expect(fetchMock).toHaveBeenCalledTimes(2);
  });

  it('recovers the original on-chain quote id when Strike reports a duplicate idempotency key', async () => {
    const fetchMock = vi.fn<FetchLike>(async (input) => {
      const url = String(input);

      if (url === 'https://api.strike.test/v1/payment-quotes/onchain/tiers') {
        return jsonResponse([
          {
            id: 'tier_standard',
            estimatedDeliveryDurationInMin: 60,
            estimatedFee: { amount: '0.00001000', currency: 'BTC' },
          },
        ]);
      }

      if (url === 'https://api.strike.test/v1/payment-quotes/onchain') {
        return jsonResponse(
          {
            code: 'DUPLICATE_PAYMENT_QUOTE',
            message: 'A payment quote for the specified idempotency key already exists.',
            data: {
              paymentQuoteId: 'quote-original',
            },
          },
          { status: 422 }
        );
      }

      return new Response('not found', { status: 404 });
    });

    const node = new StrikeNode(
      { apiKey: 'test-token', baseUrl: 'https://api.strike.test/v1' },
      { fetch: fetchMock }
    );

    const transaction = await node.prepareOnchainTransaction({
      address: 'bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh',
      amountSats: 10_000,
      fee: { type: 'speed', speed: 'normal' },
      feePayer: 'sender',
      idempotencyKey: '00000000-0000-4000-8000-000000000001',
    });

    expect(transaction).toMatchObject({
      id: 'quote-original',
      address: 'bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh',
      amountSats: 10_000,
      feePayer: 'sender',
      fee: { type: 'speed', speed: 'normal' },
    });
    expect(transaction.feeSats).toBeUndefined();
    expect(fetchMock).toHaveBeenCalledTimes(2);
  });

  it('executes an on-chain transaction and returns txid when available', async () => {
    const fetchMock = vi.fn<FetchLike>(async (input) => {
      const url = String(input);

      if (url === 'https://api.strike.test/v1/payment-quotes/quote-1/execute') {
        return jsonResponse({
          paymentId: 'payment-1',
          state: 'PENDING',
          amount: { amount: '0.00010000', currency: 'BTC' },
          totalFee: { amount: '0.00001000', currency: 'BTC' },
          totalAmount: { amount: '0.00011000', currency: 'BTC' },
        });
      }

      if (url === 'https://api.strike.test/v1/payments/payment-1') {
        return jsonResponse({
          paymentId: 'payment-1',
          state: 'COMPLETED',
          created: '2026-05-29T12:00:00Z',
          amount: { amount: '0.00010000', currency: 'BTC' },
          totalFee: { amount: '0.00001000', currency: 'BTC' },
          totalAmount: { amount: '0.00011000', currency: 'BTC' },
          onchain: { txnId: 'txid-1' },
        });
      }

      return new Response('not found', { status: 404 });
    });

    const node = new StrikeNode(
      { apiKey: 'test-token', baseUrl: 'https://api.strike.test/v1' },
      { fetch: fetchMock }
    );

    const payment = await node.payOnchain({
      id: 'quote-1',
      address: 'bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh',
      amountSats: 10_000,
      feeSats: 1_000,
      totalAmountSats: 11_000,
      recipientAmountSats: 10_000,
      feePayer: 'sender',
      fee: { type: 'speed', speed: 'normal' },
    });

    expect(payment).toMatchObject({
      paymentId: 'payment-1',
      txid: 'txid-1',
      state: 'completed',
      address: 'bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh',
      amountSats: 10_000,
      feeSats: 1_000,
      totalAmountSats: 11_000,
      recipientAmountSats: 10_000,
    });
    expect(fetchMock).toHaveBeenCalledTimes(2);
  });

  it('blocks on-chain execution when the quoted fee exceeds the default guardrail', async () => {
    const fetchMock = vi.fn<FetchLike>();
    const node = new StrikeNode(
      { apiKey: 'test-token', baseUrl: 'https://api.strike.test/v1' },
      { fetch: fetchMock }
    );

    await expect(
      node.payOnchain({
        id: 'quote-1',
        address: 'bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh',
        amountSats: 10_000,
        feeSats: 3_000,
        feePayer: 'sender',
        fee: { type: 'speed', speed: 'normal' },
      })
    ).rejects.toMatchObject({
      code: 'InvalidInput',
    });
    expect(fetchMock).not.toHaveBeenCalled();
  });

  it('allows on-chain execution to bypass the default fee guardrail only with the dangerous opt-out', async () => {
    const fetchMock = vi.fn<FetchLike>(async (input) => {
      const url = String(input);

      if (url === 'https://api.strike.test/v1/payment-quotes/quote-1/execute') {
        return jsonResponse({
          paymentId: 'payment-1',
          state: 'PENDING',
          amount: { amount: '0.00010000', currency: 'BTC' },
          totalFee: { amount: '0.00003000', currency: 'BTC' },
          totalAmount: { amount: '0.00013000', currency: 'BTC' },
        });
      }

      return new Response('not found', { status: 404 });
    });
    const node = new StrikeNode(
      { apiKey: 'test-token', baseUrl: 'https://api.strike.test/v1' },
      { fetch: fetchMock }
    );

    const payment = await node.payOnchain(
      {
        id: 'quote-1',
        address: 'bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh',
        amountSats: 10_000,
        feeSats: 3_000,
        totalAmountSats: 13_000,
        recipientAmountSats: 10_000,
        feePayer: 'sender',
        fee: { type: 'speed', speed: 'normal' },
      },
      { dangerouslyDisableFeeGuardrail: true }
    );

    expect(payment).toMatchObject({
      paymentId: 'payment-1',
      state: 'pending',
      feeSats: 3_000,
    });
    expect(fetchMock).toHaveBeenCalledTimes(2);
  });

  it('fails closed when the on-chain fee is unknown', async () => {
    const fetchMock = vi.fn<FetchLike>();
    const node = new StrikeNode(
      { apiKey: 'test-token', baseUrl: 'https://api.strike.test/v1' },
      { fetch: fetchMock }
    );

    await expect(
      node.payOnchain({
        id: 'quote-original',
        address: 'bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh',
        amountSats: 10_000,
        feePayer: 'sender',
        fee: { type: 'speed', speed: 'free' },
      })
    ).rejects.toMatchObject({
      code: 'InvalidInput',
    });
    expect(fetchMock).not.toHaveBeenCalled();
  });

  it('rejects fee preferences Strike cannot map to on-chain tiers', async () => {
    const fetchMock = vi.fn<FetchLike>();
    const node = new StrikeNode(
      { apiKey: 'test-token', baseUrl: 'https://api.strike.test/v1' },
      { fetch: fetchMock }
    );

    await expect(
      node.prepareOnchainTransaction({
        address: 'bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh',
        amountSats: 10_000,
        fee: { type: 'satsPerVbyte', satsPerVbyte: 5 },
      })
    ).rejects.toMatchObject({
      code: 'InvalidInput',
    });

    expect(fetchMock).not.toHaveBeenCalled();
  });
});
