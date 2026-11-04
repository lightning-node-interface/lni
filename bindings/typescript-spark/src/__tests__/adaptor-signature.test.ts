import { describe, expect, it } from 'vitest';
import { schnorr, secp256k1 } from '@noble/curves/secp256k1.js';
import { validateOutboundAdaptorSignatureLocal as validate } from '../internal/adaptor-signature.js';

const order = secp256k1.Point.Fn.ORDER;
const scalarBytes = (value: bigint): Uint8Array =>
  Uint8Array.from(value.toString(16).padStart(64, '0').match(/../g)!, (byte) => parseInt(byte, 16));
const number = (bytes: Uint8Array): bigint =>
  BigInt('0x' + Array.from(bytes, (byte) => byte.toString(16).padStart(2, '0')).join(''));

// Synthetic keys only. Complete each adaptor signature and independently verify
// the resulting BIP340 signature, so a changed curve API cannot silently pass.
function fixture() {
  const secret = 1n;
  const adaptor = 3n;
  const pubkey = schnorr.getPublicKey(scalarBytes(secret));
  const adaptorPubkey = secp256k1.getPublicKey(scalarBytes(adaptor));
  const message = new Uint8Array(32).fill(42);
  let nonce = 2n;
  let adaptedR = secp256k1.Point.BASE.multiply(nonce + adaptor);
  while (adaptedR.toAffine().y % 2n !== 0n) {
    nonce++;
    adaptedR = secp256k1.Point.BASE.multiply(nonce + adaptor);
  }
  const r = scalarBytes(adaptedR.toAffine().x);
  const challenge = number(schnorr.utils.taggedHash('BIP0340/challenge', r, pubkey, message)) % order;
  const s = (nonce + challenge * secret) % order;
  const signature = new Uint8Array([...r, ...scalarBytes(s)]);
  const completed = new Uint8Array([...r, ...scalarBytes((s + adaptor) % order)]);
  return { signature, message, pubkey, adaptorPubkey, completed };
}

describe('outbound adaptor signature curve compatibility', () => {
  it('accepts an adaptor signature that completes to a valid BIP340 signature', () => {
    const vector = fixture();
    expect(schnorr.verify(vector.completed, vector.message, vector.pubkey)).toBe(true);
    expect(schnorr.verify(vector.signature, vector.message, vector.pubkey)).toBe(false);
    expect(validate(vector)).toEqual({ ok: true });
  });

  it('accepts the equivalent compressed signer public key', () => {
    const vector = fixture();
    expect(validate({ ...vector, pubkey: new Uint8Array([2, ...vector.pubkey]) })).toEqual({ ok: true });
  });

  it('rejects a changed message, scalar, or adaptor key', () => {
    const vector = fixture();
    const message = vector.message.slice();
    message[0] = message[0]! ^ 1;
    const signature = vector.signature.slice();
    signature[63] = signature[63]! ^ 1;
    expect(validate({ ...vector, message }).ok).toBe(false);
    expect(validate({ ...vector, signature }).ok).toBe(false);
    expect(validate({ ...vector, adaptorPubkey: secp256k1.getPublicKey(scalarBytes(4n)) }).ok).toBe(false);
  });

  it('rejects noncanonical signature scalars at the field and group boundaries', () => {
    const vector = fixture();
    expect(validate({ ...vector, signature: new Uint8Array([...scalarBytes(secp256k1.Point.Fp.ORDER), ...vector.signature.slice(32)]) })).toMatchObject({ ok: false, reason: 'invalid signature r >= field order' });
    expect(validate({ ...vector, signature: new Uint8Array([...vector.signature.slice(0, 32), ...scalarBytes(order)]) })).toMatchObject({ ok: false, reason: 'invalid signature s >= curve order' });
  });

  it('rejects invalid input lengths and invalid curve points without throwing', () => {
    const vector = fixture();
    expect(validate({ ...vector, message: new Uint8Array(31) }).ok).toBe(false);
    expect(validate({ ...vector, signature: new Uint8Array(63) }).ok).toBe(false);
    expect(validate({ ...vector, pubkey: new Uint8Array(31) }).ok).toBe(false);
    expect(validate({ ...vector, adaptorPubkey: new Uint8Array(33) }).ok).toBe(false);
    expect(validate({ ...vector, pubkey: scalarBytes(secp256k1.Point.Fp.ORDER) }).ok).toBe(false);
  });
});
