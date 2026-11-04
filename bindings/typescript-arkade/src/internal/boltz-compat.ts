import {
  assertValidServerUnrollScript as assertCurrentServerUnrollScript,
  isVtxoSpent,
} from '@arkade-os/sdk';

export * from '@arkade-os/sdk';

// The deprecated Boltz package imports SDK names removed in 0.5. Keep the
// compatibility boundary here, using the current SDK's canonical checks.
export const hasTerminalSpend = isVtxoSpent;

export function isRecoverable(vtxo: Parameters<typeof isVtxoSpent>[0]): boolean {
  return Boolean(vtxo.isSwept) && !isVtxoSpent(vtxo);
}

type CheckpointPolicy = {
  network: Parameters<typeof assertCurrentServerUnrollScript>[1];
  advertisedForfeitPubkey?: Uint8Array;
};

export function resolveCheckpointExitDelayPolicy(
  network: CheckpointPolicy['network'],
  overrides: Pick<CheckpointPolicy, 'advertisedForfeitPubkey'> = {}
): CheckpointPolicy {
  return { network, ...overrides };
}

export function assertValidServerUnrollScript(
  script: Parameters<typeof assertCurrentServerUnrollScript>[0],
  policy: CheckpointPolicy
) {
  return assertCurrentServerUnrollScript(script, policy.network, policy.advertisedForfeitPubkey);
}
