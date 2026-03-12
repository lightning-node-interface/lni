import { beforeEach, describe, expect, it, vi } from 'vitest';

describe('ArkadeBoltzNode', () => {
  beforeEach(() => {
    vi.resetModules();
    vi.clearAllMocks();
  });

  it('initializes wallet and swaps with in-memory defaults', async () => {
    const createSpy = vi.fn(async () => ({
      getBalance: async () => ({ available: 321 }),
      networkName: 'mutinynet',
    }));
    const createSwapsSpy = vi.fn(async () => ({
      getSwapHistory: async () => [],
    }));
    const swapProviderSpy = vi.fn(function SwapProvider(this: Record<string, unknown>, config: unknown) {
      this.config = config;
    });

    class InMemoryWalletRepository {
      readonly kind = 'wallet';
    }
    class InMemoryContractRepository {
      readonly kind = 'contract';
    }

    vi.doMock('@arkade-os/sdk', () => ({
      MnemonicIdentity: {
        fromMnemonic: vi.fn(() => ({ kind: 'identity' })),
      },
      Wallet: {
        create: createSpy,
      },
      InMemoryWalletRepository,
      InMemoryContractRepository,
    }));
    vi.doMock('@arkade-os/boltz-swap', () => ({
      ArkadeSwaps: {
        create: createSwapsSpy,
      },
      BoltzSwapProvider: swapProviderSpy,
      decodeInvoice: vi.fn(),
      getInvoicePaymentHash: vi.fn(),
    }));

    const { ArkadeBoltzNode } = await import('../nodes/arkade-boltz.js');
    const node = new ArkadeBoltzNode({
      mnemonic: 'abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about',
      arkServerUrl: 'https://mutinynet.arkade.sh',
      network: 'mutinynet',
    });

    const info = await node.getInfo();
    expect(info.network).toBe('mutinynet');
    expect(info.sendBalanceMsat).toBe(321_000);

    expect(createSpy).toHaveBeenCalledTimes(1);
    const [walletConfig] = createSpy.mock.calls[0] as unknown as [Record<string, unknown>];
    expect(walletConfig.arkServerUrl).toBe('https://mutinynet.arkade.sh');
    const storage = walletConfig.storage as Record<string, { kind: string }>;
    expect(storage.walletRepository!.kind).toBe('wallet');
    expect(storage.contractRepository!.kind).toBe('contract');

    expect(swapProviderSpy).toHaveBeenCalledWith({
      network: 'mutinynet',
      apiUrl: undefined,
      referralId: undefined,
    });

    const [swapsConfig] = createSwapsSpy.mock.calls[0] as unknown as [Record<string, unknown>];
    expect(swapsConfig.swapRepository).toMatchObject({ version: 1 });
  });

  it('maps createInvoice into an incoming transaction', async () => {
    const { ArkadeBoltzNode } = await import('../nodes/arkade-boltz.js');
    const node = new ArkadeBoltzNode({
      mnemonic: 'abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about',
      arkServerUrl: 'https://arkade.example',
    });

    (node as unknown as {
      getContext: () => Promise<unknown>;
    }).getContext = async () => ({
      wallet: { getBalance: async () => ({ available: 0 }) },
      swaps: {
        createLightningInvoice: async () => ({
          amount: 97,
          expiry: 1234,
          invoice: 'lnbc1incoming',
          paymentHash: 'hash-in',
          preimage: 'preimage-in',
          pendingSwap: { createdAt: 111, id: 'swap-in' },
        }),
      },
      decodeInvoice: vi.fn(),
      getInvoicePaymentHash: vi.fn(),
    });

    const tx = await node.createInvoice({
      amountMsats: 100_000,
      description: 'incoming memo',
    });

    console.log('Arkade Boltz Invoice', tx);

    expect(tx).toMatchObject({
      type: 'incoming',
      invoice: 'lnbc1incoming',
      paymentHash: 'hash-in',
      preimage: 'preimage-in',
      amountMsats: 100_000,
      feesPaid: 3_000,
      createdAt: 111,
      expiresAt: 1234,
      externalId: 'swap-in',
    });
  });

  it('rejects amountless invoices for payInvoice', async () => {
    const { ArkadeBoltzNode } = await import('../nodes/arkade-boltz.js');
    const node = new ArkadeBoltzNode({
      mnemonic: 'abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about',
      arkServerUrl: 'https://arkade.example',
    });

    (node as unknown as {
      getContext: () => Promise<unknown>;
    }).getContext = async () => ({
      wallet: { getBalance: async () => ({ available: 0 }) },
      swaps: {
        sendLightningPayment: vi.fn(),
      },
      decodeInvoice: vi.fn(() => ({
        amountSats: 0,
        expiry: 3600,
        description: '',
        paymentHash: 'hash-out',
      })),
      getInvoicePaymentHash: vi.fn(() => 'hash-out'),
    });

    await expect(node.payInvoice({ invoice: 'amountless' })).rejects.toThrow(
      'ArkadeBoltzNode does not support amountless invoices.',
    );
  });

  it('maps swap history for lookupInvoice and listTransactions', async () => {
    const { ArkadeBoltzNode } = await import('../nodes/arkade-boltz.js');
    const node = new ArkadeBoltzNode({
      mnemonic: 'abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about',
      arkServerUrl: 'https://arkade.example',
    });

    (node as unknown as {
      getContext: () => Promise<unknown>;
    }).getContext = async () => ({
      wallet: { getBalance: async () => ({ available: 0 }) },
      swaps: {
        getSwapHistory: async () => [
          {
            id: 'reverse-1',
            type: 'reverse',
            createdAt: 200,
            preimage: 'preimage-in',
            status: 'invoice.settled',
            request: {
              invoiceAmount: 10,
              preimageHash: 'hash-in',
              description: 'incoming memo',
            },
            response: {
              invoice: 'lnbc1incoming',
              onchainAmount: 9,
            },
          },
          {
            id: 'sub-1',
            type: 'submarine',
            createdAt: 300,
            preimage: 'preimage-out',
            preimageHash: 'hash-out',
            status: 'transaction.claimed',
            request: {
              invoice: 'lnbc1outgoing',
            },
            response: {
              expectedAmount: 12,
            },
          },
        ],
      },
      decodeInvoice: vi.fn(() => ({
        amountSats: 10,
        expiry: 4444,
        description: 'outgoing memo',
        paymentHash: 'hash-out',
      })),
      getInvoicePaymentHash: vi.fn(),
    });

    const incoming = await node.lookupInvoice({ paymentHash: 'hash-in' });
    expect(incoming).toMatchObject({
      type: 'incoming',
      paymentHash: 'hash-in',
      amountMsats: 10_000,
      feesPaid: 1_000,
      settledAt: 200,
    });

    const outgoing = await node.listTransactions({
      from: 0,
      limit: 10,
      search: 'outgoing memo',
    });
    expect(outgoing).toHaveLength(1);
    expect(outgoing[0]).toMatchObject({
      type: 'outgoing',
      paymentHash: 'hash-out',
      amountMsats: 10_000,
      feesPaid: 2_000,
      settledAt: 300,
    });
  });

  it('wraps initialization failures as LniError', async () => {
    vi.doMock('@arkade-os/sdk', () => ({
      MnemonicIdentity: {
        fromMnemonic: vi.fn(() => ({ kind: 'identity' })),
      },
      Wallet: {
        create: vi.fn(async () => {
          throw new Error('wallet init failed');
        }),
      },
      InMemoryWalletRepository: class {},
      InMemoryContractRepository: class {},
    }));
    vi.doMock('@arkade-os/boltz-swap', () => ({
      ArkadeSwaps: {
        create: vi.fn(),
      },
      BoltzSwapProvider: class {},
      decodeInvoice: vi.fn(),
      getInvoicePaymentHash: vi.fn(),
    }));

    const { ArkadeBoltzNode } = await import('../nodes/arkade-boltz.js');
    const node = new ArkadeBoltzNode({
      mnemonic: 'abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about',
      arkServerUrl: 'https://arkade.example',
    });

    await expect(node.getInfo()).rejects.toMatchObject({
      name: 'LniError',
      code: 'Api',
      message: 'wallet init failed',
    });
  });
});
