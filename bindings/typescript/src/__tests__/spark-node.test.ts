import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { StorageProvider } from '../types.js';

const TEST_MNEMONIC = 'abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about';

function mockBolt11Decoder(options: { amountMsats?: string } = {}): void {
  vi.doMock('light-bolt11-decoder', () => ({
    decode: vi.fn((invoice: string) => {
      if (invoice === 'with-amount' && options.amountMsats) {
        return {
          sections: [{ name: 'amount', value: options.amountMsats }],
          expiry: 3600,
        };
      }
      return {
        sections: [],
        expiry: 3600,
      };
    }),
  }));
}

describe('SparkNode payInvoice', () => {
  beforeEach(() => {
    vi.resetModules();
    vi.clearAllMocks();
  });

  it('rejects amountless invoice when amountMsats is missing', async () => {
    mockBolt11Decoder();
    const { SparkNode } = await import('../nodes/spark.js');
    const node = new SparkNode({
      mnemonic: TEST_MNEMONIC,
      sdkEntry: 'bare',
    });

    await expect(
      node.payInvoice({
        invoice: 'amountless',
      }),
    ).rejects.toThrow('Spark amountless invoice requires amountMsats.');
  });

  it('does not send amountSatsToSend for fixed-amount invoice', async () => {
    mockBolt11Decoder({ amountMsats: '118000' });
    const { SparkNode } = await import('../nodes/spark.js');
    const payLightningInvoice = vi.fn(async () => ({
      paymentPreimage: '',
      fee: { originalValue: 0, originalUnit: 'SATOSHI' },
    }));

    const node = new SparkNode({
      mnemonic: TEST_MNEMONIC,
      sdkEntry: 'bare',
    });
    (node as unknown as { getWallet: () => Promise<{ payLightningInvoice: typeof payLightningInvoice }> }).getWallet =
      async () => ({
        payLightningInvoice,
      });

    await node.payInvoice({
      invoice: 'with-amount',
    });

    expect(payLightningInvoice).toHaveBeenCalledTimes(1);
    expect(payLightningInvoice).toHaveBeenCalledWith(
      expect.objectContaining({
        amountSatsToSend: undefined,
      }),
    );
  });

  it('sends amountSatsToSend for amountless invoice when amountMsats is provided', async () => {
    mockBolt11Decoder();
    const { SparkNode } = await import('../nodes/spark.js');
    const payLightningInvoice = vi.fn(async () => ({
      paymentPreimage: '',
      fee: { originalValue: 0, originalUnit: 'SATOSHI' },
    }));

    const node = new SparkNode({
      mnemonic: TEST_MNEMONIC,
      sdkEntry: 'bare',
    });
    (node as unknown as { getWallet: () => Promise<{ payLightningInvoice: typeof payLightningInvoice }> }).getWallet =
      async () => ({
        payLightningInvoice,
      });

    await node.payInvoice({
      invoice: 'amountless',
      amountMsats: 118000,
    });

    expect(payLightningInvoice).toHaveBeenCalledTimes(1);
    expect(payLightningInvoice).toHaveBeenCalledWith(
      expect.objectContaining({
        amountSatsToSend: 118,
      }),
    );
  });
});

function makeTransfer(overrides: {
  id?: string;
  paymentHash?: string;
  status?: string;
  direction?: string;
  createdTime?: string;
} = {}) {
  return {
    id: overrides.id ?? 'transfer-1',
    status: overrides.status ?? 'COMPLETED',
    totalValue: 1000,
    createdTime: overrides.createdTime ?? new Date().toISOString(),
    transferDirection: overrides.direction ?? 'INCOMING',
    userRequest: {
      encodedInvoice: '',
      invoice: {
        paymentHash: overrides.paymentHash ?? 'abc123',
        memo: 'test',
      },
    },
  };
}

