import { describe, expect } from 'vitest';
import { ClnNode } from '../../nodes/cln.js';
import { hasEnv, itIf, testInvoiceLabel, timeout } from './helpers.js';

describe('Real integration from crates/lni/.env > ClnNode', () => {
  const enabled = hasEnv('CLN_URL', 'CLN_RUNE');
  const onchainEnabled = enabled && hasEnv('CLN_ONCHAIN_TEST_ADDRESS', 'CLN_ONCHAIN_AMOUNT_SATS');
  const onchainSendConfirmation = 'I_UNDERSTAND_THIS_BROADCASTS_BITCOIN';
  const shouldBroadcastOnchain =
    process.env.CLN_RUN_ONCHAIN_SEND === 'true' &&
    process.env.CLN_ONCHAIN_SEND_CONFIRM === onchainSendConfirmation;

  const makeNode = () =>
    new ClnNode({
      url: process.env.CLN_URL!,
      rune: process.env.CLN_RUNE!,
    });

  const clnPost = async (path: string, body: unknown): Promise<void> => {
    const url = new URL(path, process.env.CLN_URL!);
    const response = await fetch(url, {
      method: 'POST',
      headers: {
        rune: process.env.CLN_RUNE!,
        'content-type': 'application/json',
      },
      body: JSON.stringify(body),
    });
    if (!response.ok) {
      throw new Error(`CLN ${path} failed: ${response.status} ${await response.text()}`);
    }
  };

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
    console.log('CLN Invoice:', invoice);
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

  itIf(onchainEnabled)('prepareOnchainTransaction + optionally payOnchain', async () => {
    const node = makeNode();
    const amountSats = Number(process.env.CLN_ONCHAIN_AMOUNT_SATS);
    expect(Number.isSafeInteger(amountSats)).toBe(true);
    expect(amountSats).toBeGreaterThan(0);

    const transaction = await node.prepareOnchainTransaction({
      address: process.env.CLN_ONCHAIN_TEST_ADDRESS!,
      amountSats,
      fee: { type: 'speed', speed: 'normal' },
      description: testInvoiceLabel('cln onchain'),
    });

    expect(transaction.id?.length).toBeGreaterThan(0);
    expect(transaction.amountSats).toBe(amountSats);
    expect(transaction.feeSats).toBeGreaterThanOrEqual(0);

    if (!shouldBroadcastOnchain) {
      await clnPost('/v1/txdiscard', { txid: transaction.id });
      return;
    }

    const payment = await node.payOnchain(transaction);
    expect(payment.txid?.length).toBeGreaterThan(0);
  }, timeout);
});
