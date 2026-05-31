import { decode as decodeBolt11 } from 'light-bolt11-decoder';
import type { DecodedInvoice as LightDecodedInvoice } from 'light-bolt11-decoder';

type LightBolt11Section = LightDecodedInvoice['sections'][number];

export interface DecodedInvoice {
  [key: string]: unknown;
  paymentRequest: string;
  type: 'bolt11_invoice';
  network?: {
    bech32: string;
    pubKeyHash: number;
    scriptHash: number;
    validWitnessVersions: number[];
  };
  amount?: string;
  amountMsats?: number;
  amountSats?: number | null;
  timestamp?: number;
  timestampString?: string;
  expiry?: number;
  expiresAt?: number;
  expiresAtString?: string;
  payment_hash?: string;
  payment_secret?: string;
  description?: string;
  description_hash?: string;
  payee_node_key?: string;
  payeeNodeKey?: string;
  min_final_cltv_expiry?: number;
  route_hints?: LightDecodedInvoice['route_hints'];
  feature_bits?: unknown;
  signature?: string;
}

const BECH32_CHARSET = 'qpzry9x8gf2tvdw0s3jn54khce6mua7l';
const MAX_BLINDED_PATH_HOPS = 20;
const MAX_COLLECTION_LENGTH = BigInt(Number.MAX_SAFE_INTEGER);

export interface DecodedOfferSection {
  name: string;
  type?: number;
  value?: unknown;
  raw?: string;
  letters?: string;
}

export interface DecodedBlindedPath {
  introductionNode:
    | { type: 'node_id'; nodeId: string }
    | { type: 'directed_short_channel_id'; direction: 'node_one' | 'node_two'; shortChannelId: string };
  blindingPoint: string;
  blindedHops: Array<{
    blindedNodeId: string;
    encryptedPayload: string;
  }>;
}

export interface DecodedOffer {
  offer: string;
  prefix: 'lno';
  type: 'bolt12_offer';
  sections: DecodedOfferSection[];
  chains: string[];
  metadata?: string;
  currency?: string;
  amount?: string;
  amountMsats?: number;
  description?: string;
  features?: string;
  absoluteExpiry?: number;
  paths?: DecodedBlindedPath[];
  pathsRaw?: string;
  issuer?: string;
  quantityMax?: number | 'unbounded';
  issuerSigningPubkey?: string;
}

export function decode(invoice: string): DecodedInvoice {
  const decoded = decodeBolt11(invoice) as LightDecodedInvoice;
  const result: DecodedInvoice = {
    paymentRequest: decoded.paymentRequest ?? invoice,
    type: 'bolt11_invoice',
    expiresAt: decoded.expiry,
    route_hints: decoded.route_hints,
  };

  for (const section of decoded.sections) {
    applyBolt11Section(result, section);
  }

  if (result.timestamp !== undefined && result.expiry !== undefined) {
    result.expiresAt = result.timestamp + result.expiry;
  }

  return omitUndefined(result);
}

export function decodeBolt11ToJson(invoice: string): string {
  return JSON.stringify(decode(invoice));
}

export function decodeOfferToJson(offer: string): string {
  return JSON.stringify(decodeOffer(offer));
}

