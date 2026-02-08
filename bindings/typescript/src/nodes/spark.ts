import { Transaction as BtcTransaction } from '@scure/btc-signer';
import { mnemonicToSeedSync } from '@scure/bip39';
import { sha256 } from '@noble/hashes/sha2';
import { schnorr, secp256k1 } from '@noble/curves/secp256k1';
import {
  Identifier as FrostIdentifier,
  KeyPackage as FrostKeyPackage,
  Nonce as FrostNonce,
  NonceCommitment as FrostNonceCommitment,
  PublicKeyPackage as FrostPublicKeyPackage,
  Signature as FrostSignature,
  Secp256K1Sha256TR,
  SignatureShare as FrostSignatureShare,
  SigningCommitments as FrostSigningCommitments,
  SigningNonces as FrostSigningNonces,
  SigningPackageImpl as FrostSigningPackageImpl,
  SigningShare as FrostSigningShare,
  VerifyingKey as FrostVerifyingKey,
  VerifyingShare as FrostVerifyingShare,
  hasEvenYPublicKey,
  intoEvenYKeyPackage,
  tweakKeyPackage,
  tweakPublicKeyPackage,
} from '../vendor/frosts-bridge.js';
import { decrypt as eciesDecrypt, encrypt as eciesEncrypt } from 'eciesjs';
import { decode as decodeBolt11 } from 'light-bolt11-decoder';
import { LniError } from '../errors.js';
import { bytesToHex, hexToBytes } from '../internal/encoding.js';
import { pollInvoiceEvents } from '../internal/polling.js';
import { emptyNodeInfo, emptyTransaction, matchesSearch, toUnixSeconds } from '../internal/transform.js';
import { InvoiceType, type CreateInvoiceParams, type CreateOfferParams, type InvoiceEventCallback, type LightningNode, type ListTransactionsParams, type LookupInvoiceParams, type NodeInfo, type NodeRequestOptions, type Offer, type OnInvoiceEventParams, type PayInvoiceParams, type PayInvoiceResponse, type SparkConfig, type Transaction } from '../types.js';

type SparkSdkEntry = 'auto' | 'bare' | 'native' | 'default';

type SparkSdkModuleLike = {
  SparkWallet: {
    initialize(args: {
      mnemonicOrSeed?: string | Uint8Array;
      accountNumber?: number;
      options?: Record<string, unknown>;
      signer?: unknown;
    }): Promise<{ wallet: SparkWalletLike; mnemonic?: string }>;
  };
  getSparkFrost?: () => SparkFrostLike;
  setSparkFrostOnce?: (sparkFrost: SparkFrostLike) => void;
  SparkFrostBase?: new () => SparkFrostLike;
};

type SparkWalletLike = {
  getBalance(): Promise<{ balance: number | bigint }>;
  getIdentityPublicKey(): Promise<string>;
  getLeaves?(isBalanceCheck?: boolean): Promise<Array<{ value?: unknown }>>;
  createLightningInvoice(args: {
    amountSats: number;
    memo?: string;
    expirySeconds?: number;
    descriptionHash?: string;
  }): Promise<unknown>;
  payLightningInvoice(args: {
    invoice: string;
    maxFeeSats: number;
    preferSpark?: boolean;
    amountSatsToSend?: number;
    idempotencyKey?: string;
  }): Promise<unknown>;
  getTransfers(limit?: number, offset?: number): Promise<{ transfers: unknown[]; offset: number }>;
  getLightningSendRequest?(id: string): Promise<{
    status?: string;
    paymentPreimage?: string;
    fee?: unknown;
  } | null>;
  cleanupConnections?: () => Promise<void>;
};

type SparkSigningCommitmentLike = {
  hiding: Uint8Array;
  binding: Uint8Array;
};

type SignFrostBindingParamsLike = {
  message: Uint8Array;
  keyPackage: {
    secretKey: Uint8Array;
    publicKey: Uint8Array;
    verifyingKey: Uint8Array;
  };
  nonce: {
    hiding: Uint8Array;
    binding: Uint8Array;
  };
  selfCommitment: SparkSigningCommitmentLike;
  statechainCommitments?: Record<string, SparkSigningCommitmentLike>;
  adaptorPubKey?: Uint8Array;
};

type AggregateFrostBindingParamsLike = {
  message: Uint8Array;
  statechainSignatures?: Record<string, Uint8Array>;
  statechainPublicKeys?: Record<string, Uint8Array>;
  verifyingKey: Uint8Array;
  statechainCommitments?: Record<string, SparkSigningCommitmentLike>;
  selfCommitment: SparkSigningCommitmentLike;
  selfPublicKey: Uint8Array;
  selfSignature: Uint8Array;
  adaptorPubKey?: Uint8Array;
};

type SparkFrostLike = {
  signFrost(params: SignFrostBindingParamsLike): Promise<Uint8Array>;
  aggregateFrost(params: AggregateFrostBindingParamsLike): Promise<Uint8Array>;
  createDummyTx(address: string, amountSats: bigint): Promise<{ tx: Uint8Array; txid: string }>;
  encryptEcies(msg: Uint8Array, publicKey: Uint8Array): Promise<Uint8Array>;
  decryptEcies(encryptedMsg: Uint8Array, privateKey: Uint8Array): Promise<Uint8Array>;
};

const DEFAULT_MAX_FEE_SATS = 20;
const DEFAULT_PAGE_SIZE = 50;
const DEFAULT_SCAN_LIMIT = 1000;
const SPARK_SDK_DEFAULT_ENTRY = '@buildonspark/spark-sdk';
const SPARK_SDK_PACKAGED_BARE_ENTRY = '../vendor/spark-sdk-bare.js';
const SPARK_SDK_BARE_ENTRY = '@buildonspark/spark-sdk/bare';
const SPARK_SDK_NATIVE_ENTRY = '@buildonspark/spark-sdk/native';

function createUserIdentifier(): any {
  return FrostIdentifier.derive(Secp256K1Sha256TR, new TextEncoder().encode('user'));
}
function mapNetworkToSpark(network?: SparkConfig['network']): 'MAINNET' | 'REGTEST' | 'TESTNET' | 'SIGNET' | 'LOCAL' {
  switch ((network ?? 'mainnet').toLowerCase()) {
    case 'mainnet':
      return 'MAINNET';
    case 'regtest':
      return 'REGTEST';
    case 'testnet':
      return 'TESTNET';
    case 'signet':
      return 'SIGNET';
    case 'local':
      return 'LOCAL';
    default:
      return 'MAINNET';
  }
}

