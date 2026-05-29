import { describe, expect, it, vi } from 'vitest';
import { StrikeNode } from '../nodes/strike.js';
import type { FetchLike } from '../types.js';

function jsonResponse(body: unknown): Response {
  return new Response(JSON.stringify(body), {
    headers: {
      'content-type': 'application/json',
    },
  });
}

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
          },
          feePolicy: 'EXCLUSIVE',
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
      { fetch: fetchMock },
    );

    const transaction = await node.prepareOnchainTransaction({
      address: 'bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh',
      amountMsats: 10_000_000,
      fee: { type: 'speed', speed: 'normal' },
      feePayer: 'sender',
      description: 'cold storage',
      idempotencyKey: '00000000-0000-4000-8000-000000000001',
    });

    expect(transaction).toMatchObject({
      id: 'quote-1',
      address: 'bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh',
      amountMsats: 10_000_000,
      feeMsats: 1_000_000,
      totalAmountMsats: 11_000_000,
      recipientAmountMsats: 10_000_000,
      feePayer: 'sender',
      fee: { type: 'speed', speed: 'normal' },
      estimatedDeliverySeconds: 3600,
    });
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
      { fetch: fetchMock },
    );

    const payment = await node.payOnchain({
      id: 'quote-1',
      address: 'bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh',
      amountMsats: 10_000_000,
      feeMsats: 1_000_000,
      totalAmountMsats: 11_000_000,
      recipientAmountMsats: 10_000_000,
      feePayer: 'sender',
      fee: { type: 'speed', speed: 'normal' },
    });

    expect(payment).toMatchObject({
      paymentId: 'payment-1',
      txid: 'txid-1',
      state: 'completed',
      address: 'bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh',
      amountMsats: 10_000_000,
      feeMsats: 1_000_000,
      totalAmountMsats: 11_000_000,
      recipientAmountMsats: 10_000_000,
    });
    expect(fetchMock).toHaveBeenCalledTimes(2);
  });

  it('rejects fee preferences Strike cannot map to on-chain tiers', async () => {
    const fetchMock = vi.fn<FetchLike>();
    const node = new StrikeNode(
      { apiKey: 'test-token', baseUrl: 'https://api.strike.test/v1' },
      { fetch: fetchMock },
    );

    await expect(
      node.prepareOnchainTransaction({
        address: 'bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh',
        amountMsats: 10_000_000,
        fee: { type: 'satsPerVbyte', satsPerVbyte: 5 },
      }),
    ).rejects.toMatchObject({
      code: 'InvalidInput',
    });

    expect(fetchMock).not.toHaveBeenCalled();
  });
});
