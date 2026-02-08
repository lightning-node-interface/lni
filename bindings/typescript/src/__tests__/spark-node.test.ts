import { beforeEach, describe, expect, it, vi } from 'vitest';

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