function numberFromUnknown(value: unknown): number {
  if (typeof value === 'number') {
    return Number.isFinite(value) ? value : 0;
  }
  if (typeof value === 'bigint') {
    if (value > BigInt(Number.MAX_SAFE_INTEGER) || value < BigInt(Number.MIN_SAFE_INTEGER)) {
      throw new Error(`BigInt value ${value} exceeds Number.MAX_SAFE_INTEGER and cannot be safely converted.`);
    }
    return Number(value);
  }
  if (typeof value === 'string') {
    const parsed = Number(value);
    return Number.isFinite(parsed) ? parsed : 0;
  }
  return 0;
}

function toUnixSecondsFromAny(value: unknown): number {
  if (value instanceof Date) {
    return Math.floor(value.getTime() / 1000);
  }
  if (typeof value === 'string') {
    const parsedDate = Date.parse(value);
    if (Number.isFinite(parsedDate)) {
      return Math.floor(parsedDate / 1000);
    }
  }
  return toUnixSeconds(value);
}

function mapCurrencyAmountToMsats(value: unknown): number {
  if (typeof value === 'number' || typeof value === 'bigint' || typeof value === 'string') {
    return Math.floor(numberFromUnknown(value) * 1000);
  }

  if (!value || typeof value !== 'object') {
    return 0;
  }

  const maybe = value as { originalValue?: unknown; originalUnit?: unknown };
  const amount = numberFromUnknown(maybe.originalValue);
  const unit = typeof maybe.originalUnit === 'string' ? maybe.originalUnit.toLowerCase() : '';

  if (!amount) {
    return 0;
  }

  if (unit.includes('millisatoshi') || unit.includes('msat')) {
    return Math.floor(amount);
  }
  if (unit.includes('sat') && !unit.includes('msat')) {
    return Math.floor(amount * 1000);
  }
  if (unit.includes('btc') || unit.includes('bitcoin')) {
    return Math.floor(amount * 100_000_000_000);
  }

  return Math.floor(amount);
}

function removeUndefinedValues(value: Record<string, unknown>): Record<string, unknown> {
  return Object.fromEntries(Object.entries(value).filter(([, entry]) => entry !== undefined));
}

async function resolveWalletBalanceSats(
  wallet: SparkWalletLike,
  rawBalance: { balance: number | bigint } | undefined,
): Promise<number> {
  const directBalance = numberFromUnknown(rawBalance?.balance);
  if (directBalance > 0) {
    return directBalance;
  }

  if (typeof wallet.getLeaves !== 'function') {
    return Math.max(0, directBalance);
  }

  try {
    const leaves = await wallet.getLeaves(true);
    if (!Array.isArray(leaves)) {
      return Math.max(0, directBalance);
    }

    const leafBalance = leaves.reduce((acc, leaf) => {
      return acc + numberFromUnknown(leaf?.value);
    }, 0);

    if (leafBalance > 0) {
      return leafBalance;
    }
  } catch {
    return Math.max(0, directBalance);
  }

  return Math.max(0, directBalance);
}

function signingCommitmentFromBinding(commitment: SparkSigningCommitmentLike): any {
  const hiding = FrostNonceCommitment.deserialize(Secp256K1Sha256TR, commitment.hiding);
  const binding = FrostNonceCommitment.deserialize(Secp256K1Sha256TR, commitment.binding);
  return new FrostSigningCommitments(Secp256K1Sha256TR, hiding, binding);
}

function identifierFromHex(identifier: string): any {
  return FrostIdentifier.deserialize(Secp256K1Sha256TR, hexToBytes(identifier));
}

function identifierToHex(identifier: any): string {
  return bytesToHex(identifier.serialize());
}

function buildUserKeyPackage(
  params: SignFrostBindingParamsLike['keyPackage'],
  identifierOverride?: any,
): any {
  const userIdentifier = identifierOverride ?? createUserIdentifier();
  const signingShare = FrostSigningShare.deserialize(Secp256K1Sha256TR, params.secretKey);
  const verifyingShare = FrostVerifyingShare.deserialize(Secp256K1Sha256TR, params.publicKey);
  const verifyingKey = FrostVerifyingKey.deserialize(Secp256K1Sha256TR, params.verifyingKey).toElement();
  const base = new FrostKeyPackage(
    Secp256K1Sha256TR,
    userIdentifier,
    signingShare,
    verifyingShare,
    verifyingKey,
    1,
  );
  const tweaked = tweakKeyPackage(base as any, new Uint8Array()) as any;
  const evenY = intoEvenYKeyPackage(base as any, hasEvenYPublicKey(params.verifyingKey)) as any;
  return new FrostKeyPackage(
    Secp256K1Sha256TR,
    evenY.identifier,
    evenY.signingShare,
    evenY.verifyingShare,
    tweaked.verifyingKey,
    evenY.minSigners,
  );
}

function buildSigningPackage(
  message: Uint8Array,
  selfCommitment: SparkSigningCommitmentLike,
  selfIdentifier: any,
  statechainCommitments?: Record<string, SparkSigningCommitmentLike>,
): any {
  const commitments = new Map<any, any>();
  const userIdHex = identifierToHex(selfIdentifier);
  const commitmentById = new Map<string, SparkSigningCommitmentLike>();

  for (const [identifier, commitment] of Object.entries(statechainCommitments ?? {})) {
    commitmentById.set(identifier, commitment);
  }
  commitmentById.set(userIdHex, selfCommitment);

  const sortedIds = Array.from(commitmentById.keys()).sort();
  for (const identifier of sortedIds) {
    const commitment = commitmentById.get(identifier);
    if (!commitment) {
      continue;
    }

    commitments.set(
      identifier === userIdHex ? selfIdentifier : identifierFromHex(identifier),
      signingCommitmentFromBinding(commitment),
    );
  }

  return new FrostSigningPackageImpl(Secp256K1Sha256TR, commitments, message);
}

function normalizeAdaptorPublicKey(adaptorPubKey?: Uint8Array): Uint8Array | undefined {
  if (!adaptorPubKey || adaptorPubKey.length === 0) {
    return undefined;
  }

  if (adaptorPubKey.length === 33) {
    const prefix = adaptorPubKey[0];
    if (prefix !== 0x02 && prefix !== 0x03) {
      throw new LniError(
        'InvalidInput',
        'Spark adaptor public key (33-byte form) must use compressed secp256k1 prefix 0x02/0x03.',
      );
    }
    return adaptorPubKey;
  }

  if (adaptorPubKey.length === 32) {
    const compressed = new Uint8Array(33);
    compressed[0] = 0x02;
    compressed.set(adaptorPubKey, 1);
    return compressed;
  }

  throw new LniError('InvalidInput', 'Spark adaptor public key must be 32 or 33 bytes.');
}

