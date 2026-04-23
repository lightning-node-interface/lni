import type { StorageProvider } from '@sunnyln/lni';

export type SparkNetwork = 'mainnet' | 'regtest' | 'testnet' | 'signet' | 'local';

export interface SparkConfig {
  // 12/24-word seed phrase.
  mnemonic: string;
  // Optional passphrase applied to mnemonic->seed derivation.
  passphrase?: string;
  // Spark account index. If omitted, spark-sdk applies its default per-network.
  accountNumber?: number;
  // Spark network. Defaults to 'mainnet'.
  network?: SparkNetwork;
  // Optional override for spark-sdk runtime entrypoint loading strategy.
  // - auto: browser/Expo uses packaged no-WASM bare vendor; Node uses '@buildonspark/spark-sdk'
  // - bare: force packaged no-WASM bare vendor path
  // - native: force '@buildonspark/spark-sdk/native'
  // - default: force '@buildonspark/spark-sdk' (may load WASM depending on runtime)
  sdkEntry?: 'auto' | 'bare' | 'native' | 'default';
  // Optional max fee used by payInvoice when no fee limit is provided.
  defaultMaxFeeSats?: number;
  // Optional spark-sdk wallet options passthrough.
  sparkOptions?: Record<string, unknown>;
  /** Optional storage for persisting the paymentHash -> transferId cache across sessions. */
  storage?: StorageProvider;
}
