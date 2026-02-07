import { describe, expect } from 'vitest';
import { SpeedNode } from '../../nodes/speed.js';
import { hasEnv, itIf, nonEmpty, runOrSkipKnownError, testInvoiceLabel, timeout, uniqueValues } from './helpers.js';

describe('Real integration from crates/lni/.env > SpeedNode', () => {
  const enabled = hasEnv('SPEED_API_KEY');

  const makeNode = () =>
    new SpeedNode({
      apiKey: process.env.SPEED_API_KEY!,
      baseUrl: nonEmpty(process.env.SPEED_BASE_URL),
    });

  itIf(enabled)('getInfo + createInvoice + listTransactions', async () => {
    const node = makeNode();
    const info = await node.getInfo();
    expect(typeof info.alias).toBe('string');

    const invoice = await node.createInvoice({
      amountMsats: 5_000,
      description: testInvoiceLabel('speed'),
    });
    console.log('Speed Invoice:', invoice);
    expect(invoice.invoice.length).toBeGreaterThan(0);

    const txs = await node.listTransactions({ from: 0, limit: 25 });
    expect(Array.isArray(txs)).toBe(true);
  }, timeout);

  itIf(enabled)('lookupInvoice by search (best effort from env or recent tx)', async () => {
    await runOrSkipKnownError(async () => {
      const node = makeNode();
      const txs = await node.listTransactions({ from: 0, limit: 50 });
      const candidateSearch = txs.find((tx) => tx.invoice.length > 0)?.invoice;
      const searches = uniqueValues([process.env.SPEED_TEST_PAYMENT_REQUEST, candidateSearch]);

      if (!searches.length) {
        return;
      }

      let lastError: unknown;
      for (const search of searches) {
        try {
          const tx = await node.lookupInvoice({ search });
          expect(typeof tx.type).toBe('string');
          expect(tx.type.length).toBeGreaterThan(0);
          return;
        } catch (error) {
          lastError = error;
        }
      }

      if (lastError) {
        throw lastError;
      }
    }, ['no transactions found']);
  }, timeout);
});
