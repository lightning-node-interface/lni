import { describe, expect, it, vi } from 'vitest';
import { PhoenixdNode } from '../nodes/phoenixd.js';
import type { FetchLike } from '../types.js';

describe('PhoenixdNode on-chain payments', () => {
  it('prepares an on-chain transaction with an explicit feerate', async () => {
    const fetchMock = vi.fn<FetchLike>();
    const node = new PhoenixdNode(
      { url: 'http://phoenixd.test', password: 'password' },
      { fetch: fetchMock },
    );

    const transaction = await node.prepareOnchainTransaction({
      address: 'bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh',
      amountSats: 10_000,
      fee: { type: 'satsPerVbyte', satsPerVbyte: 12 },
      description: 'cold storage',
    });

    expect(transaction).toMatchObject({
      address: 'bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh',
      amountSats: 10_000,
      recipientAmountSats: 10_000,
      feePayer: 'sender',
      fee: { type: 'satsPerVbyte', satsPerVbyte: 12 },
      raw: {
        sendRequest: {
          address: 'bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh',
          amountSat: 10_000,
          feerateSatByte: 12,
        },
        description: 'cold storage',
      },
    });
    expect(fetchMock).not.toHaveBeenCalled();
  });

  it('executes an on-chain transaction using Phoenixd sendtoaddress', async () => {
    const fetchMock = vi.fn<FetchLike>(async (input, init) => {
      const url = new URL(String(input));
      expect(url.pathname).toBe('/sendtoaddress');
      expect(init?.method).toBe('POST');
      expect(init?.body).toBe('address=bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh&amountSat=10000&feerateSatByte=12');
      return new Response('a'.repeat(64));
    });
    const node = new PhoenixdNode(
      { url: 'http://phoenixd.test', password: 'password' },
      { fetch: fetchMock },
    );

    const payment = await node.payOnchain(
      {
        address: 'bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh',
        amountSats: 10_000,
        recipientAmountSats: 10_000,
        feePayer: 'sender',
        fee: { type: 'satsPerVbyte', satsPerVbyte: 12 },
      },
      { dangerouslyDisableFeeGuardrail: true },
    );

    expect(payment).toMatchObject({
      txid: 'a'.repeat(64),
      state: 'pending',
      address: 'bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh',
      amountSats: 10_000,
    });
  });

  it('requires an explicit Phoenixd on-chain feerate before network calls', async () => {
    const fetchMock = vi.fn<FetchLike>();
    const node = new PhoenixdNode(
      { url: 'http://phoenixd.test', password: 'password' },
      { fetch: fetchMock },
    );

    await expect(
      node.prepareOnchainTransaction({
        address: 'bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh',
        amountSats: 10_000,
      }),
    ).rejects.toMatchObject({ code: 'InvalidInput' });

    await expect(
      node.prepareOnchainTransaction({
        address: 'bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh',
        amountSats: 10_000,
        fee: { type: 'speed', speed: 'normal' },
      }),
    ).rejects.toMatchObject({ code: 'InvalidInput' });

    expect(fetchMock).not.toHaveBeenCalled();
  });

  it('blocks Phoenixd sendtoaddress unless the fee guardrail is explicitly disabled', async () => {
    const fetchMock = vi.fn<FetchLike>();
    const node = new PhoenixdNode(
      { url: 'http://phoenixd.test', password: 'password' },
      { fetch: fetchMock },
    );

    await expect(
      node.payOnchain({
        address: 'bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh',
        amountSats: 10_000,
        recipientAmountSats: 10_000,
        feePayer: 'sender',
        fee: { type: 'satsPerVbyte', satsPerVbyte: 12 },
      }),
    ).rejects.toMatchObject({ code: 'InvalidInput' });
    expect(fetchMock).not.toHaveBeenCalled();
  });
});
