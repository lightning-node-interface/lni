const BASE64_CHARS = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/';

export function encodeBase64(input: string): string {
  if (typeof globalThis.btoa === 'function') {
    return globalThis.btoa(input);
  }

  const maybeBuffer = (globalThis as { Buffer?: { from(value: string, encoding?: string): { toString(enc: string): string } } }).Buffer;
  if (maybeBuffer) {
    return maybeBuffer.from(input, 'utf8').toString('base64');
  }

  const bytes = new TextEncoder().encode(input);
  let output = '';

  for (let i = 0; i < bytes.length; i += 3) {
    const a = bytes[i] ?? 0;
    const b = bytes[i + 1] ?? 0;
    const c = bytes[i + 2] ?? 0;

    const triple = (a << 16) | (b << 8) | c;

    output += BASE64_CHARS[(triple >> 18) & 63];
    output += BASE64_CHARS[(triple >> 12) & 63];
    output += i + 1 < bytes.length ? BASE64_CHARS[(triple >> 6) & 63] : '=';
    output += i + 2 < bytes.length ? BASE64_CHARS[triple & 63] : '=';
  }

  return output;
}

export function decodeBase64(input: string): Uint8Array {
  const normalized = input.replace(/\s+/g, '');

  if (typeof globalThis.atob === 'function') {
    const raw = globalThis.atob(normalized);
    const out = new Uint8Array(raw.length);
    for (let i = 0; i < raw.length; i += 1) {
      out[i] = raw.charCodeAt(i);
    }
    return out;
  }

  const maybeBuffer = (globalThis as { Buffer?: { from(value: string, encoding?: string): { values(): Iterable<number> } } }).Buffer;
  if (maybeBuffer) {
    return Uint8Array.from(maybeBuffer.from(normalized, 'base64').values());
  }

  const padding = (4 - (normalized.length % 4 || 4)) % 4;
  const base64 = normalized + '='.repeat(padding);
  let bits = 0;
  let bitCount = 0;
  const output: number[] = [];

  for (const char of base64) {
    if (char === '=') {
      break;
    }

    const value = BASE64_CHARS.indexOf(char);
    if (value === -1) {
      throw new Error(`Invalid base64 character: ${char}`);
    }

    bits = (bits << 6) | value;
    bitCount += 6;

    if (bitCount >= 8) {
      bitCount -= 8;
      output.push((bits >> bitCount) & 0xff);
    }
  }

  return Uint8Array.from(output);
}

export function bytesToHex(bytes: Uint8Array): string {
  return Array.from(bytes)
    .map((value) => value.toString(16).padStart(2, '0'))
    .join('');
}

export function hexToBytes(hex: string): Uint8Array {
  const normalized = hex.trim().toLowerCase();
  if (normalized.length % 2 !== 0) {
    throw new Error('Invalid hex length');
  }

  if (!/^[0-9a-f]*$/.test(normalized)) {
    throw new Error('Invalid hex characters');
  }

  const out = new Uint8Array(normalized.length / 2);
  for (let i = 0; i < normalized.length; i += 2) {
    out[i / 2] = Number.parseInt(normalized.slice(i, i + 2), 16);
  }
  return out;
}