function createMockWallet(options: {
  transfers?: unknown[];
  getTransfer?: ReturnType<typeof vi.fn>;
  on?: ReturnType<typeof vi.fn>;
  off?: ReturnType<typeof vi.fn>;
} = {}) {
  const transfers = options.transfers ?? [];
  return {
    getBalance: vi.fn(async () => ({ balance: 1000 })),
    getIdentityPublicKey: vi.fn(async () => 'pubkey'),
    createLightningInvoice: vi.fn(async () => ({})),
    payLightningInvoice: vi.fn(async () => ({})),
    getTransfers: vi.fn(async () => ({ transfers, offset: transfers.length })),
    getTransfer: options.getTransfer,
    on: options.on,
    off: options.off,
  };
}

function overrideWallet(node: unknown, wallet: unknown) {
  (node as { getWallet: () => Promise<unknown> }).getWallet = async () => wallet;
}

describe('SparkNode lookupInvoice', () => {
  beforeEach(() => {
    vi.resetModules();
    vi.clearAllMocks();
  });

  it('uses cached transfer ID on second call', async () => {
    mockBolt11Decoder();
    const { SparkNode } = await import('../nodes/spark.js');
    const transfer = makeTransfer({ id: 'tf-99', paymentHash: 'hash1' });
    const getTransfer = vi.fn(async () => transfer);
    const wallet = createMockWallet({ transfers: [transfer], getTransfer });

    const node = new SparkNode({ mnemonic: TEST_MNEMONIC, sdkEntry: 'bare' });
    overrideWallet(node, wallet);

    // First call — scans and populates cache
    const tx1 = await node.lookupInvoice({ paymentHash: 'hash1' });
    expect(tx1.paymentHash).toBe('hash1');

    // Reset getTransfers call count
    wallet.getTransfers.mockClear();

    // Second call — should use cache → getTransfer, NOT getTransfers
    const tx2 = await node.lookupInvoice({ paymentHash: 'hash1' });
    expect(tx2.paymentHash).toBe('hash1');
    expect(getTransfer).toHaveBeenCalledWith('tf-99');
    expect(wallet.getTransfers).not.toHaveBeenCalled();
  });

  it('tries 1-hour window before full scan', async () => {
    mockBolt11Decoder();
    const { SparkNode } = await import('../nodes/spark.js');
    const transfer = makeTransfer({ paymentHash: 'hash2' });
    // Return empty on first call (1h window), then results on second (24h)
    let callCount = 0;
    const wallet = createMockWallet();
    wallet.getTransfers.mockImplementation(async () => {
      callCount++;
      if (callCount === 1) return { transfers: [], offset: 0 };
      return { transfers: [transfer], offset: 1 };
    });

    const node = new SparkNode({ mnemonic: TEST_MNEMONIC, sdkEntry: 'bare' });
    overrideWallet(node, wallet);

    const tx = await node.lookupInvoice({ paymentHash: 'hash2' });
    expect(tx.paymentHash).toBe('hash2');

    // Should have called getTransfers at least twice (1h window empty → 24h window found)
    expect(wallet.getTransfers.mock.calls.length).toBeGreaterThanOrEqual(2);
    // First call should have a createdAfter date argument
    const firstCall = wallet.getTransfers.mock.calls[0] as unknown[];
    expect(firstCall[2]).toBeInstanceOf(Date); // createdAfter
  });

  it('handles stale cache gracefully', async () => {
    mockBolt11Decoder();
    const { SparkNode } = await import('../nodes/spark.js');
    const getTransfer = vi.fn(async () => undefined); // stale — returns nothing
    const transfer = makeTransfer({ id: 'tf-new', paymentHash: 'hash3' });
    const wallet = createMockWallet({ transfers: [transfer], getTransfer });

    const storage: StorageProvider = {
      get: vi.fn(async () => 'tf-stale'), // cached a stale ID
      set: vi.fn(async () => {}),
      remove: vi.fn(async () => {}),
    };

    const node = new SparkNode({ mnemonic: TEST_MNEMONIC, sdkEntry: 'bare', storage });
    overrideWallet(node, wallet);

    const tx = await node.lookupInvoice({ paymentHash: 'hash3' });
    expect(tx.paymentHash).toBe('hash3');
    // getTransfer was called with stale ID, returned undefined
    expect(getTransfer).toHaveBeenCalledWith('tf-stale');
    // Should have removed stale entry
    expect(storage.remove).toHaveBeenCalledWith('lni:txcache:hash3');
    // Fell through to scan
    expect(wallet.getTransfers).toHaveBeenCalled();
  });
});

