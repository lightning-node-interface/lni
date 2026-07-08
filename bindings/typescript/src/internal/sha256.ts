import { LniError } from '../errors.js';
import { bytesToHex } from './encoding.js';

type Sha256DigestFallback = (bytes: Uint8Array) => Promise<ArrayBuffer | Uint8Array>;

let sha256DigestFallback: Sha256DigestFallback | undefined;

export function registerSha256DigestFallback(fallback: Sha256DigestFallback | undefined): void {
  sha256DigestFallback = fallback;
}

export async function sha256Hex(bytes: Uint8Array): Promise<string> {
  if (globalThis.crypto?.subtle) {
    const digest = await globalThis.crypto.subtle.digest('SHA-256', bytes as BufferSource);
    return bytesToHex(new Uint8Array(digest));
  }

  if (sha256DigestFallback) {
    const digest = await sha256DigestFallback(bytes);
    return bytesToHex(new Uint8Array(digest));
  }

  throw new LniError(
    'Api',
    'Web Crypto API or a registered SHA-256 digest fallback is required to hash NWC preimages.'
  );
}
