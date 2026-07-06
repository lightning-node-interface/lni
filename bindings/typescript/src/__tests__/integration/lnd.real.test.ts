import { describe, expect } from 'vitest';
import { LndNode } from '../../nodes/lnd.js';
import { hasEnv, itIf, runOrSkipKnownError, testInvoiceLabel, timeout } from './helpers.js';

const ONCHAIN_SEND_CONFIRMATION = 'I_UNDERSTAND_THIS_BROADCASTS_BITCOIN';
const DEFAULT_ONCHAIN_QUOTE_AMOUNT_SATS = 10_000;

describe('Real integration from crates/lni/.env > LndNode', () => {
  const enabled = hasEnv('LND_URL', 'LND_MACAROON');
  const onchainSendAmountSats = Number.parseInt(process.env.LND_ONCHAIN_AMOUNT_SATS ?? '', 10);
  const quoteOnlyAmountSats =
    Number.isSafeInteger(onchainSendAmountSats) && onchainSendAmountSats > 0
      ? onchainSendAmountSats
      : DEFAULT_ONCHAIN_QUOTE_AMOUNT_SATS;
  const runOnchainSend =
    enabled
    && process.env.LND_RUN_ONCHAIN_SEND === 'true'
    && process.env.LND_ONCHAIN_SEND_CONFIRM === ONCHAIN_SEND_CONFIRMATION
    && hasEnv('LND_ONCHAIN_TEST_ADDRESS', 'LND_ONCHAIN_AMOUNT_SATS')
    && Number.isSafeInteger(onchainSendAmountSats)
    && onchainSendAmountSats > 0;

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

  itIf(enabled && hasEnv('LND_ONCHAIN_TEST_ADDRESS'))('prepareOnchainTransaction + optionally payOnchain', async () => {
    const node = makeNode();
    const transaction = await node.prepareOnchainTransaction({
      address: process.env.LND_ONCHAIN_TEST_ADDRESS!,
      amountSats: quoteOnlyAmountSats,
      fee: { type: 'speed', speed: 'normal' },
      feePayer: 'sender',
      description: testInvoiceLabel(runOnchainSend ? 'lnd onchain e2e' : 'lnd onchain quote'),
    });

    console.log('Prepared LND on-chain transaction:', transaction);

    expect(transaction.address).toBe(process.env.LND_ONCHAIN_TEST_ADDRESS);
    expect(transaction.amountSats).toBe(quoteOnlyAmountSats);
    expect(transaction.feePayer).toBe('sender');
    expect(transaction.feeSats).toBeGreaterThanOrEqual(0);

    if (!runOnchainSend) {
      console.log('Prepared LND on-chain quote; skipping broadcast without explicit confirmation');
      return;
    }

    const payment = await node.payOnchain(transaction);

    console.log('LND on-chain payment:', payment);

    expect(payment.state).toBe('pending');
    expect(payment.address).toBe(process.env.LND_ONCHAIN_TEST_ADDRESS);
    expect(payment.amountSats).toBe(quoteOnlyAmountSats);
    expect(payment.txid?.length).toBeGreaterThan(0);
  }, timeout);
});