export function decodeOffer(offer: string): DecodedOffer {
  const normalized = normalizeBolt12Bech32(offer);
  const separator = normalized.lastIndexOf('1');
  if (separator <= 0) {
    throw new Error('Invalid BOLT12 offer: missing bech32 separator.');
  }

  const prefix = normalized.slice(0, separator).toLowerCase();
  if (prefix !== 'lno') {
    throw new Error(`Invalid BOLT12 offer prefix: ${prefix}`);
  }

  const dataPart = normalized.slice(separator + 1);
  const words = [...dataPart].map((char) => {
    const value = BECH32_CHARSET.indexOf(char);
    if (value === -1) {
      throw new Error(`Invalid BOLT12 offer character: ${char}`);
    }
    return value;
  });

  const bytes = convertBits(words, 5, 8, false);
  const records = parseTlvRecords(bytes);
  const decoded: DecodedOffer = {
    offer,
    prefix: 'lno',
    type: 'bolt12_offer',
    sections: [
      {
        name: 'offer',
        value: offer,
      },
    ],
    chains: [],
  };

  for (const record of records) {
    const hex = bytesToHex(record.value);
    switch (record.type) {
      case 2:
        decoded.chains = chunkHex(record.value, 32);
        decoded.sections.push({ name: 'chains', type: record.type, value: decoded.chains });
        break;
      case 4:
        decoded.metadata = hex;
        decoded.sections.push({ name: 'metadata', type: record.type, value: hex });
        break;
      case 6:
        decoded.currency = textFromBytes(record.value);
        decoded.sections.push({ name: 'currency', type: record.type, value: decoded.currency });
        break;
      case 8: {
        const amount = integerFromBytes(record.value);
        decoded.amount = amount.toString();
        if (!decoded.currency) {
          decoded.amountMsats = Number(amount);
        }
        decoded.sections.push({ name: 'amount', type: record.type, value: decoded.amount });
        break;
      }
      case 10:
        decoded.description = textFromBytes(record.value);
        decoded.sections.push({ name: 'description', type: record.type, value: decoded.description });
        break;
      case 12:
        decoded.features = hex;
        decoded.sections.push({ name: 'features', type: record.type, value: hex });
        break;
      case 14:
        decoded.absoluteExpiry = Number(integerFromBytes(record.value));
        decoded.sections.push({ name: 'absolute_expiry', type: record.type, value: decoded.absoluteExpiry });
        break;
      case 16: {
        const paths = parseBlindedPaths(record.value);
        decoded.paths = paths;
        decoded.pathsRaw = hex;
        decoded.sections.push({ name: 'paths', type: record.type, value: paths, raw: hex });
        break;
      }
      case 18:
        decoded.issuer = textFromBytes(record.value);
        decoded.sections.push({ name: 'issuer', type: record.type, value: decoded.issuer });
        break;
      case 20: {
        const quantity = integerFromBytes(record.value);
        decoded.quantityMax = quantity === 0n ? 'unbounded' : Number(quantity);
        decoded.sections.push({ name: 'quantity_max', type: record.type, value: decoded.quantityMax });
        break;
      }
      case 22:
        decoded.issuerSigningPubkey = hex;
        decoded.sections.push({ name: 'issuer_id', type: record.type, value: hex });
        break;
      default:
        decoded.sections.push({ name: record.type % 2 === 0 ? 'unknown_required' : 'unknown', type: record.type, value: hex });
        break;
    }
  }

  return decoded;
}

function applyBolt11Section(result: DecodedInvoice, section: LightBolt11Section): void {
  const value = sectionValue(section);

  switch (section.name) {
    case 'paymentRequest':
      if (typeof value === 'string') {
        result.paymentRequest = value;
      }
      break;
    case 'coin_network':
      result.coin_network = value;
      if (value && typeof value === 'object') {
        result.network = value as DecodedInvoice['network'];
      }
      break;
    case 'amount':
      if (typeof value === 'string') {
        result.amount = value;
        result.amountMsats = parseSafeInteger(value);
      }
      break;
    case 'timestamp':
      if (typeof value === 'number') {
        result.timestamp = value;
      }
      break;
    case 'expiry':
      if (typeof value === 'number') {
        result.expiry = value;
      }
      break;
    case 'route_hint':
      result.route_hint = value;
      break;
    default:
      result[String(section.name)] = value;
      break;
  }
}

function sectionValue(section: LightBolt11Section): unknown {
  if ('value' in section) {
    return section.value;
  }
  if ('letters' in section) {
    return section.letters;
  }
  return undefined;
}

function parseSafeInteger(value: unknown): number | undefined {
  if (value === undefined || value === null) {
    return undefined;
  }
  const parsed = Number(value);
  return Number.isSafeInteger(parsed) ? parsed : undefined;
}

function omitUndefined<T extends object>(input: T): T {
  const output = { ...input };
  for (const key of Object.keys(output) as Array<keyof T>) {
    if (output[key] === undefined) {
      delete output[key];
    }
  }
  return output;
}

