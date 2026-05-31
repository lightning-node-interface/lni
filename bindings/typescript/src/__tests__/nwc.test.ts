import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const nwcMocks = vi.hoisted(() => ({
  lookupInvoice: vi.fn(),
  listTransactions: vi.fn(),
  close: vi.fn(),
}));

const bolt11Mocks = vi.hoisted(() => ({
  decode: vi.fn(),
}));

vi.mock('@getalby/sdk/nwc', () => ({
  NWCClient: Object.assign(
    vi.fn().mockImplementation(() => ({
      lookupInvoice: nwcMocks.lookupInvoice,
      listTransactions: nwcMocks.listTransactions,
      close: nwcMocks.close,
    })),
    {
      parseWalletConnectUrl: vi.fn(() => ({ walletPubkey: 'wallet-pubkey' })),
    },
  ),
}));

vi.mock('../decode.js', () => ({
  decode: bolt11Mocks.decode,
  decodeBolt11ToJson: vi.fn((invoice: string) => JSON.stringify(bolt11Mocks.decode(invoice))),
  decodeOfferToJson: vi.fn(),
}));

import { NwcNode } from '../nodes/nwc.js';

const PAYMENT_HASH = '31b06bf9be4c938914030eb23d583a4fe6f6e2f3374293170f027be248ed6370';
const OTHER_PAYMENT_HASH = '0000000000000000000000000000000000000000000000000000000000000000';
const BOLT11_INVOICE = 'lnbc1testinvoice';
const NWC_URI = 'nostr+walletconnect://wallet?relay=wss://relay.example&secret=test';
const originalConsoleError = console.error;

function nwcTransaction(overrides: Record<string, unknown> = {}) {
  return {
    type: 'incoming',
    state: 'settled',
    invoice: BOLT11_INVOICE,
    description: 'test invoice',
    description_hash: '',
    preimage: '',
    payment_hash: PAYMENT_HASH,
    amount: 1234,
    fees_paid: 0,
    created_at: 100,
    expires_at: 200,
    settled_at: 150,
    ...overrides,
  };
}

function makeNode() {
  return new NwcNode({ nwcUri: NWC_URI });
}

function mockBolt11Decode() {
  bolt11Mocks.decode.mockImplementation((invoice: string) => {
    if (invoice !== BOLT11_INVOICE) {
      throw new Error('invalid invoice');
    }

    return {
      payment_hash: PAYMENT_HASH,
    };
  });
}

function mockLookupFailure(error: Error) {
  nwcMocks.lookupInvoice.mockImplementation(async () => {
    console.error('Failed to request lookup_invoice', error);
    throw error;
  });
}

beforeEach(() => {
  nwcMocks.lookupInvoice.mockReset();
  nwcMocks.listTransactions.mockReset();
  nwcMocks.close.mockReset();
  bolt11Mocks.decode.mockReset();
  mockBolt11Decode();
});

afterEach(() => {
  console.error = originalConsoleError;
});

