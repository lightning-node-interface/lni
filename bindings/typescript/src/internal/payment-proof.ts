import { hexToBytes } from './encoding.js';
import { sha256Hex } from './sha256.js';

const PREIMAGE_BYTE_LENGTH = 32;

export async function validPaymentPreimageForHash(
  preimage: string | undefined,
  paymentHash: string
): Promise<string> {
  if (!preimage || !paymentHash) {
    return '';
  }

  let preimageBytes: Uint8Array;
  try {
    preimageBytes = hexToBytes(preimage);
  } catch {
    return '';
  }

  if (preimageBytes.length !== PREIMAGE_BYTE_LENGTH) {
    return '';
  }

  const derivedPaymentHash = await sha256Hex(preimageBytes);
  return derivedPaymentHash === paymentHash.trim().toLowerCase() ? preimage : '';
}
