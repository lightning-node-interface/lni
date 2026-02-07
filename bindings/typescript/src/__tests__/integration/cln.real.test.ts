import { describe, expect, it } from 'vitest';
import { ClnNode } from '../../nodes/cln.js';
import { hasEnv, testInvoiceLabel, timeout } from './helpers.js';

const itIf = (condition: boolean) => (condition ? it : it.skip);

describe('Real integration from crates/lni/.env > ClnNode', () => {
  const enabled = hasEnv('CLN_URL', 'CLN_RUNE');

  const makeNode = () =>
    new ClnNode({
      url: process.env.CLN_URL!,
      rune: process.env.CLN_RUNE!,
    });

  itIf(enabled)('getInfo', async () => {
    const node = makeNode();
    const info = await node.getInfo();
    expect(typeof info.pubkey).toBe('string');
    expect(info.pubkey.length).toBeGreaterThan(0);
  }, timeout);

  itIf(enabled)('createInvoice + lookupInvoice + listTransactions', async () => {
    const node = makeNode();
    const invoice = await node.createInvoice({
      amountMsats: 2_000,
      description: testInvoiceLabel('cln'),
    });

    expect(invoice.invoice.length).toBeGreaterThan(0);
    expect(invoice.paymentHash.length).toBeGreaterThan(0);

    const lookedUp = await node.lookupInvoice({ paymentHash: invoice.paymentHash });
    expect(lookedUp.paymentHash).toBe(invoice.paymentHash);

    const txs = await node.listTransactions({ from: 0, limit: 25, paymentHash: invoice.paymentHash });
    expect(Array.isArray(txs)).toBe(true);
    expect(txs.some((tx) => tx.paymentHash === invoice.paymentHash)).toBe(true);
  }, timeout);

  itIf(enabled && hasEnv('CLN_TEST_PAYMENT_REQUEST'))('decode', async () => {
    const node = makeNode();
    const decoded = await node.decode(process.env.CLN_TEST_PAYMENT_REQUEST!);
    expect(decoded.length).toBeGreaterThan(0);
  }, timeout);
});
