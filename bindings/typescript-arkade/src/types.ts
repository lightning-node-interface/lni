export type ArkadeBoltzNetwork =
  | 'mainnet'
  | 'bitcoin'
  | 'testnet'
  | 'signet'
  | 'mutinynet'
  | 'regtest';

export interface ArkadeBoltzSwapFilter {
  id?: string | string[];
  status?: string | string[];
  type?: string | string[];
  orderBy?: 'createdAt';
  orderDirection?: 'asc' | 'desc';
}

export interface ArkadeBoltzSwapRepository {
  readonly version: 1;
  saveSwap<T = unknown>(swap: T): Promise<void>;
  deleteSwap(id: string): Promise<void>;
  getAllSwaps<T = unknown>(filter?: ArkadeBoltzSwapFilter): Promise<T[]>;
  clear(): Promise<void>;
  [Symbol.asyncDispose]?: () => Promise<void>;
}

export interface ArkadeBoltzWalletStorage {
  walletRepository: unknown;
  contractRepository: unknown;
}

export interface ArkadeBoltzConfig {
  mnemonic: string;
  passphrase?: string;
  network?: ArkadeBoltzNetwork;
  arkServerUrl: string;
  indexerUrl?: string;
  esploraUrl?: string;
  arkServerPublicKey?: string;
  swapApiUrl?: string;
  referralId?: string;
  swapManager?: boolean | Record<string, unknown>;
  swapRepository?: ArkadeBoltzSwapRepository;
  walletStorage?: ArkadeBoltzWalletStorage;
  socks5Proxy?: string;
  acceptInvalidCerts?: boolean;
  httpTimeout?: number;
}
