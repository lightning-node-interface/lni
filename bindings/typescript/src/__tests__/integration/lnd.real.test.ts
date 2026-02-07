import { describe, expect } from 'vitest';
import { LndNode } from '../../nodes/lnd.js';
import { hasEnv, itIf, runOrSkipKnownError, testInvoiceLabel, timeout } from './helpers.js';

describe('Real integration from crates/lni/.env > LndNode', () => {
  const enabled = hasEnv('LND_URL', 'LND_MACAROON');

  const makeNode = () =>
    new LndNode({
      url: process.env.LND_URL!,
      macaroon: process.env.LND_MACAROON!,
    });

  itIf(enabled)('getInfo', async () => {
    await runOrSkipKnownError(async () => {
      const node = makeNode();
      const info = await node.getInfo();
      expect(typeof info.pubkey).toBe('string');
      expect(info.pubkey.length).toBeGreaterThan(0);
    }, ['permission denied']);
  }, timeout);

  itIf(enabled)('createInvoice + lookupInvoice + listTransactions', async () => {
    const node = makeNode();
    const invoice = await node.createInvoice({
      amountMsats: 3_000,
      description: testInvoiceLabel('lnd'),
    });
    console.log('LND Invoice:', invoice);
    expect(invoice.invoice.length).toBeGreaterThan(0);
    expect(invoice.paymentHash.length).toBeGreaterThan(0);

    const lookedUp = await node.lookupInvoice({ paymentHash: invoice.paymentHash });
    expect(lookedUp.paymentHash.length).toBeGreaterThan(0);

    const txs = await node.listTransactions({ from: 0, limit: 25, paymentHash: invoice.paymentHash });
    expect(Array.isArray(txs)).toBe(true);
  }, timeout);

  itIf(enabled && hasEnv('LND_TEST_PAYMENT_REQUEST'))('decode', async () => {
    const node = makeNode();
    const decoded = await node.decode(process.env.LND_TEST_PAYMENT_REQUEST!);
    expect(decoded.length).toBeGreaterThan(0);
  }, timeout);
});
