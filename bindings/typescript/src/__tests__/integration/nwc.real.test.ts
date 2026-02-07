import { describe, expect, it } from 'vitest';
import { NwcNode } from '../../nodes/nwc.js';
import { hasEnv, testInvoiceLabel, timeout } from './helpers.js';

const itIf = (condition: boolean) => (condition ? it : it.skip);

describe('Real integration from crates/lni/.env > NwcNode', () => {
  const enabled = hasEnv('NWC_URI');

  const makeNode = () => new NwcNode({ nwcUri: process.env.NWC_URI! });

  itIf(enabled)('getInfo + createInvoice + listTransactions + lookupInvoice', async () => {
    const node = makeNode();
    try {
      const info = await node.getInfo();
      expect(typeof info.alias).toBe('string');

      const invoice = await node.createInvoice({
        amountMsats: 3_000,
        description: testInvoiceLabel('nwc'),
      });

      expect(invoice.invoice.length).toBeGreaterThan(0);

      const txs = await node.listTransactions({ from: 0, limit: 25 });
      expect(Array.isArray(txs)).toBe(true);

      if (invoice.paymentHash.length > 0) {
        const hashLookup = await node.lookupInvoice({ paymentHash: invoice.paymentHash });
        expect(hashLookup.paymentHash.length).toBeGreaterThan(0);
      }

      const invoiceLookup = await node.lookupInvoice({ search: invoice.invoice });
      expect(typeof invoiceLookup.type).toBe('string');
    } finally {
      node.close();
    }
  }, timeout);
});
