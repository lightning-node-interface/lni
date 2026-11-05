import { schnorr, secp256k1 } from '@noble/curves/secp256k1.js';
import { LniError } from '@sunnyln/lni';

function bytesToBigInt(bytes: Uint8Array): bigint {
  let value = 0n;
  for (const byte of bytes) {
    value = (value << 8n) | BigInt(byte);
  }
  return value;
}

function toXOnlyPublicKey(pubkey: Uint8Array): Uint8Array {
  if (pubkey.length === 32) {
    return pubkey;
  }

  if (pubkey.length === 33 && (pubkey[0] === 0x02 || pubkey[0] === 0x03)) {
    return pubkey.slice(1);
  }

  throw new LniError(
    'InvalidInput',
    `Spark public key must be 32-byte x-only or 33-byte compressed, got ${pubkey.length} bytes.`
  );
}

export function validateOutboundAdaptorSignatureLocal(params: {
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
    if (rNum >= secp256k1.Point.Fp.ORDER) {
      return { ok: false, reason: 'invalid signature r >= field order' };
    }
    if (sNum >= secp256k1.Point.Fn.ORDER) {
      return { ok: false, reason: 'invalid signature s >= curve order' };
    }

    const xOnlyPubkey = toXOnlyPublicKey(pubkey);
    const signerPoint = schnorr.utils.lift_x(bytesToBigInt(xOnlyPubkey));
    signerPoint.assertValidity();

    const challengeBytes = schnorr.utils.taggedHash(
      'BIP0340/challenge',
      r,
      signerPoint.toBytes().slice(1),
      message
    );
    const challengeScalar = bytesToBigInt(challengeBytes) % secp256k1.Point.Fn.ORDER;
    const negChallenge = (secp256k1.Point.Fn.ORDER - challengeScalar) % secp256k1.Point.Fn.ORDER;

    const sG = secp256k1.Point.BASE.multiplyUnsafe(sNum);
    const eP = signerPoint.multiplyUnsafe(negChallenge);
    const baseR = sG.add(eP);
    if (baseR.equals(secp256k1.Point.ZERO)) {
      return { ok: false, reason: 'calculated base R is zero' };
    }
    baseR.assertValidity();

    const adaptorPoint = secp256k1.Point.fromBytes(adaptorPubkey);
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
    const message = error instanceof Error ? error.message : String(error);
    return { ok: false, reason: message.length > 200 ? `${message.slice(0, 200)}...` : message };
  }
}