function scalarFromLike(value: unknown, label: string): bigint {
  if (typeof value === 'bigint') {
    return value;
  }

  if (
    typeof value === 'object' &&
    value !== null &&
    'toScalar' in value &&
    typeof (value as { toScalar?: unknown }).toScalar === 'function'
  ) {
    return ((value as { toScalar: () => bigint }).toScalar());
  }

  throw new LniError('Api', `Spark signer expected scalar-like value for ${label}.`);
}

function elementBytesFromLike(value: unknown, label: string): Uint8Array {
  if (value instanceof Uint8Array) {
    return value;
  }

  if (
    typeof value === 'object' &&
    value !== null &&
    'serialize' in value &&
    typeof (value as { serialize?: unknown }).serialize === 'function'
  ) {
    return (value as { serialize: () => Uint8Array }).serialize();
  }

  throw new LniError('Api', `Spark signer expected element-like value for ${label}.`);
}

function bytesToBigInt(bytes: Uint8Array): bigint {
  let value = 0n;
  for (const byte of bytes) {
    value = (value << 8n) | BigInt(byte);
  }
  return value;
}

function bigIntToFixedBytes(value: bigint, size: number): Uint8Array {
  const bytes = new Uint8Array(size);
  let remainder = value;
  for (let index = size - 1; index >= 0; index -= 1) {
    bytes[index] = Number(remainder & 0xffn);
    remainder >>= 8n;
  }
  return bytes;
}

function toXOnlyPublicKey(pubkey: Uint8Array): Uint8Array {
  if (pubkey.length === 32) {
    return pubkey;
  }

  if (pubkey.length === 33 && (pubkey[0] === 0x02 || pubkey[0] === 0x03)) {
    return pubkey.slice(1);
  }

  throw new LniError('InvalidInput', `Spark public key must be 32-byte x-only or 33-byte compressed, got ${pubkey.length} bytes.`);
}

function validateOutboundAdaptorSignatureLocal(params: {
  signature: Uint8Array;
  message: Uint8Array;
  pubkey: Uint8Array;
  adaptorPubkey: Uint8Array;
}): { ok: true } | { ok: false; reason: string } {
  try {
    const { signature, message, pubkey, adaptorPubkey } = params;
    if (message.length !== 32) {
      return { ok: false, reason: `invalid message length: ${message.length}` };
    }
    if (signature.length !== 64) {
      return { ok: false, reason: `invalid signature length: ${signature.length}` };
    }

    const r = signature.slice(0, 32);
    const s = signature.slice(32, 64);
    const rNum = bytesToBigInt(r);
    const sNum = bytesToBigInt(s);
    if (rNum >= secp256k1.CURVE.Fp.ORDER) {
      return { ok: false, reason: 'invalid signature r >= field order' };
    }
    if (sNum >= secp256k1.CURVE.n) {
      return { ok: false, reason: 'invalid signature s >= curve order' };
    }

    const xOnlyPubkey = toXOnlyPublicKey(pubkey);
    const signerPoint = schnorr.utils.lift_x(bytesToBigInt(xOnlyPubkey));
    signerPoint.assertValidity();

    const challengeBytes = schnorr.utils.taggedHash('BIP0340/challenge', r, signerPoint.toBytes().slice(1), message);
    const challengeScalar = bytesToBigInt(challengeBytes) % secp256k1.CURVE.n;
    const negChallenge = (secp256k1.CURVE.n - challengeScalar) % secp256k1.CURVE.n;

    const sG = secp256k1.Point.BASE.multiplyUnsafe(sNum);
    const eP = signerPoint.multiplyUnsafe(negChallenge);
    const baseR = sG.add(eP);
    if (baseR.equals(secp256k1.Point.ZERO)) {
      return { ok: false, reason: 'calculated base R is zero' };
    }
    baseR.assertValidity();

    const adaptorPoint = secp256k1.Point.fromHex(adaptorPubkey);
    const adaptedR = baseR.add(adaptorPoint);
    if (adaptedR.equals(secp256k1.Point.ZERO)) {
      return { ok: false, reason: 'calculated adapted R is infinity' };
    }
    adaptedR.assertValidity();
    if (adaptedR.toAffine().y % 2n !== 0n) {
      return { ok: false, reason: 'calculated adapted R y-value is odd' };
    }
    if (adaptedR.toAffine().x !== rNum) {
      return { ok: false, reason: 'calculated adapted R x does not match signature r' };
    }

    return { ok: true };
  } catch (error) {
    return { ok: false, reason: toDebugReason(error) };
  }
}

function computeSignatureShareRustCompat(params: {
  groupCommitmentElement: Uint8Array;
  signerNonces: any;
  bindingFactor: any;
  lambdaI: unknown;
  keyPackage: any;
  challenge: any;
}): Uint8Array {
  const negateNonces = !hasEvenYPublicKey(params.groupCommitmentElement);
  const hiding = scalarFromLike((params.signerNonces as any).hiding, 'signerNonces.hiding');
  const binding = scalarFromLike((params.signerNonces as any).binding, 'signerNonces.binding');
  const adjustedHiding = negateNonces
    ? Secp256K1Sha256TR.scalarSub(Secp256K1Sha256TR.scalarZero(), hiding)
    : hiding;
  const adjustedBinding = negateNonces
    ? Secp256K1Sha256TR.scalarSub(Secp256K1Sha256TR.scalarZero(), binding)
    : binding;

  const bindingFactorScalar = scalarFromLike(params.bindingFactor, 'bindingFactor');
  const lambdaScalar = scalarFromLike(params.lambdaI, 'lambdaI');
  let signingShareScalar = scalarFromLike(params.keyPackage.signingShare, 'keyPackage.signingShare');
  const challengeScalar = scalarFromLike(params.challenge, 'challenge');

  const bindingTimesRho = Secp256K1Sha256TR.scalarMul(adjustedBinding, bindingFactorScalar);
  const lambdaTimesShare = Secp256K1Sha256TR.scalarMul(lambdaScalar, signingShareScalar);
  const lambdaShareChallenge = Secp256K1Sha256TR.scalarMul(lambdaTimesShare, challengeScalar);
  const hidingPlusBinding = Secp256K1Sha256TR.scalarAdd(adjustedHiding, bindingTimesRho);
  const zShare = Secp256K1Sha256TR.scalarAdd(hidingPlusBinding, lambdaShareChallenge);

  return FrostSignatureShare.fromScalar(Secp256K1Sha256TR, zShare as any).serialize();
}

