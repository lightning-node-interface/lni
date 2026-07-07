import { describe, expect } from 'vitest';
import { NwcNode } from '../../nodes/nwc.js';
import { hasEnv, itIf, testInvoiceLabel, timeout } from './helpers.js';

describe('Real integration from crates/lni/.env > NwcNode', async () => {
  const enabled = hasEnv('NWC_URI') && typeof globalThis.WebSocket === 'function';

  const makeNode = () => new NwcNode({ nwcUri: process.env.NWC_URI!, httpTimeout: 15 });
 
  itIf(enabled)('getInfo + createInvoice + listTransactions + lookupInvoice', async () => {
    const node = makeNode();

    try {
      const info = await node.getInfo();
      expect(typeof info.alias).toBe('string');

      const invoice1 = await node.lookupInvoice({ paymentHash: process.env.NWC_TEST_PAYMENT_HASH! });
      console.log('NWC Invoice Lookup by Hash:', invoice1);
      expect(invoice1.paymentHash.length).toBeGreaterThan(0);

      const invoice = await node.createInvoice({
        amountMsats: 3_000,
        description: testInvoiceLabel('nwc'),
      });
      console.log('NWC Invoice:', invoice);
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

  itIf(enabled)('payInvoice can be canceled against a real NWC connection', async () => {
    const verificationNode = makeNode();
    try {
      const info = await verificationNode.getInfo();
      expect(typeof info.alias).toBe('string');
    } finally {
      verificationNode.close();
    }

    const node = new NwcNode({ nwcUri: process.env.NWC_URI!, httpTimeout: 0.25 });
    const controller = new AbortController();

    try {
      const startedAt = Date.now();
      const payment = node.payInvoice(
        {
          invoice: 'lnbc1lniaborttest',
        },
        {
          signal: controller.signal,
        },
      );

      queueMicrotask(() => {
        controller.abort();
      });

      await expect(payment).rejects.toMatchObject({
        code: 'Canceled',
      });
      expect(Date.now() - startedAt).toBeLessThan(1_000);

      await new Promise((resolve) => setTimeout(resolve, 350));
    } finally {
      node.close();
    }
  }, timeout);
});
