import { describe, expect } from 'vitest';
import { PhoenixdNode } from '../../nodes/phoenixd.js';
import { hasEnv, itIf, runOrSkipKnownError, testInvoiceLabel, timeout } from './helpers.js';

describe('Real integration from crates/lni/.env > PhoenixdNode', () => {
  const enabled = hasEnv('PHOENIXD_URL', 'PHOENIXD_PASSWORD');
  const onchainEnabled =
    enabled &&
    hasEnv('PHOENIXD_ONCHAIN_TEST_ADDRESS', 'PHOENIXD_ONCHAIN_AMOUNT_SATS', 'PHOENIXD_ONCHAIN_FEERATE_SAT_BYTE');
  const onchainSendConfirmation = 'I_UNDERSTAND_THIS_BROADCASTS_BITCOIN';
  const shouldBroadcastOnchain =
    process.env.PHOENIXD_RUN_ONCHAIN_SEND === 'true' &&
    process.env.PHOENIXD_ONCHAIN_SEND_CONFIRM === onchainSendConfirmation;

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
      console.log('Phoenixd Invoice:', invoice);
      expect(invoice.invoice.length).toBeGreaterThan(0);
      expect(invoice.paymentHash.length).toBeGreaterThan(0);

      const lookedUp = await node.lookupInvoice({ paymentHash: invoice.paymentHash });
      expect(lookedUp.paymentHash).toBe(invoice.paymentHash);

      const txs = await node.listTransactions({ from: 0, limit: 25, paymentHash: invoice.paymentHash });
      expect(Array.isArray(txs)).toBe(true);
    }, ['fetch failed', 'econnrefused', 'enotfound', 'timed out']);
  }, timeout);

  itIf(onchainEnabled)('prepareOnchainTransaction + optionally payOnchain', async () => {
    await runOrSkipKnownError(async () => {
      const node = makeNode();
      const amountSats = Number(process.env.PHOENIXD_ONCHAIN_AMOUNT_SATS);
      const feerateSatByte = Number(process.env.PHOENIXD_ONCHAIN_FEERATE_SAT_BYTE);
      expect(Number.isSafeInteger(amountSats)).toBe(true);
      expect(amountSats).toBeGreaterThan(0);
      expect(Number.isSafeInteger(feerateSatByte)).toBe(true);
      expect(feerateSatByte).toBeGreaterThan(0);

      const transaction = await node.prepareOnchainTransaction({
        address: process.env.PHOENIXD_ONCHAIN_TEST_ADDRESS!,
        amountSats,
        fee: { type: 'satsPerVbyte', satsPerVbyte: feerateSatByte },
        description: testInvoiceLabel('phoenixd onchain'),
      });

      expect(transaction.amountSats).toBe(amountSats);
      expect(transaction.feeSats).toBeUndefined();

      if (!shouldBroadcastOnchain) {
        return;
      }

      const payment = await node.payOnchain(transaction, {
        dangerouslyDisableFeeGuardrail: true,
      });
      expect(payment.txid?.length).toBeGreaterThan(0);
    }, ['fetch failed', 'econnrefused', 'enotfound', 'timed out']);
  }, timeout);
});
