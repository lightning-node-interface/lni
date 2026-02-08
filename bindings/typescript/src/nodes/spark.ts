import { Transaction as BtcTransaction } from '@scure/btc-signer';
import { mnemonicToSeedSync } from '@scure/bip39';
import { sha256 } from '@noble/hashes/sha2';
import {
  Identifier as FrostIdentifier,
  KeyPackage as FrostKeyPackage,
  Nonce as FrostNonce,
  NonceCommitment as FrostNonceCommitment,
  PublicKeyPackage as FrostPublicKeyPackage,
  Secp256K1Sha256TR,
  SignatureShare as FrostSignatureShare,
  SigningCommitments as FrostSigningCommitments,
  SigningNonces as FrostSigningNonces,
  SigningPackageImpl as FrostSigningPackageImpl,
  SigningShare as FrostSigningShare,
  VerifyingKey as FrostVerifyingKey,
  VerifyingShare as FrostVerifyingShare,
  aggregateWithTweak,
  hasEvenYPublicKey,
  intoEvenYKeyPackage,
  round2 as frostRound2,
  tweakKeyPackage,
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
const USER_IDENTIFIER = FrostIdentifier.derive(Secp256K1Sha256TR, new TextEncoder().encode('user'));

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
  if (unit.includes('satoshi') || unit === 'sat') {
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

function userIdentifierHex(): string {
  return bytesToHex(USER_IDENTIFIER.serialize());
}

function buildUserKeyPackage(params: SignFrostBindingParamsLike['keyPackage']): any {
  const signingShare = FrostSigningShare.deserialize(Secp256K1Sha256TR, params.secretKey);
  const verifyingShare = FrostVerifyingShare.deserialize(Secp256K1Sha256TR, params.publicKey);
  const verifyingKey = FrostVerifyingKey.deserialize(Secp256K1Sha256TR, params.verifyingKey).toElement();
  const base = new FrostKeyPackage(
    Secp256K1Sha256TR,
    USER_IDENTIFIER,
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
    tweaked.minSigners,
  );
}

function buildSigningPackage(
  message: Uint8Array,
  selfCommitment: SparkSigningCommitmentLike,
  statechainCommitments?: Record<string, SparkSigningCommitmentLike>,
): any {
  const commitments = new Map<any, any>();

  for (const [identifier, commitment] of Object.entries(statechainCommitments ?? {})) {
    commitments.set(identifierFromHex(identifier), signingCommitmentFromBinding(commitment));
  }

  commitments.set(USER_IDENTIFIER, signingCommitmentFromBinding(selfCommitment));

  return new FrostSigningPackageImpl(Secp256K1Sha256TR, commitments, message);
}

async function pureSignFrost(params: SignFrostBindingParamsLike): Promise<Uint8Array> {
  if (params.adaptorPubKey && params.adaptorPubKey.length > 0) {
    throw new LniError('Api', 'Pure TypeScript Spark signer does not support adaptor signatures yet.');
  }

  const keyPackage = buildUserKeyPackage(params.keyPackage);
  const hiding = FrostNonce.deserialize(Secp256K1Sha256TR, params.nonce.hiding);
  const binding = FrostNonce.deserialize(Secp256K1Sha256TR, params.nonce.binding);
  const nonces = FrostSigningNonces.fromNonces(Secp256K1Sha256TR, hiding, binding);
  const signingPackage = buildSigningPackage(
    params.message,
    params.selfCommitment,
    params.statechainCommitments,
  );

  const signatureShare = frostRound2.sign(signingPackage, nonces, keyPackage);
  return signatureShare.serialize();
}

async function pureAggregateFrost(params: AggregateFrostBindingParamsLike): Promise<Uint8Array> {
  if (params.adaptorPubKey && params.adaptorPubKey.length > 0) {
    throw new LniError('Api', 'Pure TypeScript Spark signer does not support adaptor signatures yet.');
  }

  const signingPackage = buildSigningPackage(
    params.message,
    params.selfCommitment,
    params.statechainCommitments,
  );

  const signatureShares = new Map<any, any>();
  for (const [identifier, shareBytes] of Object.entries(params.statechainSignatures ?? {})) {
    signatureShares.set(identifierFromHex(identifier), FrostSignatureShare.deserialize(Secp256K1Sha256TR, shareBytes));
  }
  signatureShares.set(USER_IDENTIFIER, FrostSignatureShare.deserialize(Secp256K1Sha256TR, params.selfSignature));

  const verifyingShares = new Map<string, any>();
  for (const [identifier, publicKey] of Object.entries(params.statechainPublicKeys ?? {})) {
    verifyingShares.set(identifier, FrostVerifyingShare.deserialize(Secp256K1Sha256TR, publicKey));
  }
  verifyingShares.set(userIdentifierHex(), FrostVerifyingShare.deserialize(Secp256K1Sha256TR, params.selfPublicKey));

  const verifyingKey = FrostVerifyingKey.deserialize(Secp256K1Sha256TR, params.verifyingKey).toElement();
  const publicKeyPackage = new FrostPublicKeyPackage(
    Secp256K1Sha256TR,
    verifyingShares,
    verifyingKey,
    1,
  );

  const signature = aggregateWithTweak(signingPackage, signatureShares, publicKeyPackage, new Uint8Array());
  return signature.serialize(Secp256K1Sha256TR);
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
    amountMsats: numberFromUnknown(item.totalValue) * 1000,
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
      sparkFrost.signFrost = pureSignFrost;
      sparkFrost.aggregateFrost = pureAggregateFrost;
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
    const amountSats = params.amountMsats ? Math.floor(params.amountMsats / 1000) : 0;
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
    const wallet = await this.getWallet();
    const amountSatsToSend = params.amountMsats ? Math.floor(params.amountMsats / 1000) : undefined;
    const maxFeeSats = params.feeLimitMsat
      ? Math.max(0, Math.floor(params.feeLimitMsat / 1000))
      : (this.config.defaultMaxFeeSats ?? DEFAULT_MAX_FEE_SATS);

    const response = await wallet.payLightningInvoice({
      invoice: params.invoice,
      maxFeeSats,
      amountSatsToSend,
      preferSpark: false,
    });

    const result = response as { paymentPreimage?: unknown; fee?: unknown; userRequest?: unknown };
    const userRequest = (result.userRequest ?? {}) as Record<string, unknown>;
    const preimage =
      (typeof result.paymentPreimage === 'string' ? result.paymentPreimage : '') ||
      (typeof userRequest.paymentPreimage === 'string' ? userRequest.paymentPreimage : '');
    const fee = result.fee ?? userRequest.fee;
    let paymentHash = extractPaymentHashFromInvoice(params.invoice);

    if (!paymentHash && preimage) {
      paymentHash = await sha256HexOfHexString(preimage);
    }

    return {
      paymentHash,
      preimage,
      feeMsats: mapCurrencyAmountToMsats(fee),
    };
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
