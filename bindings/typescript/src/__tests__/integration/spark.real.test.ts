import { describe, expect } from 'vitest';
import { SparkNode } from '../../nodes/spark.js';
import { hasEnv, itIf, nonEmpty, runOrSkipKnownError, testInvoiceLabel, timeout, uniqueValues } from './helpers.js';
import type { SparkNetwork } from '../../types.js';

function sparkNetworkFromEnv(): SparkNetwork | undefined {
  const network = nonEmpty(process.env.SPARK_NETWORK);
  if (!network) {
    return undefined;
  }

  switch (network.toLowerCase()) {
    case 'mainnet':
    case 'regtest':
    case 'testnet':
    case 'signet':
    case 'local':
      return network.toLowerCase() as SparkNetwork;
    default:
      return undefined;
  }
}

describe('Real integration from crates/lni/.env > SparkNode', () => {
  const enabled = hasEnv('SPARK_MNEMONIC');

  const makeNode = () =>
    new SparkNode({
      mnemonic: process.env.SPARK_MNEMONIC!,
      passphrase: nonEmpty(process.env.SPARK_PASSPHRASE),
      network: sparkNetworkFromEnv(),
      // Node integration runtime currently requires the default SDK entry.
      sdkEntry: 'default',
      sparkOptions: {
        sspClientOptions:
          nonEmpty(process.env.SPARK_SSP_BASE_URL) && nonEmpty(process.env.SPARK_SSP_IDENTITY_PUBLIC_KEY)
            ? {
                baseUrl: process.env.SPARK_SSP_BASE_URL!,
                identityPublicKey: process.env.SPARK_SSP_IDENTITY_PUBLIC_KEY!,
              }
            : undefined,
      },
    });

  itIf(enabled)('getInfo + createInvoice + listTransactions', async () => {
    const node = makeNode();
    const info = await node.getInfo();
    expect(typeof info.alias).toBe('string');
    expect(info.sendBalanceMsat).toBeGreaterThan(0);

    const invoice = await node.createInvoice({
      amountMsats: 5_000,
      description: testInvoiceLabel('spark'),
      expiry: 3600,
    });
    console.log('Spark Invoice:', invoice);
    expect(invoice.invoice.length).toBeGreaterThan(0);

    const txs = await node.listTransactions({ from: 0, limit: 25, paymentHash: invoice.paymentHash });
    expect(Array.isArray(txs)).toBe(true);
  }, timeout);

  itIf(enabled)('payInvoice', async () => {
    const invoice = nonEmpty(process.env.SPARK_TEST_PAY_INVOICE);
    if (!invoice) {
      console.log('Skipping payInvoice: set SPARK_TEST_PAY_INVOICE env var');
      return;
    }

    const node = makeNode();
    const result = await node.payInvoice({ invoice });
    console.log('payInvoice result:', result);
    expect(result.paymentHash.length).toBeGreaterThan(0);
    expect(result.preimage.length).toBeGreaterThan(0);
  }, timeout);

  itIf(enabled)('lookupInvoice (best effort from env or recent tx)', async () => {
    await runOrSkipKnownError(async () => {
      const node = makeNode();
      const txs = await node.listTransactions({ from: 0, limit: 50 });
      const candidateHash = txs.find((tx) => tx.paymentHash.length > 0)?.paymentHash;
      const paymentHashes = uniqueValues([process.env.SPARK_TEST_PAYMENT_HASH, candidateHash]);

      if (!paymentHashes.length) {
        return;
      }

      let lastError: unknown;
      for (const paymentHash of paymentHashes) {
        try {
          const tx = await node.lookupInvoice({ paymentHash });
          expect(tx.paymentHash.length).toBeGreaterThan(0);
          return;
        } catch (error) {
          lastError = error;
        }
      }

      if (lastError) {
        throw lastError;
      }
    }, ['invoice not found', 'not found']);
  }, timeout);
});
