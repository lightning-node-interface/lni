import { describe, expect, it } from 'vitest';
import { decode, decodeOffer, decodeOfferToJson } from '../decode.js';

const BOLT11 =
  'lnbc2500u1pvjluezsp5zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zygspp5qqqsyqcyq5rqwzqfqqqsyqcyq5rqwzqfqqqsyqcyq5rqwzqfqypqdq5xysxxatsyp3k7enxv4jsxqzpu9qrsgquk0rl77nj30yxdy8j9vdx85fkpmdla2087ne0xh8nhedh8w27kyke0lp53ut353s06fv3qfegext0eh0ymjpf39tuven09sam30g4vgpfna3rh';

const BOLT12_OFFER =
  'lno1pgx9getnwss8vetrw3hhyuckyypwa3eyt44h6txtxquqh7lz5djge4afgfjn7k4rgrkuag0jsd5xvxg';

const BOLT12_OFFER_WITH_PATH =
  'lno1pgx9getnwss8vetrw3hhyucs5ypjgef743p5fzqq9nqxh0ah7y87rzv3ud0eleps9kl2d5348hq2k8qzqgpqyqszqgpqyqszqgpqyqszqgpqyqszqgpqyqszqgpqyqszqgpqyqszqgpqyqszqgpqyqszqgpqyqszqgpqyqszqgpqyqszqgpqyqszqgqpqqqqqqqqqqqqqqqqqqqqqqqqqqqzqgpqyqszqgpqyqszqgpqyqszqgpqyqszqgpqyqszqgpqyqszqgpqqzq3zyg3zyg3zyg3vggzamrjghtt05kvkvpcp0a79gmy3nt6jsn98ad2xs8de6sl9qmgvcvs';

describe('decode helpers', () => {
  it('decodes BOLT11 invoices through light-bolt11-decoder', () => {
    const decoded = decode(BOLT11);
    expect(decoded.paymentRequest).toBe(BOLT11);
    expect(decoded.sections.some((section) => section.name === 'payment_hash')).toBe(true);
  });

  it('decodes BOLT12 offers separately', () => {
    const decoded = decodeOffer(BOLT12_OFFER);
    expect(decoded.type).toBe('bolt12_offer');
    expect(decoded.prefix).toBe('lno');
    expect(decoded.issuerSigningPubkey).toBeTruthy();
    expect(decoded.sections.some((section) => section.name === 'issuer_id')).toBe(true);
  });

  it('serializes decoded BOLT12 offers for node adapter methods', () => {
    const decoded = JSON.parse(decodeOfferToJson(BOLT12_OFFER)) as ReturnType<typeof decodeOffer>;
    expect(decoded.offer).toBe(BOLT12_OFFER);
  });

  it('normalizes BOLT12 blinded paths', () => {
    const decoded = decodeOffer(BOLT12_OFFER_WITH_PATH);
    expect(decoded.paths?.length).toBeGreaterThan(0);
    expect(decoded.pathsRaw).toBeTruthy();
    expect(decoded.paths?.[0]?.introductionNode.type).toBe('node_id');
    expect(decoded.paths?.[0]?.blindingPoint).toMatch(/^02[0-9a-f]+$/);
    expect(decoded.paths?.[0]?.blindedHops.length).toBeGreaterThan(0);
    expect(decoded.paths?.[0]?.blindedHops[0]?.encryptedPayload).toMatch(/^[0-9a-f]+$/);
    expect(decoded.sections.find((section) => section.name === 'paths')?.value).toEqual(decoded.paths);
  });
});
