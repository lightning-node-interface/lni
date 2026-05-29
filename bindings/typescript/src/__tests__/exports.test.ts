import { describe, expect, it } from 'vitest';
import { decode, decodeOffer } from '../index.js';

describe('public exports', () => {
  it('exports BOLT11 decode from the package root', () => {
    expect(typeof decode).toBe('function');
  });

  it('exports BOLT12 offer decode from the package root', () => {
    expect(typeof decodeOffer).toBe('function');
  });
});
