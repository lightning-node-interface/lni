import { Address, OutScript, p2tr, Transaction, NETWORK, TEST_NETWORK } from '@scure/btc-signer';
import { secp256k1 } from '@noble/curves/secp256k1.js';
import { LniError } from '@sunnyln/lni';
import type { SparkFrostBase } from '@buildonspark/spark-sdk/bare';

type TransactionResult = { tx: Uint8Array; sighash: Uint8Array };
const REGTEST_NETWORK = { ...TEST_NETWORK, bech32: 'bcrt' };
const MAX_U64 = (1n << 64n) - 1n;

function invalid(reason: string): never {
  throw new LniError('InvalidInput', reason);
}

function validateSequence(sequence: number): void {
  if (!Number.isInteger(sequence) || sequence < 0 || sequence > 0xffffffff) {
    invalid('Spark transaction sequence must fit in u32.');
  }
  if ((sequence & 0x80000000) !== 0 || (sequence & 0x00400000) !== 0) {
    invalid('Spark transactions require enabled, block-based relative timelocks.');
  }
}

function validateAmount(amount: bigint): void {
  if (amount < 0n || amount > MAX_U64) {
    invalid('Spark transaction amount must fit in u64.');
  }
}

function parseTransaction(raw: Uint8Array): Transaction {
  return Transaction.fromRaw(raw, { allowUnknownInputs: true, allowUnknownOutputs: true });
}

function destinationScript(address: string): Uint8Array {
  for (const network of [NETWORK, TEST_NETWORK, REGTEST_NETWORK]) {
    try {
      return OutScript.encode(Address(network).decode(address));
    } catch {}
  }
  return invalid('Invalid Spark transaction destination address.');
}

// Match spark-frost's transaction.rs: version 3, locktime zero, BIP341
// SIGHASH_DEFAULT, and an optional zero-value P2A ephemeral anchor.
function buildTransaction(
  parent: Transaction,
  vout: number,
  script: Uint8Array,
  sequence: number,
  fee: bigint,
  includeAnchor: boolean
): TransactionResult {
  if (!Number.isInteger(vout) || vout < 0 || vout >= parent.outputsLength) {
    invalid('Invalid Spark transaction output index.');
  }
  const previous = parent.getOutput(vout);
  if (previous.amount === undefined || previous.script === undefined) {
    invalid('Spark transaction parent is missing its output amount or script.');
  }

  const tx = new Transaction({ version: 3, lockTime: 0, allowUnknownOutputs: true });
  tx.addInput({ txid: parent.id, index: vout, sequence });
  // Upstream keeps the full amount if it cannot cover the supplied fee.
  tx.addOutput({ script, amount: previous.amount > fee ? previous.amount - fee : previous.amount });
  if (includeAnchor) {
    tx.addOutput({ script: Uint8Array.of(0x51, 0x02, 0x4e, 0x73), amount: 0n });
  }
  return {
    tx: tx.toBytes(true),
    sighash: tx.preimageWitnessV1(0, [previous.script], 0, [previous.amount]),
  };
}

export function constructNodeTxPair(
  parentTx: Uint8Array,
  vout: number,
  address: string,
  sequence: number,
  directSequence: number,
  feeSats: bigint
): { cpfp: TransactionResult; direct: TransactionResult } {
  validateSequence(sequence);
  validateSequence(directSequence);
  validateAmount(feeSats);
  const parent = parseTransaction(parentTx);
  const script = destinationScript(address);
  return {
    cpfp: buildTransaction(parent, vout, script, sequence, 0n, true),
    direct: buildTransaction(parent, vout, script, directSequence, feeSats, false),
  };
}

export function constructRefundTxTrio(
  cpfpNodeTx: Uint8Array,
  directNodeTx: Uint8Array | null,
  vout: number,
  receivingPubkey: Uint8Array,
  network: string,
  sequence: number,
  directSequence: number,
  feeSats: bigint
): {
  cpfp_refund: TransactionResult;
  direct_refund?: TransactionResult;
  direct_from_cpfp_refund: TransactionResult;
} {
  validateSequence(sequence);
  validateSequence(directSequence);
  validateAmount(feeSats);
  if (!['mainnet', 'testnet', 'signet', 'regtest'].includes(network)) {
    invalid('Invalid Spark transaction network.');
  }
  // Upstream parses a full SEC1 public key, then applies the BIP341 key-only
  // TapTweak to its x-only form. Using that x-only key directly would pay the
  // refund to a different output key.
  const key = secp256k1.Point.fromBytes(receivingPubkey).toBytes(true).slice(1);
  const script = p2tr(key).script;
  const parent = parseTransaction(cpfpNodeTx);
  return {
    cpfp_refund: buildTransaction(parent, vout, script, sequence, 0n, true),
    ...(directNodeTx === null ? {} : {
      direct_refund: buildTransaction(
        parseTransaction(directNodeTx), vout, script, directSequence, feeSats, false
      ),
    }),
    direct_from_cpfp_refund: buildTransaction(parent, vout, script, directSequence, feeSats, false),
  };
}

export function computeMultiInputSighash(
  rawTx: Uint8Array,
  inputIndex: number,
  prevOutScripts: Uint8Array[],
  prevOutValues: number[]
): Uint8Array {
  const tx = parseTransaction(rawTx);
  if (prevOutScripts.length !== tx.inputsLength || prevOutValues.length !== tx.inputsLength) {
    invalid('Spark sighash requires a script and value for every input.');
  }
  if (!Number.isInteger(inputIndex) || inputIndex < 0 || inputIndex >= tx.inputsLength) {
    invalid('Invalid Spark sighash input index.');
  }
  const amounts = prevOutValues.map((value) => {
    if (!Number.isSafeInteger(value) || value < 0) {
      invalid('Spark previous output values must be non-negative safe integers.');
    }
    return BigInt(value);
  });
  return tx.preimageWitnessV1(inputIndex, prevOutScripts, 0, amounts);
}

export const pureSparkTransactions = {
  constructNodeTxPair,
  constructRefundTxTrio,
  computeMultiInputSighash,
} satisfies Pick<SparkFrostBase,
  'constructNodeTxPair' | 'constructRefundTxTrio' | 'computeMultiInputSighash'>;
