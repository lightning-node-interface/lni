import { describe, expect, it, vi } from 'vitest';
import { BlinkNode } from '../nodes/blink.js';
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

function meResponse() {
  return jsonResponse({
    data: {
      me: {
        defaultAccount: {
          wallets: [
            {
              id: 'wallet-btc',
              walletCurrency: 'BTC',
              balance: 1_000_000,
            },
          ],
        },
      },
    },
  });
}

describe('BlinkNode on-chain payments', () => {
  it('prepares an on-chain transaction using Blink fee estimate and speed mapping', async () => {
    const fetchMock = vi.fn<FetchLike>(async (_input, init) => {
      const body = init?.body ? JSON.parse(String(init.body)) : undefined;

      if (body.query.includes('query Me')) {
        return meResponse();
      }

      if (body.query.includes('onChainTxFee')) {
        expect(body.variables).toEqual({
          walletId: 'wallet-btc',
          address: 'bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh',
          amount: 10_000,
          speed: 'MEDIUM',
        });

        return jsonResponse({
          data: {
            onChainTxFee: {
              amount: 1_000,
            },
          },
        });
      }

      return new Response('not found', { status: 404 });
    });

    const node = new BlinkNode(
      { apiKey: 'test-token', baseUrl: 'https://api.blink.test/graphql' },
      { fetch: fetchMock }
    );

    const transaction = await node.prepareOnchainTransaction({
      address: 'bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh',
      amountSats: 10_000,
      fee: { type: 'speed', speed: 'normal' },
      feePayer: 'sender',
    });

    expect(transaction).toMatchObject({
      address: 'bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh',
      amountSats: 10_000,
      feeSats: 1_000,
      totalAmountSats: 11_000,
      recipientAmountSats: 10_000,
      feePayer: 'sender',
      fee: { type: 'speed', speed: 'normal' },
    });
    expect(fetchMock).toHaveBeenCalledTimes(2);
  });

  it('executes an on-chain transaction and returns txid when Blink includes one', async () => {
    const fetchMock = vi.fn<FetchLike>(async (_input, init) => {
      const body = init?.body ? JSON.parse(String(init.body)) : undefined;

      if (body.query.includes('query Me')) {
        return meResponse();
      }

      if (body.query.includes('onChainPaymentSend')) {
        expect(body.variables).toEqual({
          input: {
            address: 'bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh',
            amount: 10_000,
            walletId: 'wallet-btc',
            speed: 'FAST',
          },
        });

        return jsonResponse({
          data: {
            onChainPaymentSend: {
              status: 'SUCCESS',
              transaction: {
                id: 'tx-1',
                settlementAmount: 10_000,
                settlementCurrency: 'BTC',
                settlementFee: 1_000,
                settlementVia: {
                  __typename: 'SettlementViaOnChain',
                  transactionHash: 'txid-1',
                },
              },
              errors: [],
            },
          },
        });
      }

      return new Response('not found', { status: 404 });
    });

    const node = new BlinkNode(
      { apiKey: 'test-token', baseUrl: 'https://api.blink.test/graphql' },
      { fetch: fetchMock }
    );

    const payment = await node.payOnchain({
      address: 'bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh',
      amountSats: 10_000,
      feeSats: 1_000,
      totalAmountSats: 11_000,
      recipientAmountSats: 10_000,
      feePayer: 'sender',
      fee: { type: 'speed', speed: 'fast' },
    });

    expect(payment).toMatchObject({
      paymentId: 'tx-1',
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

  it('rejects unsupported Blink on-chain fee preferences before network calls', async () => {
    const fetchMock = vi.fn<FetchLike>();
    const node = new BlinkNode(
      { apiKey: 'test-token', baseUrl: 'https://api.blink.test/graphql' },
      { fetch: fetchMock }
    );

    await expect(
      node.prepareOnchainTransaction({
        address: 'bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh',
        amountSats: 10_000,
        fee: { type: 'speed', speed: 'free' },
      })
    ).rejects.toMatchObject({
      code: 'InvalidInput',
    });

    await expect(
      node.prepareOnchainTransaction({
        address: 'bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh',
        amountSats: 10_000,
        feePayer: 'recipient',
      })
    ).rejects.toMatchObject({
      code: 'InvalidInput',
    });

    expect(fetchMock).not.toHaveBeenCalled();
  });

  it('blocks on-chain execution when the quoted fee exceeds the default guardrail', async () => {
    const fetchMock = vi.fn<FetchLike>();
    const node = new BlinkNode(
      { apiKey: 'test-token', baseUrl: 'https://api.blink.test/graphql' },
      { fetch: fetchMock }
    );

    await expect(
      node.payOnchain({
        address: 'bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh',
        amountSats: 10_000,
        feeSats: 3_000,
        feePayer: 'sender',
        fee: { type: 'speed', speed: 'fast' },
      })
    ).rejects.toMatchObject({
      code: 'InvalidInput',
    });
    expect(fetchMock).not.toHaveBeenCalled();
  });
});
