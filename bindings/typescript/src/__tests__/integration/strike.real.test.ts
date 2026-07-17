import { createHash } from 'node:crypto';
import { describe, expect } from 'vitest';
import { StrikeNode } from '../../nodes/strike.js';
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

describe('Real integration from crates/lni/.env > StrikeNode', () => {
  const enabled = hasEnv('STRIKE_API_KEY');
  const onchainSendAmountSats = Number.parseInt(process.env.STRIKE_ONCHAIN_AMOUNT_SATS ?? '', 10);
  const quoteOnlyAmountSats =
    Number.isSafeInteger(onchainSendAmountSats) && onchainSendAmountSats > 0
      ? onchainSendAmountSats
      : DEFAULT_ONCHAIN_QUOTE_AMOUNT_SATS;
  const runOnchainSend =
    enabled &&
    process.env.STRIKE_RUN_ONCHAIN_SEND === 'true' &&
    process.env.STRIKE_ONCHAIN_SEND_CONFIRM === ONCHAIN_SEND_CONFIRMATION &&
    hasEnv('STRIKE_ONCHAIN_TEST_ADDRESS', 'STRIKE_ONCHAIN_AMOUNT_SATS') &&
    Number.isSafeInteger(onchainSendAmountSats) &&
    onchainSendAmountSats > 0;

  const makeNode = () =>
    new StrikeNode({
      apiKey: process.env.STRIKE_API_KEY!,
      baseUrl: nonEmpty(process.env.STRIKE_BASE_URL),
    });

  itIf(enabled)(
    'getInfo + createInvoice + listTransactions',
    async () => {
      const node = makeNode();
      const info = await node.getInfo();
      expect(typeof info.alias).toBe('string');

      const invoice = await node.createInvoice({
        amountMsats: 10_000,
        description: testInvoiceLabel('strike'),
      });
      console.log('Strike Invoice:', invoice);
      expect(invoice.invoice.length).toBeGreaterThan(0);
      expect(invoice.paymentHash.length).toBeGreaterThan(0);

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
        const lookupHashes = uniqueValues([process.env.STRIKE_TEST_PAYMENT_HASH, candidateHash]);

        if (!lookupHashes.length) {
          return;
        }

        let lastError: unknown;
        for (const paymentHash of lookupHashes) {
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
      }, ['no receive found', 'http 404']);
    },
    timeout
  );

  itIf(enabled && hasEnv('STRIKE_TEST_PAYMENT_REQUEST'))(
    'payInvoice returns a valid Lightning preimage',
    async () => {
      const node = makeNode();
      const payment = await node.payInvoice({
        invoice: process.env.STRIKE_TEST_PAYMENT_REQUEST!,
      });
      console.log('Strike payInvoice result:', payment);

      expect(payment.paymentHash).toMatch(/^[0-9a-f]{64}$/i);
      expect(payment.preimage).toMatch(/^[0-9a-f]{64}$/i);
      expect(payment.feeMsats).toBeGreaterThanOrEqual(0);

      const paymentHashFromPreimage = createHash('sha256')
        .update(Buffer.from(payment.preimage, 'hex'))
        .digest('hex');
      expect(paymentHashFromPreimage).toBe(payment.paymentHash.toLowerCase());
    },
    timeout
  );

  itIf(enabled && hasEnv('STRIKE_ONCHAIN_TEST_ADDRESS'))(
    'prepareOnchainTransaction quote only',
    async () => {
      await runOrSkipKnownError(async () => {
        const node = makeNode();
        const transaction = await node.prepareOnchainTransaction({
          address: process.env.STRIKE_ONCHAIN_TEST_ADDRESS!,
          amountSats: quoteOnlyAmountSats,
          fee: { type: 'speed', speed: 'free' },
          feePayer: 'sender',
          description: testInvoiceLabel('strike onchain quote'),
        });

        expect(transaction.id?.length).toBeGreaterThan(0);
        expect(transaction.address).toBe(process.env.STRIKE_ONCHAIN_TEST_ADDRESS);
        expect(transaction.amountSats).toBe(quoteOnlyAmountSats);
      }, ['forbidden', 'scope', 'minimum amount', 'not supported', 'invalid bitcoin address']);
    },
    timeout
  );

  itIf(runOnchainSend)(
    'prepareOnchainTransaction + payOnchain broadcasts an on-chain payment',
    async () => {
      const node = makeNode();
      const address = process.env.STRIKE_ONCHAIN_TEST_ADDRESS!;
      const transaction = await node.prepareOnchainTransaction({
        address,
        amountSats: onchainSendAmountSats,
        fee: { type: 'speed', speed: 'fast' },
        feePayer: 'sender',
        description: testInvoiceLabel('strike onchain e2e'),
        idempotencyKey: crypto.randomUUID(),
      });

      console.log('Prepared on-chain transaction:', transaction);
      // https://mempool.space/signet/address/tb1qwpk7qm4v3gxwz7u0z0fq8lkwkrpn2twmh90uy0
      // Prepared on-chain transaction: {
      //   id: 'a1870d73-3a58-4554-abeb-a79a4c26ab02',
      //   address: 'tb1qwpk7qm4v3gxwz7u0z0fq8lkwkrpn2twmh90uy0',
      //   amountSats: 76000,
      //   feeSats: 2047,
      //   totalAmountSats: 78047,
      //   recipientAmountSats: 76000,
      //   feePayer: 'sender',
      //   fee: { type: 'speed', speed: 'fast' },
      //   expiresAt: 1780197065,
      //   estimatedDeliverySeconds: 600,
      //   raw: {
      //     estimatedDeliveryDurationInMin: 10,
      //     paymentQuoteId: 'a1870d73-3a58-4554-abeb-a79a4c26ab02',
      //     description: 'strike onchain e2e ts integration 1780193465605',
      //     validUntil: '2026-05-31T03:11:05.9996885+00:00',
      //     amount: { amount: '0.00076', currency: 'BTC' },
      //     totalFee: { amount: '0.00002047', currency: 'BTC' },
      //     totalAmount: { amount: '0.00078047', currency: 'BTC' }
      //   }
      // }

      expect(transaction.id?.length).toBeGreaterThan(0);
      expect(transaction.address).toBe(address);
      expect(transaction.amountSats).toBe(onchainSendAmountSats);

      // payOnchain enforces the default fee guardrail before broadcasting.

      const payment = await node.payOnchain(transaction);

      expect(['pending', 'completed']).toContain(payment.state);
      expect(payment.address).toBe(address);
      expect(payment.amountSats).toBe(onchainSendAmountSats);
    },
    timeout
  );
});
