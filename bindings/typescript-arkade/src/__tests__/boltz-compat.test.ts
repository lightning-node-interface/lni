import { describe, expect, it } from 'vitest';
import { CSVMultisigTapscript, networks, type VirtualCoin } from '@arkade-os/sdk';
import {
  assertValidServerUnrollScript,
  hasTerminalSpend,
  isRecoverable,
  resolveCheckpointExitDelayPolicy,
} from '../internal/boltz-compat.js';

function coin(overrides: Partial<VirtualCoin> = {}): VirtualCoin {
  return {
    txid: '00'.repeat(32), vout: 0, value: 1000,
    status: { confirmed: false }, createdAt: new Date(0),
    script: '51', isUnrolled: false,
    ...overrides,
  };
}

const publicKey = Uint8Array.from(Buffer.from(
  '79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798', 'hex'
));

function checkpoint(type: 'seconds' | 'blocks', value: bigint): string {
  return Buffer.from(CSVMultisigTapscript.encode({
    pubkeys: [publicKey], timelock: { type, value },
  }).script).toString('hex');
}

describe('Boltz compatibility with the current Arkade SDK', () => {
  it.each([
    { isSpent: true }, { spentBy: 'checkpoint' }, { settledBy: 'settlement' },
  ])('does not classify a swept, consumed output as recoverable: %o', (spent) => {
    const output = coin({ isSwept: true, ...spent });
    expect(hasTerminalSpend(output)).toBe(true);
    expect(isRecoverable(output)).toBe(false);
  });

  it('requires a swept output before attempting recovery', () => {
    expect(isRecoverable(coin({ isSwept: true }))).toBe(true);
    expect(isRecoverable(coin())).toBe(false);
  });

  it('preserves the current mainnet checkpoint validation', () => {
    const policy = resolveCheckpointExitDelayPolicy(networks.bitcoin, {
      advertisedForfeitPubkey: publicKey,
    });
    expect(assertValidServerUnrollScript(checkpoint('seconds', 1024n), policy).params.timelock)
      .toEqual({ type: 'seconds', value: 1024n });
    expect(() => assertValidServerUnrollScript(checkpoint('blocks', 10n), policy)).toThrow();
  });

  it('rejects a checkpoint that commits to a different forfeit key', () => {
    const policy = resolveCheckpointExitDelayPolicy(networks.bitcoin, {
      advertisedForfeitPubkey: new Uint8Array(32).fill(1),
    });
    expect(() => assertValidServerUnrollScript(checkpoint('seconds', 1024n), policy))
      .toThrow(/forfeitPubkey/);
  });
});
