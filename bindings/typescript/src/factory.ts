import { BlinkNode } from './nodes/blink.js';
import { ClnNode } from './nodes/cln.js';
import { LndNode } from './nodes/lnd.js';
import { NwcNode } from './nodes/nwc.js';
import { PhoenixdNode } from './nodes/phoenixd.js';
import { SparkNode } from './nodes/spark.js';
import { SpeedNode } from './nodes/speed.js';
import { StrikeNode } from './nodes/strike.js';
import type {
  BackendNodeConfig,
  BlinkConfig,
  ClnConfig,
  LightningNode,
  LndConfig,
  NodeRequestOptions,
  NwcConfig,
  PhoenixdConfig,
  SparkConfig,
  SpeedConfig,
  StrikeConfig,
} from './types.js';

export function createNode(
  input: { kind: 'phoenixd'; config: PhoenixdConfig },
  options?: NodeRequestOptions,
): PhoenixdNode;
export function createNode(
  input: { kind: 'cln'; config: ClnConfig },
  options?: NodeRequestOptions,
): ClnNode;
export function createNode(
  input: { kind: 'lnd'; config: LndConfig },
  options?: NodeRequestOptions,
): LndNode;
export function createNode(
  input: { kind: 'nwc'; config: NwcConfig },
  options?: NodeRequestOptions,
): NwcNode;
export function createNode(
  input: { kind: 'strike'; config: StrikeConfig },
  options?: NodeRequestOptions,
): StrikeNode;
export function createNode(
  input: { kind: 'speed'; config: SpeedConfig },
  options?: NodeRequestOptions,
): SpeedNode;
export function createNode(
  input: { kind: 'blink'; config: BlinkConfig },
  options?: NodeRequestOptions,
): BlinkNode;
export function createNode(
  input: { kind: 'spark'; config: SparkConfig },
  options?: NodeRequestOptions,
): SparkNode;
export function createNode(input: BackendNodeConfig, options?: NodeRequestOptions): LightningNode;
export function createNode(input: BackendNodeConfig, options: NodeRequestOptions = {}): LightningNode {
  switch (input.kind) {
    case 'phoenixd':
      return new PhoenixdNode(input.config, options);
    case 'cln':
      return new ClnNode(input.config, options);
    case 'lnd':
      return new LndNode(input.config, options);
    case 'nwc':
      return new NwcNode(input.config, options);
    case 'strike':
      return new StrikeNode(input.config, options);
    case 'speed':
      return new SpeedNode(input.config, options);
    case 'blink':
      return new BlinkNode(input.config, options);
    case 'spark':
      return new SparkNode(input.config, options);
    default:
      throw new Error(`Unsupported backend kind: ${(input as { kind: string }).kind}`);
  }
}
