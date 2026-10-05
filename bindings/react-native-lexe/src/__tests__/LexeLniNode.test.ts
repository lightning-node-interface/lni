import { LniError } from '@sunnyln/lni';
import type { LightningNode, OnchainPayments } from '@sunnyln/lni';
import { describe, expect, it, vi } from 'vitest';

import { LexeLniNode } from '../LexeLniNode';
import {
  OnchainFeePayer,
  OnchainFeePreferenceType,
  OnchainFeeSpeed,
} from '../generated/react_native_lexe';
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
    prepareOnchainTransaction: vi.fn(async (params) => ({
      feeLimitSupported: false,
      id: params.idempotencyKey ?? 'ab'.repeat(32),
      address: params.address,
      amountSats: params.amountSats,
      recipientAmountSats: params.amountSats,
      feePayer: OnchainFeePayer.Sender,
      fee: params.fee ?? { preferenceType: OnchainFeePreferenceType.Default },
      raw: JSON.stringify(params.description ?? null),
    })),
    payOnchain: vi.fn(async (transaction) => ({
      paymentId: 'payment-index',
      txid: 'cd'.repeat(32),
      state: 'pending',
      address: transaction.address,
      amountSats: transaction.amountSats,
      feeSats: 250n,
      totalAmountSats: transaction.amountSats + 250n,
      recipientAmountSats: transaction.amountSats,
      createdAt: 1_700_000_000n,
    })),
    createInvoice: vi.fn(async () => nativeTransaction()),
    createOffer: vi.fn(async () => ({ offerId: 'id', bolt12: 'offer' })),
    decode: vi.fn(async (value: string) => value),
    decodeOffer: vi.fn(async (offer: string) => offer),
    getClientInfo: vi.fn(async () => ({
      kind: 'client_credentials',
      clientPubkey: 'client-public-key',
      label: 'Zaprite spending',
      createdAtMs: 1_700_000_000_000n,
      expiresAtMs: undefined,
      scopes: ['read_info'],
      permissions: [],
      effectivePermissions: ['node_info'],
    })),
    getInfo: vi.fn(async () => nativeInfo),
    getHumanBitcoinAddress: vi.fn(async () => ({
      humanBitcoinAddress: '₿test@lexe.app',
      lightningAddress: 'test@lexe.app',
      offer: 'lno1test',
      updatable: true,
    })),
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

  it('returns the wallet Human Bitcoin Address', async () => {
    const node = makeNode();

    await expect(node.getHumanBitcoinAddress()).resolves.toEqual({
      humanBitcoinAddress: '₿test@lexe.app',
      lightningAddress: 'test@lexe.app',
      offer: 'lno1test',
      updatable: true,
    });
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

describe('authenticated Lexe grants', () => {
  it('preserves actual read-only grants independently of adapter capabilities', async () => {
    const native = makeNativeNode();
    const node = makeNode(native);
    expect((await node.getPermissions()).payInvoice).toBe(true);
    expect(await node.getClientInfo()).toEqual({
      kind: 'client_credentials',
      clientPubkey: 'client-public-key',
      label: 'Zaprite spending',
      createdAtMs: 1_700_000_000_000,
      expiresAtMs: undefined,
      scopes: ['read_info'],
      permissions: [],
      effectivePermissions: ['node_info'],
    });
    expect(native.getClientInfo).toHaveBeenCalledOnce();
  });
  it('propagates inspection failures without inventing a grant', async () => {
    const node = makeNode(
      makeNativeNode({
        getClientInfo: vi.fn(async () => {
          throw new Error('inspection unavailable');
        }),
      })
    );
    await expect(node.getClientInfo()).rejects.toThrow(
      'inspection unavailable'
    );
  });
});

describe('Lexe on-chain adapter', () => {
  it('preserves the prepared id and note through native execution', async () => {
    const native = makeNativeNode();
    const node: OnchainPayments = new LexeLniNode(
      { clientCredentials: 'test' },
      native
    );
    const transaction = await node.prepareOnchainTransaction({
      address: 'test-address',
      amountSats: 10_000,
      fee: { type: 'speed', speed: 'slow' },
      description: 'test note',
      idempotencyKey: 'ab'.repeat(32),
    });
    expect(native.prepareOnchainTransaction).toHaveBeenCalledWith(
      expect.objectContaining({
        amountSats: 10_000n,
        fee: {
          preferenceType: OnchainFeePreferenceType.Speed,
          speed: OnchainFeeSpeed.Slow,
        },
      })
    );
    expect(transaction.feeSats).toBeUndefined();
    expect(transaction.raw).toBe('test note');
    expect(transaction).toMatchObject({
      feeLimitSupported: false,
    });
    const result = await node.payOnchain(transaction);
    expect(native.payOnchain).toHaveBeenCalledWith(
      expect.objectContaining({
        id: 'ab'.repeat(32),
        amountSats: 10_000n,
        raw: '"test note"',
      }),
      { dangerouslyDisableFeeGuardrail: false, feeGuardrail: undefined }
    );
    expect(result).toMatchObject({
      state: 'pending',
      amountSats: 10_000,
      feeSats: 250,
      totalAmountSats: 10_250,
      txid: 'cd'.repeat(32),
    });
    await node.payOnchain(transaction);
    expect(native.payOnchain).toHaveBeenLastCalledWith(expect.anything(), {
      dangerouslyDisableFeeGuardrail: false,
      feeGuardrail: undefined,
    });
  });

  it('rejects unsafe amounts and unsupported fee controls before calling native code', async () => {
    const native = makeNativeNode();
    const node = new LexeLniNode({ clientCredentials: 'test' }, native);
    for (const amountSats of [
      NaN,
      Infinity,
      1.5,
      Number.MAX_SAFE_INTEGER + 1,
    ]) {
      await expect(
        node.prepareOnchainTransaction({ address: 'test', amountSats })
      ).rejects.toBeInstanceOf(LniError);
    }
    await expect(
      node.prepareOnchainTransaction({
        address: 'test',
        amountSats: 1,
        fee: { type: 'satsPerVbyte', satsPerVbyte: 2 },
      })
    ).rejects.toBeInstanceOf(LniError);
    await expect(
      node.prepareOnchainTransaction({
        address: 'test',
        amountSats: 1,
        feePayer: 'recipient',
      })
    ).rejects.toBeInstanceOf(LniError);
    expect(native.prepareOnchainTransaction).not.toHaveBeenCalled();
  });

  it('rejects responses that lose integer precision', async () => {
    const native = makeNativeNode({
      payOnchain: vi.fn(async () => ({
        address: 'test',
        amountSats: 9007199254740992n,
        state: 'pending',
      })),
    });
    const node = new LexeLniNode({ clientCredentials: 'test' }, native);
    const transaction = await node.prepareOnchainTransaction({
      address: 'test',
      amountSats: 1,
    });
    await expect(
      node.payOnchain(transaction, { dangerouslyDisableFeeGuardrail: true })
    ).rejects.toBeInstanceOf(LniError);
  });
});

it('rejects explicit fee limits before native execution, including with the old override', async () => {
  const native = makeNativeNode();
  const node = new LexeLniNode({ clientCredentials: 'test' }, native);
  const transaction = await node.prepareOnchainTransaction({
    address: 'test',
    amountSats: 10000,
  });
  for (const feeGuardrail of [{ maxFeeSats: 500 }, { maxFeePercent: 5 }, {}]) {
    for (const dangerouslyDisableFeeGuardrail of [false, true]) {
      await expect(
        node.payOnchain(transaction, {
          feeGuardrail,
          dangerouslyDisableFeeGuardrail,
        })
      ).rejects.toMatchObject({
        message: expect.stringContaining(
          'does not support a maximum network fee'
        ),
      });
    }
  }
  expect(native.payOnchain).not.toHaveBeenCalled();
});
