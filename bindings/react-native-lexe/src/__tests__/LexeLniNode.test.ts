import { LniError } from '@sunnyln/lni';
import type { LightningNode } from '@sunnyln/lni';
import { describe, expect, it, vi } from 'vitest';

import { LexeLniNode } from '../LexeLniNode';
import type {
  LexeNodeLike,
  NodeInfo,
  OnInvoiceEventCallback,
  PayInvoiceParams,
  Permissions,
  Transaction,
} from '../generated/react_native_lexe';

type TestNativeNode = LexeNodeLike & { uniffiDestroy(): void };

const nativeTransaction = (
  overrides: Partial<Transaction> = {}
): Transaction => ({
  type: 'incoming',
  invoice: 'test-invoice',
  description: 'test payment',
  descriptionHash: '',
  preimage: 'test-preimage',
  paymentHash: 'test-payment-hash',
  amountMsats: 42_000n,
  feesPaid: 21n,
  createdAt: 1_700_000_000n,
  expiresAt: 1_700_003_600n,
  settledAt: 1_700_000_010n,
  ...overrides,
});

const nativeInfo: NodeInfo = {
  alias: 'Lexe',
  color: '#000000',
  pubkey: 'test-pubkey',
  network: 'mainnet',
  blockHeight: 800_000n,
  blockHash: 'test-block-hash',
  sendBalanceMsat: 1n,
  receiveBalanceMsat: 2n,
  feeCreditBalanceMsat: 3n,
  unsettledSendBalanceMsat: 4n,
  unsettledReceiveBalanceMsat: 5n,
  pendingOpenSendBalance: 6n,
  pendingOpenReceiveBalance: 7n,
};

const nativePermissions: Permissions = {
  getInfo: true,
  createInvoice: true,
  payInvoice: true,
  createOffer: true,
  getOffer: true,
  listOffers: true,
  payOffer: true,
  lookupInvoice: true,
  listTransactions: true,
  decode: true,
  onInvoiceEvents: true,
};

function makeNativeNode(
  overrides: Partial<TestNativeNode> = {}
): TestNativeNode {
  return {
    createInvoice: vi.fn(async () => nativeTransaction()),
    createOffer: vi.fn(async () => ({ offerId: 'id', bolt12: 'offer' })),
    decode: vi.fn(async (value: string) => value),
    decodeOffer: vi.fn(async (offer: string) => offer),
    getInfo: vi.fn(async () => nativeInfo),
    getOffer: vi.fn(async () => ({ offerId: 'id', bolt12: 'offer' })),
    getPermissions: vi.fn(async () => nativePermissions),
    listOffers: vi.fn(async () => []),
    listTransactions: vi.fn(async () => [nativeTransaction()]),
    lookupInvoice: vi.fn(async () => nativeTransaction()),
    onInvoiceEvents: vi.fn(async () => undefined),
    payInvoice: vi.fn(async () => ({
      paymentHash: 'test-payment-hash',
      preimage: 'test-preimage',
      feeMsats: 21n,
    })),
    payOffer: vi.fn(async () => ({
      paymentHash: 'test-payment-hash',
      preimage: 'test-preimage',
      feeMsats: 21n,
    })),
    uniffiDestroy: vi.fn(),
    ...overrides,
  };
}

function makeNode(nativeNode = makeNativeNode()): LexeLniNode {
  return new LexeLniNode(
    {
      clientCredentials: 'test-client-credentials',
      dataDir: '/app/documents/lexe',
      network: 'mainnet',
    },
    nativeNode
  );
}

