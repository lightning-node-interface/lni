import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const nwcMocks = vi.hoisted(() => ({
  Nip47Error: class Nip47Error extends Error {
    code: string;

    constructor(message: string, code: string) {
      super(message);
      this.code = code;
    }
  },
  getInfo: vi.fn(),
  getBalance: vi.fn(),
  makeInvoice: vi.fn(),
  payInvoice: vi.fn(),
  lookupInvoice: vi.fn(),
  listTransactions: vi.fn(),
  executeNip47Request: vi.fn(),
  close: vi.fn(),
  usePrivateExecute: false,
}));

const bolt11Mocks = vi.hoisted(() => ({
  decode: vi.fn(),
}));

vi.mock('@getalby/sdk/nwc', () => ({
  Nip47Error: nwcMocks.Nip47Error,
  NWCClient: Object.assign(
    vi.fn().mockImplementation(() => {
      const client = {
        getInfo: nwcMocks.getInfo,
        getBalance: nwcMocks.getBalance,
        makeInvoice: nwcMocks.makeInvoice,
        payInvoice: nwcMocks.payInvoice,
        lookupInvoice: nwcMocks.lookupInvoice,
        listTransactions: nwcMocks.listTransactions,
        close: nwcMocks.close,
      };

      if (nwcMocks.usePrivateExecute) {
        return {
          ...client,
          executeNip47Request: nwcMocks.executeNip47Request,
        };
      }

      return client;
    }),
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
import { NwcError } from '../errors.js';
import { registerSha256DigestFallback } from '../internal/sha256.js';

const PAYMENT_HASH = '31b06bf9be4c938914030eb23d583a4fe6f6e2f3374293170f027be248ed6370';
const OTHER_PAYMENT_HASH = '0000000000000000000000000000000000000000000000000000000000000000';
const ZERO_PREIMAGE = '0000000000000000000000000000000000000000000000000000000000000000';
const ZERO_PREIMAGE_PAYMENT_HASH = '66687aadf862bd776c8fc18b8e9f8e20089714856ee233b3902a591d0d5f2925';
const BOLT11_INVOICE = 'lnbc1testinvoice';
const NWC_URI = 'nostr+walletconnect://wallet?relay=wss://relay.example&secret=test';
const NWC_URI_WITH_LUD16 = `${NWC_URI}&lud16=test%40example.com`;
const originalConsoleError = console.error;
const originalCryptoDescriptor = Object.getOwnPropertyDescriptor(globalThis, 'crypto');

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

function makeNodeWithTimeout(httpTimeout: number) {
  return new NwcNode({ nwcUri: NWC_URI, httpTimeout });
}

function makeJsonResponse(body: unknown): Response {
  return new Response(JSON.stringify(body), {
    headers: {
      'content-type': 'application/json',
    },
  });
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
  nwcMocks.getInfo.mockReset();
  nwcMocks.getBalance.mockReset();
  nwcMocks.makeInvoice.mockReset();
  nwcMocks.payInvoice.mockReset();
  nwcMocks.lookupInvoice.mockReset();
  nwcMocks.listTransactions.mockReset();
  nwcMocks.executeNip47Request.mockReset();
  nwcMocks.close.mockReset();
  nwcMocks.usePrivateExecute = false;
  bolt11Mocks.decode.mockReset();
  mockBolt11Decode();
});

afterEach(() => {
  vi.useRealTimers();
  console.error = originalConsoleError;
  registerSha256DigestFallback(undefined);

  if (originalCryptoDescriptor) {
    Object.defineProperty(globalThis, 'crypto', originalCryptoDescriptor);
  } else {
    delete (globalThis as { crypto?: Crypto }).crypto;
  }
});

describe('NwcNode.payInvoice', () => {
  it('passes httpTimeout to the SDK NIP-47 timeout values when the SDK request API is available', async () => {
    nwcMocks.usePrivateExecute = true;
    nwcMocks.executeNip47Request.mockResolvedValue({
      preimage: '',
      fees_paid: 21,
    });

    const response = await makeNodeWithTimeout(15).payInvoice({ invoice: BOLT11_INVOICE });

    expect(response).toEqual({
      paymentHash: '',
      preimage: '',
      feeMsats: 21,
    });
    expect(nwcMocks.executeNip47Request).toHaveBeenCalledWith(
      'pay_invoice',
      {
        invoice: BOLT11_INVOICE,
        amount: undefined,
      },
      expect.any(Function),
      {
        replyTimeout: 15_000,
        publishTimeout: 5_000,
      },
    );
    expect(nwcMocks.payInvoice).not.toHaveBeenCalled();
  });

  it('times out and closes the NWC client when the websocket request never settles', async () => {
    vi.useFakeTimers();
    nwcMocks.payInvoice.mockReturnValue(new Promise(() => undefined));

    const response = makeNodeWithTimeout(0.001).payInvoice({ invoice: BOLT11_INVOICE });
    const assertion = expect(response).rejects.toMatchObject({
      code: 'NetworkError',
      message:
        'Failed to pay invoice: NWC pay invoice timed out after 0.001s. Check that the relay websocket is reachable and the wallet connection still exists.',
    });

    await vi.advanceTimersByTimeAsync(1);
    await assertion;
    expect(nwcMocks.close).toHaveBeenCalledTimes(1);
  });

  it('preserves typed NIP-47 wallet error codes from the SDK', async () => {
    nwcMocks.payInvoice.mockRejectedValue(new nwcMocks.Nip47Error('quota spent', 'QUOTA_EXCEEDED'));

    await expect(makeNode().payInvoice({ invoice: BOLT11_INVOICE })).rejects.toMatchObject({
      name: 'NwcError',
      code: 'NwcError',
      nwcCode: 'QUOTA_EXCEEDED',
      nwcMessage: 'quota spent',
      operation: 'pay_invoice',
      message: 'quota spent',
    });

    await expect(makeNode().payInvoice({ invoice: BOLT11_INVOICE })).rejects.toBeInstanceOf(NwcError);
  });

  it('hashes returned preimages with a registered fallback when global crypto.subtle is absent', async () => {
    Object.defineProperty(globalThis, 'crypto', {
      configurable: true,
      value: {},
    });
    registerSha256DigestFallback(async () => hexToBytesForTest(ZERO_PREIMAGE_PAYMENT_HASH));
    nwcMocks.payInvoice.mockResolvedValue({
      preimage: ZERO_PREIMAGE,
      fees_paid: 21,
    });

    const response = await makeNode().payInvoice({ invoice: BOLT11_INVOICE });

    expect(response).toEqual({
      paymentHash: ZERO_PREIMAGE_PAYMENT_HASH,
      preimage: ZERO_PREIMAGE,
      feeMsats: 21,
    });
    expect(nwcMocks.payInvoice).toHaveBeenCalledWith({
      invoice: BOLT11_INVOICE,
      amount: undefined,
    });
  });

  it('throws a clear error when no SHA-256 implementation is available', async () => {
    Object.defineProperty(globalThis, 'crypto', {
      configurable: true,
      value: {},
    });
    nwcMocks.payInvoice.mockResolvedValue({
      preimage: ZERO_PREIMAGE,
      fees_paid: 21,
    });

    await expect(makeNode().payInvoice({ invoice: BOLT11_INVOICE })).rejects.toThrow(
      'Web Crypto API or a registered SHA-256 digest fallback is required to hash NWC preimages.',
    );
  });
});

describe('NwcNode.getLightningAddress', () => {
  it('returns the lud16 Lightning Address and true when LNURL verify succeeds', async () => {
    const fetchMock = vi
      .fn<typeof fetch>()
      .mockResolvedValueOnce(
        makeJsonResponse({
          callback: 'https://example.com/lnurl/callback',
          maxSendable: 500_000_000,
          minSendable: 1,
          metadata: '[["text/plain","test"]]',
          tag: 'payRequest',
        }),
      )
      .mockResolvedValueOnce(
        makeJsonResponse({
          pr: 'lnbc1testinvoice',
          verify: 'https://example.com/lnurl/verify',
        }),
      )
      .mockResolvedValueOnce(makeJsonResponse({ status: 'OK' }));

    const response = await new NwcNode({ nwcUri: NWC_URI_WITH_LUD16 }, { fetch: fetchMock }).getLightningAddress();

    expect(response).toEqual({
      lightningAddress: 'test@example.com',
      lnurlVerifySupported: true,
    });
    expect(fetchMock).toHaveBeenCalledTimes(3);
    expect(fetchMock.mock.calls[0]?.[0]).toBe('https://example.com/.well-known/lnurlp/test');
    expect(fetchMock.mock.calls[1]?.[0]).toBe('https://example.com/lnurl/callback?amount=100000');
    expect(fetchMock.mock.calls[2]?.[0]).toBe('https://example.com/lnurl/verify');
  });

  it('returns false when the Lightning Address LNURL callback has no verify endpoint', async () => {
    const fetchMock = vi
      .fn<typeof fetch>()
      .mockResolvedValueOnce(
        makeJsonResponse({
          callback: 'https://example.com/lnurl/callback',
          maxSendable: 500_000_000,
          minSendable: 1,
          metadata: '[["text/plain","test"]]',
          tag: 'payRequest',
        }),
      )
      .mockResolvedValueOnce(makeJsonResponse({ pr: 'lnbc1testinvoice' }));

    const response = await new NwcNode({ nwcUri: NWC_URI_WITH_LUD16 }, { fetch: fetchMock }).getLightningAddress();

    expect(response).toEqual({
      lightningAddress: 'test@example.com',
      lnurlVerifySupported: false,
    });
    expect(fetchMock).toHaveBeenCalledTimes(2);
  });

  it('throws clearly when the NWC URI has no lud16 value', async () => {
    await expect(makeNode().getLightningAddress()).rejects.toThrow(
      'NWC URI does not include a lud16 Lightning Address.',
    );
  });
});

function hexToBytesForTest(hex: string): Uint8Array {
  const bytes = new Uint8Array(hex.length / 2);
  for (let i = 0; i < bytes.length; i += 1) {
    bytes[i] = Number.parseInt(hex.slice(i * 2, i * 2 + 2), 16);
  }

  return bytes;
}

describe('NwcNode.lookupInvoice', () => {
  it('does not try the list_transactions fallback when lookup_invoice times out', async () => {
    vi.useFakeTimers();
    nwcMocks.lookupInvoice.mockReturnValue(new Promise(() => undefined));

    const response = makeNodeWithTimeout(0.001).lookupInvoice({ paymentHash: PAYMENT_HASH });
    const assertion = expect(response).rejects.toMatchObject({
      code: 'NetworkError',
      message:
        'Failed to lookup invoice: NWC lookup invoice timed out after 0.001s. Check that the relay websocket is reachable and the wallet connection still exists.',
    });

    await vi.advanceTimersByTimeAsync(1);
    await assertion;
    expect(nwcMocks.listTransactions).not.toHaveBeenCalled();
    expect(nwcMocks.close).toHaveBeenCalledTimes(1);
  });

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