function normalizePublicKeyPackageForPreAggregate<T extends {
  verifyingKey: any;
  verifyingShares: Map<string, any>;
}>(publicKeyPackage: T): T {
  if (hasEvenYPublicKey(publicKeyPackage.verifyingKey)) {
    return publicKeyPackage;
  }

  const negatedVerifyingShares = new Map<string, any>();
  for (const [identifier, share] of publicKeyPackage.verifyingShares.entries()) {
    const shareElement =
      typeof share?.toElement === 'function'
        ? share.toElement()
        : share;
    negatedVerifyingShares.set(
      identifier,
      Secp256K1Sha256TR.elementSub(Secp256K1Sha256TR.identity(), shareElement),
    );
  }

  return {
    ...publicKeyPackage,
    verifyingKey: Secp256K1Sha256TR.elementSub(
      Secp256K1Sha256TR.identity(),
      publicKeyPackage.verifyingKey,
    ),
    verifyingShares: negatedVerifyingShares,
  };
}

type SparkDebugCheckpoint = {
  phase: string;
  ts: number;
  meta?: Record<string, unknown>;
};

type SparkDebugHook =
  | ((checkpoint: SparkDebugCheckpoint) => void)
  | {
      enabled?: boolean;
      emit?: (checkpoint: SparkDebugCheckpoint) => void;
    };

function toDebugReason(error: unknown): string {
  const message = error instanceof Error ? error.message : String(error);
  if (message.length <= 200) {
    return message;
  }
  return `${message.slice(0, 200)}...`;
}

function emitSparkDebugCheckpoint(phase: string, meta: Record<string, unknown> = {}): void {
  const runtime = globalThis as typeof globalThis & {
    __LNI_SPARK_DEBUG__?: SparkDebugHook;
  };
  const hook = runtime.__LNI_SPARK_DEBUG__;

  if (!hook) {
    return;
  }

  const checkpoint: SparkDebugCheckpoint = {
    phase,
    ts: Date.now(),
    meta,
  };

  try {
    if (typeof hook === 'function') {
      hook(checkpoint);
      return;
    }

    if (hook.enabled !== false && typeof hook.emit === 'function') {
      hook.emit(checkpoint);
    }
  } catch {}
}

async function pureSignFrost(params: SignFrostBindingParamsLike): Promise<Uint8Array> {
  const commitmentKeys = Object.keys(params.statechainCommitments ?? {});
  emitSparkDebugCheckpoint('sign_frost:start', {
    messageBytes: params.message.length,
    statechainCommitments: commitmentKeys.length,
    firstCommitmentKeyLen: commitmentKeys[0]?.length ?? 0,
    firstCommitmentKeyPrefix: commitmentKeys[0]?.slice(0, 8) ?? '',
    hasAdaptor: Boolean(params.adaptorPubKey && params.adaptorPubKey.length > 0),
    adaptorInputBytes: params.adaptorPubKey?.length ?? 0,
    adaptorInputPrefix:
      params.adaptorPubKey && params.adaptorPubKey.length > 0
        ? Number(params.adaptorPubKey[0]).toString(16).padStart(2, '0')
        : '',
  });

  try {
    const userIdentifier = createUserIdentifier();
    const userIdentifierHex = identifierToHex(userIdentifier);
    const keyPackage = buildUserKeyPackage(params.keyPackage, userIdentifier);
    const preSignedKeyPackage = intoEvenYKeyPackage(keyPackage as any) as any;
    const hiding = FrostNonce.deserialize(Secp256K1Sha256TR, params.nonce.hiding);
    const binding = FrostNonce.deserialize(Secp256K1Sha256TR, params.nonce.binding);
    const nonces = FrostSigningNonces.fromNonces(Secp256K1Sha256TR, hiding, binding);
    const signingPackage = buildSigningPackage(
      params.message,
      params.selfCommitment,
      userIdentifier,
      params.statechainCommitments,
    );

    const adaptorPublicKey = normalizeAdaptorPublicKey(params.adaptorPubKey);
    const statechainIds = Object.keys(params.statechainCommitments ?? {}).sort();
    emitSparkDebugCheckpoint('sign_frost:package_ready', {
      adaptorCompressedBytes: adaptorPublicKey?.length ?? 0,
      userIdentifierHex,
      statechainIds,
    });

    const bindingFactorList = Secp256K1Sha256TR.computeBindingFactorList(
      signingPackage,
      preSignedKeyPackage.verifyingKey,
      new Uint8Array(),
    );
    const bindingFactor = bindingFactorList.get(preSignedKeyPackage.identifier);
    if (!bindingFactor) {
      throw new LniError('Api', 'Failed to compute Spark signing binding factor.');
    }
    emitSparkDebugCheckpoint('sign_frost:binding_factor_ready');

    const groupCommitment = Secp256K1Sha256TR.computeGroupCommitment(signingPackage, bindingFactorList);
    const challengeCommitment = adaptorPublicKey
      ? Secp256K1Sha256TR.elementAdd(groupCommitment.toElement(), adaptorPublicKey)
      : groupCommitment.toElement();
    const lambdaI = Secp256K1Sha256TR.scalarOne();
    const challenge = Secp256K1Sha256TR.challenge(
      challengeCommitment,
      preSignedKeyPackage.verifyingKey,
      signingPackage.message,
    );
    emitSparkDebugCheckpoint('sign_frost:challenge_ready');

    emitSparkDebugCheckpoint('sign_frost:parity_adjusted');

    const groupCommitmentElement = adaptorPublicKey
      ? challengeCommitment
      : groupCommitment.toElement();
    const serialized = computeSignatureShareRustCompat({
      groupCommitmentElement,
      signerNonces: nonces,
      bindingFactor,
      lambdaI,
      keyPackage: preSignedKeyPackage,
      challenge,
    });
    emitSparkDebugCheckpoint('sign_frost:complete');
    return serialized;
  } catch (error) {
    emitSparkDebugCheckpoint('sign_frost:error', {
      reason: toDebugReason(error),
    });
    throw error;
  }
}

