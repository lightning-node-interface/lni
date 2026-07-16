import { createHash } from 'node:crypto';
import { describe, expect } from 'vitest';
import { BlinkNode } from '../../nodes/blink.js';
import {
  hasEnv,
  itIf,
  nonEmpty,
  runOrSkipKnownError,
  testInvoiceLabel,
  timeout,
  uniqueValues,
} from './helpers.js';

const ONCHAIN_SEND_CONFIRMATION = 'I_UNDERSTAND_THIS_BROADCASTS_BITCOIN';
const DEFAULT_ONCHAIN_QUOTE_AMOUNT_SATS = 10_000;

describe('Real integration from crates/lni/.env > BlinkNode', () => {
  const enabled = hasEnv('BLINK_API_KEY');
  const onchainSendAmountSats = Number.parseInt(process.env.BLINK_ONCHAIN_AMOUNT_SATS ?? '', 10);
  const quoteOnlyAmountSats =
    Number.isSafeInteger(onchainSendAmountSats) && onchainSendAmountSats > 0
      ? onchainSendAmountSats
      : DEFAULT_ONCHAIN_QUOTE_AMOUNT_SATS;
  const runOnchainSend =
    enabled &&
    process.env.BLINK_RUN_ONCHAIN_SEND === 'true' &&
    process.env.BLINK_ONCHAIN_SEND_CONFIRM === ONCHAIN_SEND_CONFIRMATION &&
    hasEnv('BLINK_ONCHAIN_TEST_ADDRESS', 'BLINK_ONCHAIN_AMOUNT_SATS') &&
    Number.isSafeInteger(onchainSendAmountSats) &&
    onchainSendAmountSats > 0;

  const makeNode = () =>
    new BlinkNode({
      apiKey: process.env.BLINK_API_KEY!,
      baseUrl: nonEmpty(process.env.BLINK_BASE_URL),
    });

  itIf(enabled)(
    'getInfo + createInvoice + listTransactions',
    async () => {
      const node = makeNode();
      const info = await node.getInfo();
      expect(typeof info.alias).toBe('string');

      const invoice = await node.createInvoice({
        amountMsats: 5_000,
        description: testInvoiceLabel('blink'),
      });
      console.log('Blink Invoice:', invoice);
      expect(invoice.invoice.length).toBeGreaterThan(0);

      const txs = await node.listTransactions({ from: 0, limit: 25 });
      expect(Array.isArray(txs)).toBe(true);
    },
    timeout
  );

  itIf(enabled)(
    'lookupInvoice (best effort from env or recent tx)',
    async () => {
      await runOrSkipKnownError(async () => {
        const node = makeNode();
        const txs = await node.listTransactions({ from: 0, limit: 50 });
        const candidateHash = txs.find((tx) => tx.paymentHash.length > 0)?.paymentHash;
        const hashes = uniqueValues([process.env.BLINK_TEST_PAYMENT_HASH, candidateHash]);

        if (!hashes.length) {
          return;
        }

        let lastError: unknown;
        for (const paymentHash of hashes) {
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
      }, ['transaction not found', 'http 404']);
    },
    timeout
  );

  // itIf(enabled && hasEnv('BLINK_TEST_PAYMENT_REQUEST'))(
  //   'payInvoice returns a valid Lightning preimage',
  //   async () => {
  //     const node = makeNode();
  //     const payment = await node.payInvoice({
  //       invoice: process.env.BLINK_TEST_PAYMENT_REQUEST!,
  //     });
  //     console.log('Blink payInvoice result:', payment);
  //     expect(payment.paymentHash).toMatch(/^[0-9a-f]{64}$/i);
  //     expect(payment.preimage).toMatch(/^[0-9a-f]{64}$/i);
  //     expect(payment.feeMsats).toBeGreaterThanOrEqual(0);

  //     const paymentHashFromPreimage = createHash('sha256')
  //       .update(Buffer.from(payment.preimage, 'hex'))
  //       .digest('hex');
  //     expect(paymentHashFromPreimage).toBe(payment.paymentHash.toLowerCase());
  //   },
  //   timeout
  // );

  itIf(enabled && hasEnv('BLINK_ONCHAIN_TEST_ADDRESS'))(
    'prepareOnchainTransaction + optionally payOnchain',
    async () => {
      const node = makeNode();
      let transaction: Awaited<ReturnType<BlinkNode['prepareOnchainTransaction']>> | undefined;

      await runOrSkipKnownError(async () => {
        transaction = await node.prepareOnchainTransaction({
          address: process.env.BLINK_ONCHAIN_TEST_ADDRESS!,
          amountSats: quoteOnlyAmountSats,
          fee: { type: 'speed', speed: runOnchainSend ? 'fast' : 'normal' },
          feePayer: 'sender',
          description: testInvoiceLabel(
            runOnchainSend ? 'blink onchain e2e' : 'blink onchain quote'
          ),
        });

        expect(transaction.address).toBe(process.env.BLINK_ONCHAIN_TEST_ADDRESS);
        expect(transaction.amountSats).toBe(quoteOnlyAmountSats);
        expect(transaction.feePayer).toBe('sender');
        expect(transaction.feeSats).toBeGreaterThanOrEqual(0);
      }, ['insufficient balance', 'invalid address', 'amount']);

      console.log('Prepared Blink on-chain transaction:', transaction);
      if (!transaction) {
        return;
      }

      if (!runOnchainSend) {
        console.log(
          'Prepared Blink on-chain quote; skipping broadcast without explicit confirmation'
        );
        return;
      }

      // Blink can quote high miner/provider fees for small test sends; keep the absolute guardrail
      // default, but allow this real test up to 52% before broadcasting.
      const payment = await node.payOnchain(transaction, {
        feeGuardrail: {
          maxFeePercent: 52,
        },
      });
      console.log('Blink on-chain payment result:', payment);

      expect(['pending', 'completed']).toContain(payment.state);
      expect(payment.address).toBe(process.env.BLINK_ONCHAIN_TEST_ADDRESS);
    },
    timeout
  );
});
