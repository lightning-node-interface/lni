import { describe, expect } from 'vitest';
import { StrikeNode } from '../../nodes/strike.js';
import { hasEnv, itIf, nonEmpty, runOrSkipKnownError, testInvoiceLabel, timeout, uniqueValues } from './helpers.js';

describe('Real integration from crates/lni/.env > StrikeNode', () => {
  const enabled = hasEnv('STRIKE_API_KEY');

  const makeNode = () =>
    new StrikeNode({
      apiKey: process.env.STRIKE_API_KEY!,
      baseUrl: nonEmpty(process.env.STRIKE_BASE_URL),
    });

  itIf(enabled)('getInfo + createInvoice + listTransactions', async () => {
    const node = makeNode();
    const info = await node.getInfo();
    expect(typeof info.alias).toBe('string');

    const invoice = await node.createInvoice({
      amountMsats: 5_000,
      description: testInvoiceLabel('strike'),
    });
    console.log('Strike Invoice:', invoice);
    expect(invoice.invoice.length).toBeGreaterThan(0);
    expect(invoice.paymentHash.length).toBeGreaterThan(0);

    const txs = await node.listTransactions({ from: 0, limit: 25 });
    expect(Array.isArray(txs)).toBe(true);
  }, timeout);

  itIf(enabled)('lookupInvoice (best effort from env or recent tx)', async () => {
    await runOrSkipKnownError(async () => {
      const node = makeNode();
      const txs = await node.listTransactions({ from: 0, limit: 50 });
      const candidateHash = txs.find((tx) => tx.paymentHash.length > 0)?.paymentHash;
      const lookupHashes = uniqueValues([process.env.STRIKE_TEST_PAYMENT_HASH, candidateHash]);

      if (!lookupHashes.length) {
        return;
      }

      let lastError: unknown;
      for (const paymentHash of lookupHashes) {
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
    }, ['no receive found', 'http 404']);
  }, timeout);
});
