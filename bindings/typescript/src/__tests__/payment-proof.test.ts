import { describe, expect, it } from 'vitest';
import { validPaymentPreimageForHash } from '../internal/payment-proof.js';

const ZERO_PREIMAGE = '0000000000000000000000000000000000000000000000000000000000000000';
const ZERO_PREIMAGE_PAYMENT_HASH =
  '66687aadf862bd776c8fc18b8e9f8e20089714856ee233b3902a591d0d5f2925';

describe('payment proof validation', () => {
  it('accepts a 32-byte hex preimage only when it hashes to the invoice payment hash', async () => {
    await expect(validPaymentPreimageForHash(ZERO_PREIMAGE, ZERO_PREIMAGE_PAYMENT_HASH)).resolves.toBe(
      ZERO_PREIMAGE
    );
  });

  it('rejects forged, malformed, or wrong-length preimages', async () => {
    await expect(validPaymentPreimageForHash('ff'.repeat(32), ZERO_PREIMAGE_PAYMENT_HASH)).resolves.toBe(
      ''
    );
    await expect(validPaymentPreimageForHash('not-hex', ZERO_PREIMAGE_PAYMENT_HASH)).resolves.toBe('');
    await expect(validPaymentPreimageForHash('00', ZERO_PREIMAGE_PAYMENT_HASH)).resolves.toBe('');
  });
});
