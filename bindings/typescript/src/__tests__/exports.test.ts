import { describe, expect, it } from 'vitest';
import { decode, decodeOffer, FeeError } from '../index.js';

describe('public exports', () => {
  it('exports BOLT11 decode from the package root', () => {
    expect(typeof decode).toBe('function');
  });

  it('exports BOLT12 offer decode from the package root', () => {
    expect(typeof decodeOffer).toBe('function');
  });

  it('exports FeeError from the package root', () => {
    const error = new FeeError('quoted fee exceeded the configured limit');

    expect(error).toMatchObject({
      name: 'FeeError',
      code: 'FeeError',
      message: 'quoted fee exceeded the configured limit',
    });
  });
});
