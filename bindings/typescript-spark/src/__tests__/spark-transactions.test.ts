import { describe, expect, it } from 'vitest';
import { getSparkFrost } from '@buildonspark/spark-sdk';
import type { SparkFrostBase } from '@buildonspark/spark-sdk/bare';
import { p2tr, Transaction, NETWORK, TEST_NETWORK } from '@scure/btc-signer';
import { secp256k1 } from '@noble/curves/secp256k1.js';
import { SparkNode } from '../nodes/spark.js';
import {
  constructNodeTxPair,
  constructRefundTxTrio,
  computeMultiInputSighash,
} from '../internal/spark-transactions.js';

// The SDK's Rust/WASM implementation is an independent, offline oracle used
// only in tests. The browser/Expo package continues to use no WASM.
const reference = getSparkFrost();
const secret = new Uint8Array(32);
secret[31] = 1;
const publicKey = secp256k1.getPublicKey(secret);
const payment = p2tr(publicKey.slice(1));
const regtest = { ...TEST_NETWORK, bech32: 'bcrt' };

function parent(amount = 100_000n, witness = false): Transaction {
  const tx = new Transaction({ version: 2 });
  tx.addInput({
    txid: new Uint8Array(32).fill(1), index: 3, sequence: 0xffffffff,
  });
  tx.addOutput({ script: payment.script, amount: 123n });
  tx.addOutput({ script: payment.script, amount });
  if (witness) {
    tx.updateInput(0, { finalScriptWitness: [new Uint8Array(64).fill(7)] });
  }
  return tx;
}

function transactionResult(result: { tx: Uint8Array }): { tx: string; sighash: string } {
  const withHash = result as { tx: Uint8Array; sighash: Uint8Array };
  return {
    tx: Buffer.from(withHash.tx).toString('hex'),
    sighash: Buffer.from(withHash.sighash).toString('hex'),
  };
}