function normalizeBolt12Bech32(input: string): string {
  const hasLower = /[a-z]/.test(input);
  const hasUpper = /[A-Z]/.test(input);
  if (hasLower && hasUpper) {
    throw new Error('Invalid BOLT12 offer: mixed case bech32 string.');
  }

  if (!input.includes('+')) {
    return input.trim().toLowerCase();
  }

  const chunks = input.split('+');
  const [first, ...rest] = chunks;
  if (!first || /\s/.test(first)) {
    throw new Error('Invalid BOLT12 offer continuation.');
  }
  for (const chunk of rest) {
    const trimmed = chunk.trimStart();
    if (!trimmed || /\s/.test(trimmed)) {
      throw new Error('Invalid BOLT12 offer continuation.');
    }
  }

  return chunks.map((chunk, index) => (index === 0 ? chunk : chunk.trimStart())).join('').toLowerCase();
}

function convertBits(values: number[], fromBits: number, toBits: number, pad: boolean): Uint8Array {
  let acc = 0;
  let bits = 0;
  const maxv = (1 << toBits) - 1;
  const result: number[] = [];

  for (const value of values) {
    if (value < 0 || value >> fromBits !== 0) {
      throw new Error('Invalid bech32 data value.');
    }
    acc = (acc << fromBits) | value;
    bits += fromBits;
    while (bits >= toBits) {
      bits -= toBits;
      result.push((acc >> bits) & maxv);
    }
  }

  if (pad) {
    if (bits > 0) {
      result.push((acc << (toBits - bits)) & maxv);
    }
  } else if (bits >= fromBits || ((acc << (toBits - bits)) & maxv) !== 0) {
    throw new Error('Invalid bech32 padding.');
  }

  return Uint8Array.from(result);
}

function parseTlvRecords(bytes: Uint8Array): Array<{ type: number; value: Uint8Array }> {
  const records: Array<{ type: number; value: Uint8Array }> = [];
  let offset = 0;
  let previousType = -1;

  while (offset < bytes.length) {
    const type = readBigSize(bytes, offset);
    offset = type.offset;
    const length = readBigSize(bytes, offset);
    offset = length.offset;
    const end = offset + Number(length.value);

    if (end > bytes.length) {
      throw new Error('Invalid BOLT12 offer TLV length.');
    }
    if (type.value > BigInt(Number.MAX_SAFE_INTEGER)) {
      throw new Error('BOLT12 offer TLV type exceeds safe integer range.');
    }

    const recordType = Number(type.value);
    if (recordType <= previousType) {
      throw new Error('Invalid BOLT12 offer TLV ordering.');
    }
    previousType = recordType;

    records.push({ type: recordType, value: bytes.slice(offset, end) });
    offset = end;
  }

  return records;
}

function readBigSize(bytes: Uint8Array, offset: number): { value: bigint; offset: number } {
  const first = bytes[offset];
  if (first === undefined) {
    throw new Error('Unexpected end of BOLT12 offer TLV stream.');
  }

  if (first < 0xfd) {
    return { value: BigInt(first), offset: offset + 1 };
  }

  const length = first === 0xfd ? 2 : first === 0xfe ? 4 : 8;
  const start = offset + 1;
  const end = start + length;
  if (end > bytes.length) {
    throw new Error('Unexpected end of BOLT12 offer BigSize value.');
  }

  const value = integerFromBytes(bytes.slice(start, end));
  if ((first === 0xfd && value < 0xfdn) || (first === 0xfe && value <= 0xffffn) || (first === 0xff && value <= 0xffffffffn)) {
    throw new Error('Non-canonical BOLT12 offer BigSize value.');
  }

  return { value, offset: end };
}

function parseBlindedPaths(bytes: Uint8Array): DecodedBlindedPath[] {
  const paths: DecodedBlindedPath[] = [];
  let offset = 0;

  while (offset < bytes.length) {
    const parsed = parseBlindedPath(bytes, offset);
    paths.push(parsed.path);
    offset = parsed.offset;
  }

  return paths;
}