async function pureAggregateFrost(params: AggregateFrostBindingParamsLike): Promise<Uint8Array> {
  const signatureKeys = Object.keys(params.statechainSignatures ?? {});
  emitSparkDebugCheckpoint('aggregate_frost:start', {
    statechainSignatures: signatureKeys.length,
    firstSignatureKeyLen: signatureKeys[0]?.length ?? 0,
    firstSignatureKeyPrefix: signatureKeys[0]?.slice(0, 8) ?? '',
    hasAdaptor: Boolean(params.adaptorPubKey && params.adaptorPubKey.length > 0),
  });

  try {
    const signingPackage = buildSigningPackage(
      params.message,
      params.selfCommitment,
      createUserIdentifier(),
      params.statechainCommitments,
    );

    const signatureShares = new Map<any, any>();
    for (const [identifier, shareBytes] of Object.entries(params.statechainSignatures ?? {})) {
      signatureShares.set(identifierFromHex(identifier), FrostSignatureShare.deserialize(Secp256K1Sha256TR, shareBytes));
    }
    const selfIdentifier = createUserIdentifier();
    signatureShares.set(selfIdentifier, FrostSignatureShare.deserialize(Secp256K1Sha256TR, params.selfSignature));

    const verifyingShares = new Map<string, any>();
    for (const [identifier, publicKey] of Object.entries(params.statechainPublicKeys ?? {})) {
      verifyingShares.set(identifier, FrostVerifyingShare.deserialize(Secp256K1Sha256TR, publicKey));
    }
    verifyingShares.set(identifierToHex(selfIdentifier), FrostVerifyingShare.deserialize(Secp256K1Sha256TR, params.selfPublicKey));

    const verifyingKey = FrostVerifyingKey.deserialize(Secp256K1Sha256TR, params.verifyingKey).toElement();
    const publicKeyPackage = new FrostPublicKeyPackage(
      Secp256K1Sha256TR,
      verifyingShares,
      verifyingKey,
      1,
    );
    const adaptorPublicKey = normalizeAdaptorPublicKey(params.adaptorPubKey);

    const tweakedPublicKeyPackage = tweakPublicKeyPackage(
      publicKeyPackage as any,
      new Uint8Array(),
    ) as any;
    const preAggregatedPublicKeyPackage = normalizePublicKeyPackageForPreAggregate(
      tweakedPublicKeyPackage,
    );
    const bindingFactorList = Secp256K1Sha256TR.computeBindingFactorList(
      signingPackage,
      preAggregatedPublicKeyPackage.verifyingKey,
      new Uint8Array(),
    );
    const groupCommitment = Secp256K1Sha256TR.computeGroupCommitment(signingPackage, bindingFactorList);

    if (!adaptorPublicKey) {
      let z = Secp256K1Sha256TR.scalarZero();
      for (const signatureShare of signatureShares.values()) {
        z = Secp256K1Sha256TR.scalarAdd(z, scalarFromLike(signatureShare, 'signatureShare'));
      }
      const signature = new FrostSignature(groupCommitment.toElement(), z);
      const serialized = signature.serialize(Secp256K1Sha256TR);
      emitSparkDebugCheckpoint('aggregate_frost:complete', {
        mode: 'standard',
      });
      return serialized;
    }

    const challengeCommitment = Secp256K1Sha256TR.elementAdd(
      groupCommitment.toElement(),
      adaptorPublicKey,
    );
    const adaptedGroupCommitment = hasEvenYPublicKey(challengeCommitment)
      ? challengeCommitment
      : Secp256K1Sha256TR.elementSub(Secp256K1Sha256TR.identity(), challengeCommitment);

    let z = Secp256K1Sha256TR.scalarZero();
    for (const signatureShare of signatureShares.values()) {
      z = Secp256K1Sha256TR.scalarAdd(z, scalarFromLike(signatureShare, 'signatureShare'));
    }

    const zCandidates = [
      z,
      Secp256K1Sha256TR.scalarSub(Secp256K1Sha256TR.scalarZero(), z),
    ];

    let fallbackSerialized: Uint8Array | undefined;
    const candidateDiagnostics: Array<Record<string, unknown>> = [];
    for (const [candidateIndex, candidateZ] of zCandidates.entries()) {
      const preSignature = new FrostSignature(adaptedGroupCommitment, candidateZ);
      const serialized = preSignature.serialize(Secp256K1Sha256TR);
      if (!fallbackSerialized) {
        fallbackSerialized = serialized;
      }

      const validation = validateOutboundAdaptorSignatureLocal({
        signature: serialized,
        message: params.message,
        pubkey: preAggregatedPublicKeyPackage.verifyingKey,
        adaptorPubkey: adaptorPublicKey,
      });
      candidateDiagnostics.push({
        candidateIndex,
        valid: validation.ok,
        reason: validation.ok ? 'ok' : validation.reason,
      });

      if (validation.ok) {
        emitSparkDebugCheckpoint('aggregate_frost:complete', {
          mode: 'adaptor',
          candidateIndex,
        });
        return serialized;
      }
    }

    emitSparkDebugCheckpoint('aggregate_frost:adaptor_validation_failed', {
      candidates: candidateDiagnostics,
    });
    throw new LniError(
      'Api',
      `Adaptor signature validation failed: no z-candidate passed validation (${candidateDiagnostics.length} tried).`,
    );
  } catch (error) {
    emitSparkDebugCheckpoint('aggregate_frost:error', {
      reason: toDebugReason(error),
    });
    throw error;
  }
}

async function pureCreateDummyTx(address: string, amountSats: bigint): Promise<{ tx: Uint8Array; txid: string }> {
  const tx = new BtcTransaction({ version: 3 });
  tx.addInput({
    txid: new Uint8Array(32),
    index: 0,
    sequence: 0,
  });
  tx.addOutputAddress(address, amountSats);
  return {
    tx: tx.toBytes(),
    txid: tx.id,
  };
}

async function pureEncryptEcies(msg: Uint8Array, publicKey: Uint8Array): Promise<Uint8Array> {
  return Uint8Array.from(eciesEncrypt(publicKey, msg));
}

async function pureDecryptEcies(encryptedMsg: Uint8Array, privateKey: Uint8Array): Promise<Uint8Array> {
  return Uint8Array.from(eciesDecrypt(privateKey, encryptedMsg));
}

function extractPaymentHashFromInvoice(invoice: string): string {
  if (!invoice) {
    return '';
  }

  try {
    const decoded = decodeBolt11(invoice) as { sections?: Array<{ name?: string; value?: unknown }> };
    const section = decoded.sections?.find((entry) => entry.name === 'payment_hash');
    return typeof section?.value === 'string' ? section.value : '';
  } catch {
    return '';
  }
}