describe('LexeLniNode', () => {
  it('converts pay-invoice integer parameters to bigint', async () => {
    const payInvoice = vi.fn(async (_params: PayInvoiceParams) => ({
      paymentHash: 'test-payment-hash',
      preimage: 'test-preimage',
      feeMsats: 21n,
    }));
    const node = makeNode(makeNativeNode({ payInvoice }));

    await node.payInvoice({
      invoice: 'test-invoice',
      feeLimitMsat: 100,
      feeLimitPercentage: 1.5,
      timeoutSeconds: 60,
      amountMsats: 42_000,
      maxParts: 4,
      firstHopPubkey: 'first-hop',
      lastHopPubkey: 'last-hop',
      allowSelfPayment: true,
      isAmp: false,
    });

    expect(payInvoice).toHaveBeenCalledWith({
      invoice: 'test-invoice',
      feeLimitMsat: 100n,
      feeLimitPercentage: 1.5,
      timeoutSeconds: 60n,
      amountMsats: 42_000n,
      maxParts: 4n,
      firstHopPubkey: 'first-hop',
      lastHopPubkey: 'last-hop',
      allowSelfPayment: true,
      isAmp: false,
    });
  });

  it('converts pay responses to safe numbers', async () => {
    const node = makeNode();

    await expect(node.payInvoice({ invoice: 'test-invoice' })).resolves.toEqual(
      {
        paymentHash: 'test-payment-hash',
        preimage: 'test-preimage',
        feeMsats: 21,
      }
    );
  });

  it('rejects bigint responses outside the safe-integer range', async () => {
    const nativeNode = makeNativeNode({
      payInvoice: vi.fn(async () => ({
        paymentHash: 'test-payment-hash',
        preimage: 'test-preimage',
        feeMsats: BigInt(Number.MAX_SAFE_INTEGER) + 1n,
      })),
    });
    const node = makeNode(nativeNode);

    await expect(node.payInvoice({ invoice: 'test-invoice' })).rejects.toEqual(
      expect.objectContaining({
        name: 'LniError',
        code: 'InvalidInput',
      })
    );
  });

  it('converts native transactions to shared LNI transactions', async () => {
    const node = makeNode();

    await expect(
      node.listTransactions({ from: 0, limit: 10 })
    ).resolves.toEqual([
      {
        type: 'incoming',
        invoice: 'test-invoice',
        description: 'test payment',
        descriptionHash: '',
        preimage: 'test-preimage',
        paymentHash: 'test-payment-hash',
        amountMsats: 42_000,
        feesPaid: 21,
        createdAt: 1_700_000_000,
        expiresAt: 1_700_003_600,
        settledAt: 1_700_000_010,
        payerNote: undefined,
        externalId: undefined,
      },
    ]);
  });

  it('adapts invoice-event params and callbacks', async () => {
    let nativeCallback: OnInvoiceEventCallback | undefined;
    const onInvoiceEvents = vi.fn(async (_params, callback) => {
      nativeCallback = callback;
    });
    const node = makeNode(makeNativeNode({ onInvoiceEvents }));
    const callback = vi.fn();

    await node.onInvoiceEvents(
      {
        paymentHash: 'test-payment-hash',
        pollingDelaySec: 2,
        maxPollingSec: 30,
      },
      callback
    );

    expect(onInvoiceEvents).toHaveBeenCalledWith(
      {
        paymentHash: 'test-payment-hash',
        search: undefined,
        pollingDelaySec: 2n,
        maxPollingSec: 30n,
      },
      expect.any(Object)
    );

    nativeCallback?.pending(nativeTransaction({ amountMsats: 9n }));
    nativeCallback?.failure(undefined);

    expect(callback).toHaveBeenNthCalledWith(
      1,
      'pending',
      expect.objectContaining({ amountMsats: 9 })
    );
    expect(callback).toHaveBeenNthCalledWith(2, 'failure', undefined);
  });

  it('preserves nested UniFFI error messages in LniError', async () => {
    const nativeError = {
      message: 'LexeError: Lni',
      inner: { message: 'payment was rejected by Lexe' },
    };
    const node = makeNode(
      makeNativeNode({
        payInvoice: vi.fn(async () => {
          throw nativeError;
        }),
      })
    );

    const error = await node
      .payInvoice({ invoice: 'test-invoice' })
      .then(() => undefined)
      .catch((cause: unknown) => cause);

    expect(error).toBeInstanceOf(LniError);
    expect(error).toEqual(
      expect.objectContaining({
        name: 'LniError',
        code: 'Api',
        message: 'payment was rejected by Lexe',
        cause: nativeError,
      })
    );
  });

  it('destroys the native node only once', () => {
    const uniffiDestroy = vi.fn();
    const node = makeNode(makeNativeNode({ uniffiDestroy }));

    node.close();
    node.close();

    expect(uniffiDestroy).toHaveBeenCalledTimes(1);
  });

  it('conforms to the shared LightningNode interface', () => {
    const node: LightningNode = makeNode();
    expect(node).toBeInstanceOf(LexeLniNode);
  });
});