function parseBlindedPath(bytes: Uint8Array, offset: number): { path: DecodedBlindedPath; offset: number } {
  const first = readByte(bytes, offset, 'introduction node');
  let currentOffset = offset + 1;

  let introductionNode: DecodedBlindedPath['introductionNode'];
  if (first === 0 || first === 1) {
    const shortChannelId = integerFromBytes(readBytes(bytes, currentOffset, 8, 'short channel id'));
    currentOffset += 8;
    introductionNode = {
      type: 'directed_short_channel_id',
      direction: first === 0 ? 'node_one' : 'node_two',
      shortChannelId: shortChannelId.toString(),
    };
  } else if (first === 2 || first === 3) {
    const nodeId = Uint8Array.from([first, ...readBytes(bytes, currentOffset, 32, 'introduction node id')]);
    currentOffset += 32;
    introductionNode = { type: 'node_id', nodeId: bytesToHex(nodeId) };
  } else {
    throw new Error('Invalid BOLT12 offer blinded path introduction node.');
  }

  const blindingPoint = bytesToHex(readBytes(bytes, currentOffset, 33, 'blinding point'));
  currentOffset += 33;

  const numHops = readByte(bytes, currentOffset, 'blinded hop count');
  currentOffset += 1;
  if (numHops === 0) {
    throw new Error('Invalid BOLT12 offer blinded path: zero hops.');
  }
  if (numHops > MAX_BLINDED_PATH_HOPS) {
    throw new Error(`Invalid BOLT12 offer blinded path: too many hops (${numHops}).`);
  }

  const blindedHops: DecodedBlindedPath['blindedHops'] = [];
  for (let index = 0; index < numHops; index += 1) {
    const blindedNodeId = bytesToHex(readBytes(bytes, currentOffset, 33, 'blinded node id'));
    currentOffset += 33;

    const payloadLength = readCollectionLength(bytes, currentOffset);
    currentOffset = payloadLength.offset;
    if (payloadLength.value > BigInt(Number.MAX_SAFE_INTEGER)) {
      throw new Error('BOLT12 offer encrypted payload length exceeds safe integer range.');
    }
    const payloadBytes = Number(payloadLength.value);
    const encryptedPayload = readBytes(bytes, currentOffset, payloadBytes, 'encrypted payload');
    currentOffset += payloadBytes;

    blindedHops.push({
      blindedNodeId,
      encryptedPayload: bytesToHex(encryptedPayload),
    });
  }

  return {
    path: {
      introductionNode,
      blindingPoint,
      blindedHops,
    },
    offset: currentOffset,
  };
}

function readCollectionLength(bytes: Uint8Array, offset: number): { value: bigint; offset: number } {
  const marker = readBytes(bytes, offset, 2, 'collection length');
  let value = integerFromBytes(marker);
  if (value !== 0xffffn) {
    return { value, offset: offset + 2 };
  }

  const extended = integerFromBytes(readBytes(bytes, offset + 2, 8, 'extended collection length'));
  if (extended < 0n || extended > MAX_COLLECTION_LENGTH - 0xffffn) {
    throw new Error('BOLT12 offer collection length exceeds safe integer range.');
  }
  value = extended + 0xffffn;
  return { value, offset: offset + 10 };
}

function readByte(bytes: Uint8Array, offset: number, label: string): number {
  const value = bytes[offset];
  if (value === undefined) {
    throw new Error(`Unexpected end of BOLT12 offer ${label}.`);
  }
  return value;
}

function readBytes(bytes: Uint8Array, offset: number, length: number, label: string): Uint8Array {
  const end = offset + length;
  if (end > bytes.length) {
    throw new Error(`Unexpected end of BOLT12 offer ${label}.`);
  }
  return bytes.slice(offset, end);
}

function integerFromBytes(bytes: Uint8Array): bigint {
  let value = 0n;
  for (const byte of bytes) {
    value = (value << 8n) | BigInt(byte);
  }
  return value;
}

function bytesToHex(bytes: Uint8Array): string {
  return [...bytes].map((byte) => byte.toString(16).padStart(2, '0')).join('');
}

function chunkHex(bytes: Uint8Array, size: number): string[] {
  const chunks: string[] = [];
  for (let index = 0; index < bytes.length; index += size) {
    chunks.push(bytesToHex(bytes.slice(index, index + size)));
  }
  return chunks;
}

function textFromBytes(bytes: Uint8Array): string {
  return new TextDecoder().decode(bytes);
}