describe('NwcNode.lookupInvoice', () => {
  it('returns successful native lookup_invoice responses', async () => {
    nwcMocks.lookupInvoice.mockResolvedValue(nwcTransaction({ amount: 2100 }));

    const tx = await makeNode().lookupInvoice({ paymentHash: PAYMENT_HASH });

    expect(tx.paymentHash).toBe(PAYMENT_HASH);
    expect(tx.amountMsats).toBe(2100);
    expect(nwcMocks.lookupInvoice).toHaveBeenCalledWith({
      payment_hash: PAYMENT_HASH,
      invoice: undefined,
    });
    expect(nwcMocks.listTransactions).not.toHaveBeenCalled();
  });

  it('falls back to list_transactions when lookup_invoice fails and returns a matching transaction', async () => {
    const lookupError = new Error('error:1e000065:Cipher functions:OPENSSL_internal:BAD_DECRYPT');
    const consoleError = vi.spyOn(console, 'error').mockImplementation(() => undefined);
    mockLookupFailure(lookupError);
    nwcMocks.listTransactions.mockResolvedValue({
      transactions: [
        nwcTransaction({ payment_hash: OTHER_PAYMENT_HASH, invoice: 'lnbc1other' }),
        nwcTransaction({ amount: 5000 }),
      ],
    });

    const tx = await makeNode().lookupInvoice({ paymentHash: PAYMENT_HASH });

    expect(tx.paymentHash).toBe(PAYMENT_HASH);
    expect(tx.amountMsats).toBe(5000);
    expect(nwcMocks.listTransactions).toHaveBeenCalledWith({ from: 0, limit: 100, offset: 0 });
    expect(consoleError).not.toHaveBeenCalled();
  });

  it('paginates list_transactions fallback until it finds a matching transaction', async () => {
    const lookupError = new Error('BAD_DECRYPT');
    const consoleError = vi.spyOn(console, 'error').mockImplementation(() => undefined);
    mockLookupFailure(lookupError);
    const firstPage = Array.from({ length: 100 }, (_, index) =>
      nwcTransaction({
        payment_hash: `${OTHER_PAYMENT_HASH.slice(0, -2)}${String(index).padStart(2, '0')}`,
        invoice: `lnbc1other${index}`,
      }),
    );
    nwcMocks.listTransactions
      .mockResolvedValueOnce({ transactions: firstPage })
      .mockResolvedValueOnce({ transactions: [nwcTransaction({ amount: 7000 })] });

    const tx = await makeNode().lookupInvoice({ paymentHash: PAYMENT_HASH });

    expect(tx.paymentHash).toBe(PAYMENT_HASH);
    expect(tx.amountMsats).toBe(7000);
    expect(nwcMocks.listTransactions).toHaveBeenNthCalledWith(1, { from: 0, limit: 100, offset: 0 });
    expect(nwcMocks.listTransactions).toHaveBeenNthCalledWith(2, { from: 0, limit: 100, offset: 100 });
    expect(consoleError).not.toHaveBeenCalled();
  });

  it('replays the buffered lookup_invoice log and throws the original lookup error when fallback misses', async () => {
    const lookupError = new Error('BAD_DECRYPT');
    const consoleError = vi.spyOn(console, 'error').mockImplementation(() => undefined);
    mockLookupFailure(lookupError);
    nwcMocks.listTransactions.mockResolvedValue({
      transactions: [nwcTransaction({ payment_hash: OTHER_PAYMENT_HASH, invoice: 'lnbc1other' })],
    });

    await expect(makeNode().lookupInvoice({ paymentHash: PAYMENT_HASH })).rejects.toThrow(
      'Failed to lookup invoice: BAD_DECRYPT',
    );

    expect(consoleError).toHaveBeenCalledTimes(1);
    expect(consoleError).toHaveBeenCalledWith('Failed to request lookup_invoice', lookupError);
  });

  it('replays the buffered lookup_invoice log and throws the original lookup error when fallback fails', async () => {
    const lookupError = new Error('BAD_DECRYPT');
    const consoleError = vi.spyOn(console, 'error').mockImplementation(() => undefined);
    mockLookupFailure(lookupError);
    nwcMocks.listTransactions.mockRejectedValue(new Error('list_transactions failed'));

    await expect(makeNode().lookupInvoice({ paymentHash: PAYMENT_HASH })).rejects.toThrow(
      'Failed to lookup invoice: BAD_DECRYPT',
    );

    expect(consoleError).toHaveBeenCalledTimes(1);
    expect(consoleError).toHaveBeenCalledWith('Failed to request lookup_invoice', lookupError);
  });

  it('derives payment hash from BOLT11 invoice-only lookup and matches fallback by payment hash', async () => {
    const lookupError = new Error('BAD_DECRYPT');
    const consoleError = vi.spyOn(console, 'error').mockImplementation(() => undefined);
    mockLookupFailure(lookupError);
    nwcMocks.listTransactions.mockResolvedValue({
      transactions: [nwcTransaction({ invoice: '', payment_hash: PAYMENT_HASH })],
    });

    const tx = await makeNode().lookupInvoice({ search: BOLT11_INVOICE });

    expect(tx.paymentHash).toBe(PAYMENT_HASH);
    expect(nwcMocks.lookupInvoice).toHaveBeenCalledWith({
      payment_hash: PAYMENT_HASH,
      invoice: undefined,
    });
    expect(consoleError).not.toHaveBeenCalled();
  });

  it('suppresses only the GetAlby lookup_invoice log when fallback succeeds', async () => {
    const lookupError = new Error('BAD_DECRYPT');
    const otherError = new Error('keep this log');
    const consoleError = vi.spyOn(console, 'error').mockImplementation(() => undefined);
    nwcMocks.lookupInvoice.mockImplementation(async () => {
      console.error('Failed to request lookup_invoice', lookupError);
      console.error('unrelated error', otherError);
      throw lookupError;
    });
    nwcMocks.listTransactions.mockResolvedValue({
      transactions: [nwcTransaction()],
    });

    const tx = await makeNode().lookupInvoice({ paymentHash: PAYMENT_HASH });

    expect(tx.paymentHash).toBe(PAYMENT_HASH);
    expect(consoleError).toHaveBeenCalledTimes(1);
    expect(consoleError).toHaveBeenCalledWith('unrelated error', otherError);
  });
});