describe('pure Spark transaction primitives against SDK Rust vectors', () => {
  it.each([0n, 330n, 955n, 956n, 1024n, 2048n, 100_000n])(
    'matches node bytes and sighashes at the %s-sat fee boundary', async (amount) => {
      const raw = parent(amount).toBytes(true);
      const fee = amount === 1024n ? 694n : 955n;
      const actual = constructNodeTxPair(raw, 1, payment.address!, 2000, 2050, fee);
      const expected = await reference.constructNodeTxPair(raw, 1, payment.address!, 2000, 2050, fee);
      expect(transactionResult(actual.cpfp)).toEqual(transactionResult(expected.cpfp));
      expect(transactionResult(actual.direct)).toEqual(transactionResult(expected.direct));
    }
  );

  it.each([
    ['mainnet', NETWORK], ['testnet', TEST_NETWORK], ['signet', TEST_NETWORK], ['regtest', regtest],
  ] as const)('matches refunds and their Taproot tweak on %s', async (network, addressNetwork) => {
    const raw = parent().toBytes(true);
    const address = p2tr(publicKey.slice(1), undefined, addressNetwork).address!;
    const pair = constructNodeTxPair(raw, 1, address, 2000, 2050, 955n);
    for (const direct of [null, pair.direct.tx]) {
      const actual = constructRefundTxTrio(pair.cpfp.tx, direct, 0, publicKey, network, 1900, 1950, 955n);
      const expected = await reference.constructRefundTxTrio(
        pair.cpfp.tx, direct, 0, publicKey, network, 1900, 1950, 955n
      );
      expect(transactionResult(actual.cpfp_refund)).toEqual(transactionResult(expected.cpfp_refund));
      expect(transactionResult(actual.direct_from_cpfp_refund))
        .toEqual(transactionResult(expected.direct_from_cpfp_refund));
      expect(actual.direct_refund && transactionResult(actual.direct_refund))
        .toEqual(expected.direct_refund && transactionResult(expected.direct_refund));
    }
  });

  it('uses the non-witness parent txid and preserves Spark bit-30 timelocks', async () => {
    const tx = parent(100_000n, true);
    const raw = tx.toBytes(true, true);
    const sequence = (1 << 30) | 500;
    const actual = constructNodeTxPair(raw, 1, payment.address!, sequence, sequence + 50, 955n);
    const expected = await reference.constructNodeTxPair(raw, 1, payment.address!, sequence, sequence + 50, 955n);
    expect(transactionResult(actual.cpfp)).toEqual(transactionResult(expected.cpfp));
    expect(transactionResult(actual.direct)).toEqual(transactionResult(expected.direct));
    expect(Transaction.fromRaw(actual.cpfp.tx).getInput(0).txid).toEqual(Uint8Array.from(Buffer.from(tx.id, 'hex')));
  });

  it('matches zero-timelock deposit root transactions', async () => {
    const raw = parent().toBytes(true);
    const actual = constructNodeTxPair(raw, 1, payment.address!, 0, 50, 955n);
    const expected = await reference.constructNodeTxPair(raw, 1, payment.address!, 0, 50, 955n);
    expect(transactionResult(actual.cpfp)).toEqual(transactionResult(expected.cpfp));
    expect(transactionResult(actual.direct)).toEqual(transactionResult(expected.direct));
  });

  it.each([
    publicKey,
    Uint8Array.of(3, ...publicKey.slice(1)),
    secp256k1.Point.fromBytes(publicKey).toBytes(false),
  ])('matches refund Taproot keys for each SEC1 encoding', async (key) => {
    const raw = parent().toBytes(true);
    const actual = constructRefundTxTrio(raw, null, 1, key, 'mainnet', 1900, 1950, 955n);
    const expected = await reference.constructRefundTxTrio(raw, null, 1, key, 'mainnet', 1900, 1950, 955n);
    expect(transactionResult(actual.cpfp_refund)).toEqual(transactionResult(expected.cpfp_refund));
    expect(transactionResult(actual.direct_from_cpfp_refund))
      .toEqual(transactionResult(expected.direct_from_cpfp_refund));
  });

  it('matches the capped-fee refund chain and retains dust-threshold outputs', async () => {
    const pair = constructNodeTxPair(parent(1024n).toBytes(true), 1, payment.address!, 2000, 2050, 694n);
    for (const fee of [0n, 694n, 955n]) {
      const actual = constructRefundTxTrio(pair.cpfp.tx, pair.direct.tx, 0, publicKey, 'mainnet', 1900, 1950, fee);
      const expected = await reference.constructRefundTxTrio(
        pair.cpfp.tx, pair.direct.tx, 0, publicKey, 'mainnet', 1900, 1950, fee
      );
      expect(transactionResult(actual.cpfp_refund)).toEqual(transactionResult(expected.cpfp_refund));
      expect(transactionResult(actual.direct_refund!)).toEqual(transactionResult(expected.direct_refund!));
      expect(transactionResult(actual.direct_from_cpfp_refund))
        .toEqual(transactionResult(expected.direct_from_cpfp_refund));
      expect(Transaction.fromRaw(actual.direct_refund!.tx).getOutput(0).amount).toBe(330n);
    }
  });

  it('matches BIP341 sighashes for each input and commits to all previous outputs', async () => {
    const tx = new Transaction({ version: 3 });
    tx.addInput({ txid: new Uint8Array(32).fill(1), index: 0, sequence: 500 });
    tx.addInput({ txid: new Uint8Array(32).fill(2), index: 1, sequence: 550 });
    tx.addOutput({ script: payment.script, amount: 1000n });
    const raw = tx.toBytes(true);
    const scripts = [payment.script, Uint8Array.of(0x51)];
    const values = [100_000, 200_000];
    for (const input of [0, 1]) {
      expect(computeMultiInputSighash(raw, input, scripts, values))
        .toEqual(await reference.computeMultiInputSighash(raw, input, scripts, values));
    }
    expect(computeMultiInputSighash(raw, 0, scripts, values))
      .not.toEqual(computeMultiInputSighash(raw, 1, scripts, values));
    expect(computeMultiInputSighash(raw, 0, scripts, values))
      .not.toEqual(computeMultiInputSighash(raw, 0, scripts, [100_000, 200_001]));
    expect(computeMultiInputSighash(raw, 0, scripts, values))
      .not.toEqual(computeMultiInputSighash(raw, 0, [payment.script, payment.script], values));
  });

  it.each([0x80000000, 0x00400000, -1, 1.5, 0x100000000])(
    'rejects unsupported timelock %s in either sequence', (sequence) => {
      const raw = parent().toBytes(true);
      for (const [cpfp, direct] of [[sequence, 50], [0, sequence]]) {
        expect(() => constructNodeTxPair(raw, 1, payment.address!, cpfp!, direct!, 955n)).toThrow();
        expect(() => constructRefundTxTrio(raw, null, 1, publicKey, 'mainnet', cpfp!, direct!, 955n)).toThrow();
      }
    }
  );

  it('rejects invalid destinations, networks, keys, output indexes and fees', () => {
    const raw = parent().toBytes(true);
    expect(() => constructNodeTxPair(raw, 2, payment.address!, 0, 50, 955n)).toThrow();
    expect(() => constructNodeTxPair(raw, 1, 'invalid', 0, 50, 955n)).toThrow();
    expect(() => constructNodeTxPair(raw, 1, payment.address!, 0, 50, -1n)).toThrow();
    expect(() => constructNodeTxPair(raw, 1, payment.address!, 0, 50, 1n << 64n)).toThrow();
    expect(() => constructRefundTxTrio(raw, null, 1, publicKey, 'invalid', 0, 50, 955n)).toThrow();
    expect(() => constructRefundTxTrio(raw, null, 1, new Uint8Array(32), 'mainnet', 0, 50, 955n)).toThrow();
  });

  it('rejects incomplete prevouts, invalid indexes and values that would lose precision', () => {
    const raw = parent().toBytes(true);
    expect(() => computeMultiInputSighash(raw, 0, [], [])).toThrow();
    expect(() => computeMultiInputSighash(raw, 0, [payment.script], [])).toThrow();
    for (const index of [-1, 1, 0.5]) {
      expect(() => computeMultiInputSighash(raw, index, [payment.script], [100_000])).toThrow();
    }
    for (const value of [-1, 0.5, Infinity, Number.MAX_SAFE_INTEGER + 1]) {
      expect(() => computeMultiInputSighash(raw, 0, [payment.script], [value])).toThrow();
    }
  });

  it('installs every transaction primitive on the SDK base provider', async () => {
    const node = new SparkNode({
      mnemonic: 'abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about',
      sdkEntry: 'bare',
    });
    const installed: { provider?: SparkFrostBase } = {};
    await (node as unknown as {
      installPureSparkFrost(sdk: unknown): Promise<void>;
    }).installPureSparkFrost({
      // Use the real abstract base, without inheriting the native provider's
      // concrete methods, so omitting a pure primitive cannot pass this test.
      SparkFrostBase: Object.getPrototypeOf(Object.getPrototypeOf(reference)).constructor,
      setSparkFrostOnce(provider: SparkFrostBase) { installed.provider = provider; },
    });
    expect(installed.provider).toBeDefined();
    const frost = installed.provider!;
    const raw = parent().toBytes(true);
    const pair = await frost.constructNodeTxPair(raw, 1, payment.address!, 2000, 2050, 955n);
    const expected = await reference.constructNodeTxPair(raw, 1, payment.address!, 2000, 2050, 955n);
    expect(transactionResult(pair.cpfp)).toEqual(transactionResult(expected.cpfp));
    expect(transactionResult(pair.direct)).toEqual(transactionResult(expected.direct));
    const refunds = await frost.constructRefundTxTrio(
      pair.cpfp.tx, pair.direct.tx, 0, publicKey, 'mainnet', 1900, 1950, 955n
    );
    expect(refunds.direct_refund).toBeDefined();
    const output = parent().getOutput(1);
    expect(await frost.computeMultiInputSighash(pair.cpfp.tx, 0, [output.script!], [Number(output.amount!)]))
      .toEqual(await reference.computeMultiInputSighash(pair.cpfp.tx, 0, [output.script!], [Number(output.amount!)]));
  });
});