function extractExpiryFromInvoice(invoice: string): number {
  if (!invoice) {
    return 0;
  }

  try {
    const decoded = decodeBolt11(invoice) as { expiry?: unknown };
    return numberFromUnknown(decoded.expiry);
  } catch {
    return 0;
  }
}

function extractAmountMsatsFromInvoice(invoice: string): number | undefined {
  if (!invoice) {
    return undefined;
  }

  try {
    const decoded = decodeBolt11(invoice) as { sections?: Array<{ name?: string; value?: unknown }> };
    const section = decoded.sections?.find((entry) => entry.name === 'amount');
    const amountMsats = numberFromUnknown(section?.value);
    if (amountMsats > 0) {
      return Math.floor(amountMsats);
    }
  } catch {
    return undefined;
  }

  return undefined;
}

async function sha256HexOfHexString(hex: string): Promise<string> {
  if (!hex) {
    return '';
  }

  const bytes = hexToBytes(hex);
  return bytesToHex(sha256(bytes));
}

function isSettledStatus(status: unknown): boolean {
  if (typeof status !== 'string') {
    return false;
  }

  return status.includes('COMPLETED') || status.includes('FINALIZED');
}

function mapSparkTransferToTransaction(transfer: unknown): Transaction {
  const item = (transfer ?? {}) as {
    id?: unknown;
    status?: unknown;
    totalValue?: unknown;
    createdTime?: unknown;
    transferDirection?: unknown;
    sparkInvoice?: unknown;
    userRequest?: unknown;
  };

  const userRequest = (item.userRequest ?? {}) as Record<string, unknown>;
  const requestInvoice = (userRequest.invoice ?? {}) as Record<string, unknown>;
  const invoice =
    (typeof userRequest.encodedInvoice === 'string' ? userRequest.encodedInvoice : '') ||
    (typeof requestInvoice.encodedInvoice === 'string' ? requestInvoice.encodedInvoice : '') ||
    (typeof item.sparkInvoice === 'string' ? item.sparkInvoice : '');

  const paymentPreimage = typeof userRequest.paymentPreimage === 'string' ? userRequest.paymentPreimage : '';
  const paymentHash =
    (typeof requestInvoice.paymentHash === 'string' ? requestInvoice.paymentHash : '') ||
    (typeof userRequest.paymentHash === 'string' ? userRequest.paymentHash : '') ||
    extractPaymentHashFromInvoice(invoice);
  const createdAt =
    toUnixSecondsFromAny(item.createdTime) ||
    toUnixSecondsFromAny((requestInvoice as { createdAt?: unknown }).createdAt);
  const expiresAt =
    toUnixSecondsFromAny(requestInvoice.expiresAt) ||
    (createdAt ? createdAt + extractExpiryFromInvoice(invoice) : 0);
  const feeMsats = mapCurrencyAmountToMsats(userRequest.fee);
  const transferDirection = typeof item.transferDirection === 'string' ? item.transferDirection : '';

  return emptyTransaction({
    type: transferDirection === 'INCOMING' ? 'incoming' : 'outgoing',
    invoice,
    description:
      (typeof requestInvoice.memo === 'string' ? requestInvoice.memo : '') ||
      (typeof userRequest.memo === 'string' ? userRequest.memo : ''),
    descriptionHash: '',
    preimage: paymentPreimage,
    paymentHash,
    amountMsats: mapCurrencyAmountToMsats(item.totalValue),
    feesPaid: feeMsats,
    createdAt,
    expiresAt,
    settledAt: isSettledStatus(item.status) ? createdAt : 0,
    externalId: typeof item.id === 'string' ? item.id : '',
  });
}

function resolveEntry(config: SparkConfig): SparkSdkEntry {
  return config.sdkEntry ?? 'auto';
}

function isNodeRuntime(): boolean {
  const runtime = globalThis as typeof globalThis & {
    navigator?: { product?: string };
    process?: { versions?: { node?: string } };
  };
  return Boolean(runtime.process?.versions?.node && runtime.navigator?.product !== 'ReactNative');
}

async function importSparkSdkCandidate(specifier: string): Promise<SparkSdkModuleLike> {
  if (specifier === SPARK_SDK_PACKAGED_BARE_ENTRY) {
    return (await import('../vendor/spark-sdk-bare.js')) as SparkSdkModuleLike;
  }
  if (specifier === SPARK_SDK_DEFAULT_ENTRY) {
    return (await import('@buildonspark/spark-sdk')) as SparkSdkModuleLike;
  }
  if (specifier === SPARK_SDK_NATIVE_ENTRY) {
    return (await import('@buildonspark/spark-sdk/native')) as SparkSdkModuleLike;
  }
  throw new LniError('InvalidInput', `Unsupported Spark SDK entry: ${specifier}`);
}

export class SparkNode implements LightningNode {
  private sdkPromise?: Promise<SparkSdkModuleLike>;
  private walletPromise?: Promise<SparkWalletLike>;
  private pureFrostInstalled = false;

  constructor(private readonly config: SparkConfig, _options: NodeRequestOptions = {}) {
    if (!config.mnemonic?.trim()) {
      throw new LniError('InvalidInput', 'Spark mnemonic is required.');
    }
  }

  private async loadSdk(): Promise<SparkSdkModuleLike> {
    if (this.sdkPromise) {
      return this.sdkPromise;
    }

    this.sdkPromise = (async () => {
      const entry = resolveEntry(this.config);
      const candidates =
        entry === 'native'
          ? [SPARK_SDK_NATIVE_ENTRY]
          : entry === 'default'
            ? [SPARK_SDK_DEFAULT_ENTRY]
          : entry === 'bare'
              ? [SPARK_SDK_PACKAGED_BARE_ENTRY]
              : isNodeRuntime()
                ? [SPARK_SDK_DEFAULT_ENTRY]
                : [SPARK_SDK_PACKAGED_BARE_ENTRY, SPARK_SDK_DEFAULT_ENTRY];

      let lastError: unknown;
      for (const specifier of candidates) {
        try {
          const module = await importSparkSdkCandidate(specifier);
          await this.installPureSparkFrost(module);
          return module;
        } catch (error) {
          lastError = error;
        }
      }

      this.sdkPromise = undefined;
      throw new LniError(
        'Api',
        `Failed to load Spark SDK entry (${candidates.join(', ')}): ${(lastError as Error)?.message ?? 'unknown error'}`,
        { cause: lastError },
      );
    })();

    return this.sdkPromise;
  }

