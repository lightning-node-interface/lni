import { describe, expect, it } from 'vitest';
import { PhoenixdNode } from '../../nodes/phoenixd.js';
import { hasEnv, runOrSkipKnownError, testInvoiceLabel, timeout } from './helpers.js';

const itIf = (condition: boolean) => (condition ? it : it.skip);

describe('Real integration from crates/lni/.env > PhoenixdNode', () => {
  const enabled = hasEnv('PHOENIXD_URL', 'PHOENIXD_PASSWORD');

  const makeNode = () =>
    new PhoenixdNode({
      url: process.env.PHOENIXD_URL!,
      password: process.env.PHOENIXD_PASSWORD!,
    });

  itIf(enabled)('getInfo', async () => {
    await runOrSkipKnownError(async () => {
      const node = makeNode();
      const info = await node.getInfo();
      expect(typeof info.pubkey).toBe('string');
      expect(info.pubkey.length).toBeGreaterThan(0);
    }, ['fetch failed', 'econnrefused', 'enotfound', 'timed out']);
  }, timeout);

  itIf(enabled)('createInvoice + lookupInvoice + listTransactions', async () => {
    await runOrSkipKnownError(async () => {
      const node = makeNode();
      const invoice = await node.createInvoice({
        amountMsats: 2_000,
        description: testInvoiceLabel('phoenixd'),
      });

      expect(invoice.invoice.length).toBeGreaterThan(0);
      expect(invoice.paymentHash.length).toBeGreaterThan(0);

      const lookedUp = await node.lookupInvoice({ paymentHash: invoice.paymentHash });
      expect(lookedUp.paymentHash).toBe(invoice.paymentHash);

      const txs = await node.listTransactions({ from: 0, limit: 25, paymentHash: invoice.paymentHash });
      expect(Array.isArray(txs)).toBe(true);
    }, ['fetch failed', 'econnrefused', 'enotfound', 'timed out']);
  }, timeout);
});
