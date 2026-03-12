import { describe, expect } from 'vitest';
import { ArkadeBoltzNode } from '../../nodes/arkade-boltz.js';
import { hasEnv, itIf, nonEmpty, testInvoiceLabel, timeout } from './helpers.js';

describe('Real integration from crates/lni/.env > ArkadeBoltzNode', () => {
  const enabled = hasEnv('ARKADE_BOLTZ_MNEMONIC', 'ARKADE_BOLTZ_ARK_SERVER_URL');

  const makeNode = () =>
    new ArkadeBoltzNode({
      mnemonic: process.env.ARKADE_BOLTZ_MNEMONIC!,
      passphrase: nonEmpty(process.env.ARKADE_BOLTZ_PASSPHRASE),
      network: nonEmpty(process.env.ARKADE_BOLTZ_NETWORK) as
        | 'mainnet'
        | 'bitcoin'
        | 'testnet'
        | 'signet'
        | 'mutinynet'
        | 'regtest'
        | undefined,
      arkServerUrl: process.env.ARKADE_BOLTZ_ARK_SERVER_URL!,
      indexerUrl: nonEmpty(process.env.ARKADE_BOLTZ_INDEXER_URL),
      esploraUrl: nonEmpty(process.env.ARKADE_BOLTZ_ESPLORA_URL),
      swapApiUrl: nonEmpty(process.env.ARKADE_BOLTZ_SWAP_API_URL),
      referralId: nonEmpty(process.env.ARKADE_BOLTZ_REFERRAL_ID),
    });

  itIf(enabled)('getInfo + createInvoice + listTransactions', async () => {
    const node = makeNode();
    const info = await node.getInfo();
    expect(typeof info.alias).toBe('string');

    const invoice = await node.createInvoice({
      amountMsats: 5_000,
      description: testInvoiceLabel('arkade-boltz'),
    });
    expect(invoice.invoice.length).toBeGreaterThan(0);

    const txs = await node.listTransactions({ from: 0, limit: 25 });
    expect(Array.isArray(txs)).toBe(true);
  }, timeout);
});
