import { describe, expect, it, vi } from 'vitest';
import { LndNode } from '../nodes/lnd.js';
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

describe('LndNode on-chain payments', () => {
  it('prepares an on-chain transaction using LND fee estimate', async () => {
    const fetchMock = vi.fn<FetchLike>(async (input) => {
      const url = new URL(String(input));

      if (url.pathname === '/v1/transactions/fee') {
        expect(url.searchParams.get('AddrToAmount[bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh]')).toBe('10000');
        expect(url.searchParams.get('target_conf')).toBe('6');
        return jsonResponse({
          fee_sat: '1000',
          sat_per_vbyte: '12',
        });
      }

      return new Response('not found', { status: 404 });
    });
    const node = new LndNode(
      { url: 'https://lnd.test', macaroon: '00' },
      { fetch: fetchMock },
    );

    const transaction = await node.prepareOnchainTransaction({
      address: 'bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh',
      amountSats: 10_000,
      fee: { type: 'speed', speed: 'normal' },
      description: 'cold storage',
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
  });

  it('executes an on-chain transaction using LND sendcoins', async () => {
    const fetchMock = vi.fn<FetchLike>(async (input, init) => {
      const url = new URL(String(input));
      const body = init?.body ? JSON.parse(String(init.body)) : undefined;

      if (url.pathname === '/v1/transactions') {
        expect(body).toEqual({
          addr: 'bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh',
          amount: 10_000,
          sat_per_vbyte: '5',
          label: 'cold storage',
        });
        return jsonResponse({ txid: 'txid-1' });
      }

      return new Response('not found', { status: 404 });
    });
    const node = new LndNode(
      { url: 'https://lnd.test', macaroon: '00' },
      { fetch: fetchMock },
    );

    const payment = await node.payOnchain({
      address: 'bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh',
      amountSats: 10_000,
      feeSats: 1_000,
      totalAmountSats: 11_000,
      recipientAmountSats: 10_000,
      feePayer: 'sender',
      fee: { type: 'satsPerVbyte', satsPerVbyte: 5 },
      raw: { label: 'cold storage' },
    });

    expect(payment).toMatchObject({
      txid: 'txid-1',
      state: 'pending',
      address: 'bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh',
      amountSats: 10_000,
      feeSats: 1_000,
      totalAmountSats: 11_000,
    });
  });

  it('rejects unsupported LND on-chain fee modes before network calls', async () => {
    const fetchMock = vi.fn<FetchLike>();
    const node = new LndNode(
      { url: 'https://lnd.test', macaroon: '00' },
      { fetch: fetchMock },
    );

    await expect(
      node.prepareOnchainTransaction({
        address: 'bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh',
        amountSats: 10_000,
        feePayer: 'recipient',
      }),
    ).rejects.toMatchObject({ code: 'InvalidInput' });

    await expect(
      node.prepareOnchainTransaction({
        address: 'bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh',
        amountSats: 10_000,
        fee: { type: 'speed', speed: 'free' },
      }),
    ).rejects.toMatchObject({ code: 'InvalidInput' });

    expect(fetchMock).not.toHaveBeenCalled();
  });

  it('blocks on-chain execution when the quoted fee exceeds the default guardrail', async () => {
    const fetchMock = vi.fn<FetchLike>();
    const node = new LndNode(
      { url: 'https://lnd.test', macaroon: '00' },
      { fetch: fetchMock },
    );

    await expect(
      node.payOnchain({
        address: 'bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh',
        amountSats: 10_000,
        feeSats: 3_000,
        feePayer: 'sender',
        fee: { type: 'targetConf', blocks: 6 },
      }),
    ).rejects.toMatchObject({ code: 'InvalidInput' });
    expect(fetchMock).not.toHaveBeenCalled();
  });
});