describe('SparkNode listTransactions', () => {
  beforeEach(() => {
    vi.resetModules();
    vi.clearAllMocks();
  });

  it('passes date filters through to getTransfers', async () => {
    mockBolt11Decoder();
    const { SparkNode } = await import('../nodes/spark.js');
    const transfer = makeTransfer();
    const wallet = createMockWallet({ transfers: [transfer] });

    const node = new SparkNode({ mnemonic: TEST_MNEMONIC, sdkEntry: 'bare' });
    overrideWallet(node, wallet);

    const afterTs = 1700000000;
    const beforeTs = 1700003600;
    await node.listTransactions({
      from: 0,
      limit: 10,
      createdAfter: afterTs,
      createdBefore: beforeTs,
    });

    expect(wallet.getTransfers).toHaveBeenCalled();
    const callArgs = wallet.getTransfers.mock.calls[0] as unknown[];
    const createdAfter = callArgs[2] as Date;
    const createdBefore = callArgs[3] as Date;
    expect(createdAfter).toBeInstanceOf(Date);
    expect(createdBefore).toBeInstanceOf(Date);
    expect(Math.floor(createdAfter.getTime() / 1000)).toBe(afterTs);
    expect(Math.floor(createdBefore.getTime() / 1000)).toBe(beforeTs);
  });
});

describe('SparkNode onInvoiceEvents', () => {
  beforeEach(() => {
    vi.resetModules();
    vi.clearAllMocks();
  });

  it('uses event listener when available', async () => {
    mockBolt11Decoder();
    const { SparkNode } = await import('../nodes/spark.js');
    const transfer = makeTransfer({ id: 'tf-event', paymentHash: 'hash-ev', status: 'PENDING' });
    const settledTransfer = makeTransfer({ id: 'tf-event', paymentHash: 'hash-ev', status: 'COMPLETED' });

    let capturedListener: ((...args: unknown[]) => void) | undefined;
    const on = vi.fn((event: string, listener: (...args: unknown[]) => void) => {
      if (event === 'transfer:claimed') capturedListener = listener;
    });
    const off = vi.fn();
    const getTransfer = vi.fn(async () => settledTransfer);

    // getTransfers returns unsettled transfer for initial lookupInvoice check
    const unsettledTransfer = { ...transfer, status: 'PENDING' };
    const wallet = createMockWallet({ transfers: [unsettledTransfer], getTransfer, on, off });

    const node = new SparkNode({ mnemonic: TEST_MNEMONIC, sdkEntry: 'bare' });
    overrideWallet(node, wallet);

    const statuses: string[] = [];
    const promise = node.onInvoiceEvents(
      { paymentHash: 'hash-ev', pollingDelaySec: 1, maxPollingSec: 5 },
      (status) => { statuses.push(status); },
    );

    // Wait for initial lookupInvoice + listener registration
    await new Promise((r) => setTimeout(r, 100));

    // Verify listener was registered
    expect(on).toHaveBeenCalledWith('transfer:claimed', expect.any(Function));
    expect(capturedListener).toBeDefined();

    // Simulate event
    capturedListener!({ transferId: 'tf-event' });

    await promise;

    expect(statuses).toContain('success');
    expect(off).toHaveBeenCalledWith('transfer:claimed', expect.any(Function));
  });

  it('falls back to polling when on is not available', async () => {
    mockBolt11Decoder();
    const { SparkNode } = await import('../nodes/spark.js');
    const transfer = makeTransfer({ paymentHash: 'hash-poll', status: 'COMPLETED' });
    const wallet = createMockWallet({ transfers: [transfer] });
    // No on/off methods

    const node = new SparkNode({ mnemonic: TEST_MNEMONIC, sdkEntry: 'bare' });
    overrideWallet(node, wallet);

    const statuses: string[] = [];
    await node.onInvoiceEvents(
      { paymentHash: 'hash-poll', pollingDelaySec: 1, maxPollingSec: 5 },
      (status) => { statuses.push(status); },
    );

    expect(statuses).toContain('success');
    // getTransfers should have been called (polling path)
    expect(wallet.getTransfers).toHaveBeenCalled();
  });
});
