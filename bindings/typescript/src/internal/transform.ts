import { decodeBase64, bytesToHex } from './encoding.js';
import type { NodeInfo, Transaction } from '../types.js';

export function emptyNodeInfo(overrides: Partial<NodeInfo> = {}): NodeInfo {
  return {
    alias: '',
    color: '',
    pubkey: '',
    network: '',
    blockHeight: 0,
    blockHash: '',
    sendBalanceMsat: 0,
    receiveBalanceMsat: 0,
    feeCreditBalanceMsat: 0,
    unsettledSendBalanceMsat: 0,
    unsettledReceiveBalanceMsat: 0,
    pendingOpenSendBalance: 0,
    pendingOpenReceiveBalance: 0,
    ...overrides,
  };
}

export function emptyTransaction(overrides: Partial<Transaction> = {}): Transaction {
  return {
    type: '',
    invoice: '',
    description: '',
    descriptionHash: '',
    preimage: '',
    paymentHash: '',
    amountMsats: 0,
    feesPaid: 0,
    createdAt: 0,
    expiresAt: 0,
    settledAt: 0,
    ...overrides,
  };
}

export function parseOptionalNumber(value: unknown): number {
  if (typeof value === 'number') {
    return Number.isFinite(value) ? value : 0;
  }

  if (typeof value === 'string') {
    const parsed = Number(value);
    return Number.isFinite(parsed) ? parsed : 0;
  }

  return 0;
}

export function toUnixSeconds(value: unknown): number {
  const parsed = parseOptionalNumber(value);
  if (!parsed) {
    return 0;
  }

  if (parsed > 10_000_000_000) {
    return Math.floor(parsed / 1000);
  }

  return Math.floor(parsed);
}

export function rHashToHex(value: string): string {
  if (!value) {
    return '';
  }

  try {
    return bytesToHex(decodeBase64(value));
  } catch {
    return value;
  }
}

export function btcToMsats(amount: string | number): number {
  const num = typeof amount === 'string' ? Number(amount) : amount;
  if (!Number.isFinite(num)) {
    return 0;
  }
  return Math.round(num * 100_000_000_000);
}

export function satsToMsats(amount: string | number): number {
  const num = typeof amount === 'string' ? Number(amount) : amount;
  if (!Number.isFinite(num)) {
    return 0;
  }
  return Math.round(num * 1000);
}

export function matchesSearch(tx: Transaction, search?: string): boolean {
  if (!search) {
    return true;
  }

  const normalized = search.toLowerCase();
  return (
    tx.paymentHash.toLowerCase().includes(normalized) ||
    tx.description.toLowerCase().includes(normalized) ||
    (tx.payerNote ?? '').toLowerCase().includes(normalized) ||
    tx.invoice.toLowerCase().includes(normalized)
  );
}