  private async installPureSparkFrost(module: SparkSdkModuleLike): Promise<void> {
    if (this.pureFrostInstalled) {
      return;
    }

    const applyPureMethods = (sparkFrost: SparkFrostLike): SparkFrostLike => {
      sparkFrost.signFrost = async (params) => pureSignFrost(params);
      sparkFrost.aggregateFrost = async (params) => pureAggregateFrost(params);

      sparkFrost.createDummyTx = pureCreateDummyTx;
      sparkFrost.encryptEcies = pureEncryptEcies;
      sparkFrost.decryptEcies = pureDecryptEcies;
      return sparkFrost;
    };

    if (typeof module.setSparkFrostOnce === 'function' && typeof module.SparkFrostBase === 'function') {
      const sparkFrost = applyPureMethods(new module.SparkFrostBase());
      module.setSparkFrostOnce(sparkFrost);
      this.pureFrostInstalled = true;
      return;
    }

    if (typeof module.getSparkFrost !== 'function') {
      throw new LniError(
        'Api',
        'Spark SDK entry does not expose SparkFrost hooks required for pure TypeScript mode.',
      );
    }

    const sparkFrost = applyPureMethods(module.getSparkFrost());
    this.pureFrostInstalled = true;
  }

  private async getWallet(): Promise<SparkWalletLike> {
    if (this.walletPromise) {
      return this.walletPromise;
    }

    this.walletPromise = (async () => {
      try {
        const sdk = await this.loadSdk();
        const mnemonic = this.config.mnemonic.trim();
        const mnemonicOrSeed = this.config.passphrase
          ? mnemonicToSeedSync(mnemonic, this.config.passphrase)
          : mnemonic;
        const init = await sdk.SparkWallet.initialize({
          mnemonicOrSeed,
          accountNumber: this.config.accountNumber,
          options: {
            network: mapNetworkToSpark(this.config.network),
            ...removeUndefinedValues(this.config.sparkOptions ?? {}),
          },
        });

        return init.wallet;
      } catch (error) {
        this.walletPromise = undefined;
        throw error;
      }
    })();

    return this.walletPromise;
  }

  async getInfo(): Promise<NodeInfo> {
    const wallet = await this.getWallet();
    const [balance, identityPublicKey] = await Promise.all([
      wallet.getBalance(),
      wallet.getIdentityPublicKey(),
    ]);
    const sendBalanceSats = await resolveWalletBalanceSats(wallet, balance);

    return emptyNodeInfo({
      alias: 'Spark Node',
      pubkey: identityPublicKey,
      network: this.config.network ?? 'mainnet',
      sendBalanceMsat: sendBalanceSats * 1000,
    });
  }

  async createInvoice(params: CreateInvoiceParams): Promise<Transaction> {
    const invoiceType = params.invoiceType ?? InvoiceType.Bolt11;
    if (invoiceType === InvoiceType.Bolt12) {
      throw new LniError('Api', 'Bolt12 offers are not implemented for SparkNode.');
    }

    const wallet = await this.getWallet();
    const amountSats = params.amountMsats ? Math.max(1, Math.floor(params.amountMsats / 1000)) : 0;
    const now = Math.floor(Date.now() / 1000);
    const response = await wallet.createLightningInvoice({
      amountSats,
      memo: params.description,
      expirySeconds: params.expiry,
      descriptionHash: params.descriptionHash,
    });

    const invoiceObject = (response as { invoice?: { encodedInvoice?: string; paymentHash?: string; memo?: string; expiresAt?: unknown } }).invoice;
    const invoice = invoiceObject?.encodedInvoice ?? '';
    const paymentHash = invoiceObject?.paymentHash ?? extractPaymentHashFromInvoice(invoice);
    const createdAt = toUnixSecondsFromAny((response as { createdAt?: unknown }).createdAt) || now;
    const expirySeconds = params.expiry ?? (extractExpiryFromInvoice(invoice) || 3600);
    const expiresAt =
      toUnixSecondsFromAny(invoiceObject?.expiresAt) ||
      (createdAt + expirySeconds);

    return emptyTransaction({
      type: 'incoming',
      invoice,
      paymentHash,
      amountMsats: params.amountMsats ?? amountSats * 1000,
      createdAt,
      expiresAt,
      description: invoiceObject?.memo ?? params.description ?? '',
      descriptionHash: params.descriptionHash ?? '',
    });
  }

