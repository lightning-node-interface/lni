import { describe, expect, it, vi } from 'vitest';
import { ClnNode } from '../nodes/cln.js';
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

function compactSize(value: number): number[] {
  if (value >= 0xfd) {
    throw new Error('test helper only supports small compact sizes');
  }
  return [value];
}

function u32le(value: number): number[] {
  return [value & 0xff, (value >> 8) & 0xff, (value >> 16) & 0xff, (value >> 24) & 0xff];
}

function u64le(value: number): number[] {
  let remaining = BigInt(value);
  const bytes: number[] = [];
  for (let i = 0; i < 8; i += 1) {
    bytes.push(Number(remaining & 0xffn));
    remaining >>= 8n;
  }
  return bytes;
}

function psbtMap(entries: Array<{ key: number[]; value: number[] }>): number[] {
  return [
    ...entries.flatMap(({ key, value }) => [
      ...compactSize(key.length),
      ...key,
      ...compactSize(value.length),
      ...value,
    ]),
    0,
  ];
}

function testPsbtWithFee(feeSats: number): string {
  const amountSats = 10_000;
  const unsignedTx = [
    ...u32le(2),
    1,
    ...Array.from({ length: 32 }, () => 1),
    ...u32le(0),
    0,
    ...u32le(0xffffffff),
    1,
    ...u64le(amountSats),
    0,
    ...u32le(0),
  ];
  const witnessUtxo = [
    ...u64le(amountSats + feeSats),
    0,
  ];
  const psbt = [
    0x70,
    0x73,
    0x62,
    0x74,
    0xff,
    ...psbtMap([{ key: [0x00], value: unsignedTx }]),
    ...psbtMap([{ key: [0x01], value: witnessUtxo }]),
    ...psbtMap([]),
  ];

  return Buffer.from(psbt).toString('base64');
}

describe('ClnNode on-chain payments', () => {
  it('prepares an on-chain transaction using CLN txprepare', async () => {
    const fetchMock = vi.fn<FetchLike>(async (_input, init) => {
      const body = init?.body ? JSON.parse(String(init.body)) : undefined;
      expect(body).toEqual({
        outputs: [{ bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh: '10000sat' }],
        feerate: 'normal',
      });

      return jsonResponse({
        txid: 'txid-1',
        unsigned_tx: '02000000',
        psbt: testPsbtWithFee(1_000),
      });
    });
    const node = new ClnNode(
      { url: 'https://cln.test', rune: 'rune' },
      { fetch: fetchMock },
    );

    const transaction = await node.prepareOnchainTransaction({
      address: 'bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh',
      amountSats: 10_000,
      fee: { type: 'speed', speed: 'normal' },
      description: 'cold storage',
    });

    expect(transaction).toMatchObject({
      id: 'txid-1',
      address: 'bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh',
      amountSats: 10_000,
      feeSats: 1_000,
      totalAmountSats: 11_000,
      recipientAmountSats: 10_000,
      feePayer: 'sender',
      fee: { type: 'speed', speed: 'normal' },
    });
  });

  it('executes a prepared CLN on-chain transaction using txsend', async () => {
    const fetchMock = vi.fn<FetchLike>(async (input, init) => {
      const url = new URL(String(input));
      const body = init?.body ? JSON.parse(String(init.body)) : undefined;

      if (url.pathname === '/v1/txsend') {
        expect(body).toEqual({ txid: 'txid-1' });
        return jsonResponse({ txid: 'txid-1', tx: '02000000' });
      }

      return new Response('not found', { status: 404 });
    });
    const node = new ClnNode(
      { url: 'https://cln.test', rune: 'rune' },
      { fetch: fetchMock },
    );

    const payment = await node.payOnchain({
      id: 'txid-1',
      address: 'bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh',
      amountSats: 10_000,
      feeSats: 1_000,
      totalAmountSats: 11_000,
      recipientAmountSats: 10_000,
      feePayer: 'sender',
      fee: { type: 'satsPerVbyte', satsPerVbyte: 5 },
    });

    expect(payment).toMatchObject({
      paymentId: 'txid-1',
      txid: 'txid-1',
      state: 'pending',
      address: 'bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh',
      amountSats: 10_000,
      feeSats: 1_000,
      totalAmountSats: 11_000,
    });
  });

  it('rejects unsupported CLN on-chain fee modes before network calls', async () => {
    const fetchMock = vi.fn<FetchLike>();
    const node = new ClnNode(
      { url: 'https://cln.test', rune: 'rune' },
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
        fee: { type: 'targetConf', blocks: 6 },
      }),
    ).rejects.toMatchObject({ code: 'InvalidInput' });

    expect(fetchMock).not.toHaveBeenCalled();
  });

  it('blocks CLN txsend when the quoted fee exceeds the default guardrail', async () => {
    const fetchMock = vi.fn<FetchLike>();
    const node = new ClnNode(
      { url: 'https://cln.test', rune: 'rune' },
      { fetch: fetchMock },
    );

    await expect(
      node.payOnchain({
        id: 'txid-1',
        address: 'bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh',
        amountSats: 10_000,
        feeSats: 3_000,
        feePayer: 'sender',
        fee: { type: 'speed', speed: 'normal' },
      }),
    ).rejects.toMatchObject({ code: 'InvalidInput' });
    expect(fetchMock).not.toHaveBeenCalled();
  });
});
