import { describe, expect, it } from 'vitest';
import { BlinkNode } from '../../nodes/blink.js';
import { hasEnv, nonEmpty, testInvoiceLabel, timeout, uniqueValues } from './helpers.js';

const itIf = (condition: boolean) => (condition ? it : it.skip);

describe('Real integration from crates/lni/.env > BlinkNode', () => {
  const enabled = hasEnv('BLINK_API_KEY');

  const makeNode = () =>
    new BlinkNode({
      apiKey: process.env.BLINK_API_KEY!,
      baseUrl: nonEmpty(process.env.BLINK_BASE_URL),
    });

  itIf(enabled)('getInfo + createInvoice + listTransactions', async () => {
    const node = makeNode();
    const info = await node.getInfo();
    expect(typeof info.alias).toBe('string');

    const invoice = await node.createInvoice({
      amountMsats: 5_000,
      description: testInvoiceLabel('blink'),
    });
    expect(invoice.invoice.length).toBeGreaterThan(0);

    const txs = await node.listTransactions({ from: 0, limit: 25 });
    expect(Array.isArray(txs)).toBe(true);
  }, timeout);

  itIf(enabled)('lookupInvoice (best effort from env or recent tx)', async () => {
    const node = makeNode();
    const txs = await node.listTransactions({ from: 0, limit: 50 });
    const candidateHash = txs.find((tx) => tx.paymentHash.length > 0)?.paymentHash;
    const hashes = uniqueValues([process.env.BLINK_TEST_PAYMENT_HASH, candidateHash]);

    if (!hashes.length) {
      return;
    }

    let lastError: unknown;
    for (const paymentHash of hashes) {
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
  }, timeout);
});