  async payInvoice(params: PayInvoiceParams): Promise<PayInvoiceResponse> {
    const amountMsatsFromInvoice = extractAmountMsatsFromInvoice(params.invoice);
    const hasInvoiceAmount = amountMsatsFromInvoice !== undefined;
    const providedAmountMsats = params.amountMsats;
    const isAmountlessInvoice = !hasInvoiceAmount;

    emitSparkDebugCheckpoint('pay_invoice:start', {
      hasAmountMsats: providedAmountMsats !== undefined,
      hasAmountInInvoice: hasInvoiceAmount,
      isAmountlessInvoice,
      hasFeeLimitMsat: params.feeLimitMsat !== undefined,
      invoiceChars: params.invoice.length,
    });

    if (isAmountlessInvoice && (!providedAmountMsats || providedAmountMsats <= 0)) {
      throw new LniError(
        'InvalidInput',
        'Spark amountless invoice requires amountMsats.',
      );
    }

    try {
      const wallet = await this.getWallet();
      emitSparkDebugCheckpoint('pay_invoice:wallet_ready');

      const amountSatsToSend = isAmountlessInvoice && providedAmountMsats
        ? Math.max(1, Math.floor(providedAmountMsats / 1000))
        : undefined;
      const maxFeeSats = params.feeLimitMsat
        ? Math.max(1, Math.ceil(params.feeLimitMsat / 1000))
        : (this.config.defaultMaxFeeSats ?? DEFAULT_MAX_FEE_SATS);

      emitSparkDebugCheckpoint('pay_invoice:submit', {
        hasAmountSatsToSend: amountSatsToSend !== undefined,
        amountSource:
          providedAmountMsats !== undefined
            ? 'params'
            : 'none',
        maxFeeSats,
      });
      const response = await wallet.payLightningInvoice({
        invoice: params.invoice,
        maxFeeSats,
        amountSatsToSend,
        preferSpark: false,
      });

      const initResult = response as {
        id?: string;
        status?: string;
        paymentPreimage?: unknown;
        fee?: unknown;
        userRequest?: unknown;
      };
      emitSparkDebugCheckpoint('pay_invoice:response_received', {
        status: initResult.status,
      });

      let preimage =
        typeof initResult.paymentPreimage === 'string' ? initResult.paymentPreimage : '';
      let fee = initResult.fee;

      // Poll for completion if payment was only initiated
      const requestId = initResult.id;
      if (!preimage && requestId && typeof wallet.getLightningSendRequest === 'function') {
        const terminalStatuses = new Set([
          'LIGHTNING_PAYMENT_SUCCEEDED',
          'PREIMAGE_PROVIDED',
          'TRANSFER_COMPLETED',
          'LIGHTNING_PAYMENT_FAILED',
          'TRANSFER_FAILED',
          'USER_TRANSFER_VALIDATION_FAILED',
          'PREIMAGE_PROVIDING_FAILED',
          'USER_SWAP_RETURNED',
          'USER_SWAP_RETURN_FAILED',
        ]);
        const maxPollMs = 60_000;
        const pollIntervalMs = 2_000;
        const startedAt = Date.now();

        while (Date.now() - startedAt < maxPollMs) {
          await new Promise((r) => setTimeout(r, pollIntervalMs));
          try {
            const req = await wallet.getLightningSendRequest(requestId);
            const status = req?.status ?? '';
            emitSparkDebugCheckpoint('pay_invoice:poll', { status });
            if (req?.paymentPreimage) {
              preimage = req.paymentPreimage;
              if (req.fee) {
                fee = req.fee;
              }
              break;
            }
            if (terminalStatuses.has(status)) {
              if (status.includes('FAILED') || status.includes('RETURN')) {
                throw new LniError('Api', `Spark Lightning payment failed: ${status}`);
              }
              break;
            }
          } catch (error) {
            if (error instanceof LniError) {
              throw error;
            }
            emitSparkDebugCheckpoint('pay_invoice:poll_error', {
              reason: toDebugReason(error),
            });
          }
        }
      }

      let paymentHash = extractPaymentHashFromInvoice(params.invoice);
      if (!paymentHash && preimage) {
        paymentHash = await sha256HexOfHexString(preimage);
      }

      emitSparkDebugCheckpoint('pay_invoice:complete', {
        hasPaymentHash: Boolean(paymentHash),
        hasPreimage: Boolean(preimage),
      });
      return {
        paymentHash,
        preimage,
        feeMsats: mapCurrencyAmountToMsats(fee),
      };
    } catch (error) {
      emitSparkDebugCheckpoint('pay_invoice:error', {
        reason: toDebugReason(error),
      });
      throw error;
    }
  }

  async createOffer(_params: CreateOfferParams): Promise<Offer> {
    throw new LniError('Api', 'Bolt12 offers are not implemented for SparkNode.');
  }

  async getOffer(_search?: string): Promise<Offer> {
    throw new LniError('Api', 'Bolt12 offers are not implemented for SparkNode.');
  }

  async listOffers(_search?: string): Promise<Offer[]> {
    throw new LniError('Api', 'Bolt12 offers are not implemented for SparkNode.');
  }

  async payOffer(_offer: string, _amountMsats: number, _payerNote?: string): Promise<PayInvoiceResponse> {
    throw new LniError('Api', 'Bolt12 offers are not implemented for SparkNode.');
  }

  private async scanTransactions(
    params: {
      from: number;
      limit: number;
      paymentHash?: string;
      search?: string;
    },
  ): Promise<Transaction[]> {
    const wallet = await this.getWallet();
    const from = Math.max(0, params.from || 0);
    const limit = params.limit > 0 ? params.limit : DEFAULT_SCAN_LIMIT;
    const pageSize = Math.min(DEFAULT_PAGE_SIZE, Math.max(1, limit));
    const results: Transaction[] = [];
    let offset = from;
    let scanned = 0;

    while (results.length < limit && scanned < DEFAULT_SCAN_LIMIT) {
      const page = await wallet.getTransfers(pageSize, offset);
      const transfers = Array.isArray(page.transfers) ? page.transfers : [];
      if (!transfers.length) {
        break;
      }

      for (const transfer of transfers) {
        const tx = mapSparkTransferToTransaction(transfer);
        if (params.paymentHash && tx.paymentHash !== params.paymentHash) {
          continue;
        }
        if (!matchesSearch(tx, params.search)) {
          continue;
        }
        results.push(tx);
        if (results.length >= limit) {
          break;
        }
      }

      const nextOffset = numberFromUnknown(page.offset);
      offset = nextOffset > offset ? nextOffset : offset + transfers.length;
      scanned += transfers.length;
      if (transfers.length < pageSize) {
        break;
      }
    }

    return results;
  }

  async lookupInvoice(params: LookupInvoiceParams): Promise<Transaction> {
    if (!params.paymentHash && !params.search) {
      throw new LniError('InvalidInput', 'lookupInvoice requires paymentHash or search for SparkNode.');
    }

    const txs = await this.scanTransactions({
      from: 0,
      limit: DEFAULT_SCAN_LIMIT,
      paymentHash: params.paymentHash,
      search: params.search,
    });

    const tx = txs[0];
    if (!tx) {
      throw new LniError(
        'Api',
        `Invoice not found for SparkNode (paymentHash=${params.paymentHash ?? ''}, search=${params.search ?? ''}).`,
      );
    }
    return tx;
  }

  async listTransactions(params: ListTransactionsParams): Promise<Transaction[]> {
    const txs = await this.scanTransactions({
      from: params.from,
      limit: params.limit > 0 ? params.limit : DEFAULT_SCAN_LIMIT,
      paymentHash: params.paymentHash,
      search: params.search,
    });

    return txs.sort((a, b) => b.createdAt - a.createdAt);
  }

  async decode(str: string): Promise<string> {
    try {
      return JSON.stringify(decodeBolt11(str));
    } catch {
      return str;
    }
  }

  async onInvoiceEvents(params: OnInvoiceEventParams, callback: InvoiceEventCallback): Promise<void> {
    await pollInvoiceEvents({
      params,
      callback,
      lookup: () =>
        this.lookupInvoice({
          paymentHash: params.paymentHash,
          search: params.search,
        }),
    });
  }

  async cleanupConnections(): Promise<void> {
    if (!this.walletPromise) {
      return;
    }
    const wallet = await this.walletPromise;
    await wallet.cleanupConnections?.();
  }
}
