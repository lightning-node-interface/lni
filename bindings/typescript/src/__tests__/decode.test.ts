import { describe, expect, it } from 'vitest';
import { decode, decodeOffer, decodeOfferToJson } from '../decode.js';

const BOLT11 =
  'lnbc2500u1pvjluezsp5zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zygspp5qqqsyqcyq5rqwzqfqqqsyqcyq5rqwzqfqqqsyqcyq5rqwzqfqypqdq5xysxxatsyp3k7enxv4jsxqzpu9qrsgquk0rl77nj30yxdy8j9vdx85fkpmdla2087ne0xh8nhedh8w27kyke0lp53ut353s06fv3qfegext0eh0ymjpf39tuven09sam30g4vgpfna3rh';

const BOLT12_OFFER =
  'lno1pgx9getnwss8vetrw3hhyuckyypwa3eyt44h6txtxquqh7lz5djge4afgfjn7k4rgrkuag0jsd5xvxg';

const BOLT12_OFFER_WITH_BITCOIN_AMOUNT =
  'lno1pqpzwyq2p32x2um5ypmx2cm5dae8x93pqthvwfzadd7jejes8q9lhc4rvjxd022zv5l44g6qah82ru5rdpnpj';

const BOLT12_OFFER_WITH_CURRENCY_AMOUNT =
  'lno1qcp4256ypqpzwyq2p32x2um5ypmx2cm5dae8x93pqthvwfzadd7jejes8q9lhc4rvjxd022zv5l44g6qah82ru5rdpnpj';

const BOLT12_OFFER_WITH_PATH =
  'lno1pgx9getnwss8vetrw3hhyucs5ypjgef743p5fzqq9nqxh0ah7y87rzv3ud0eleps9kl2d5348hq2k8qzqgpqyqszqgpqyqszqgpqyqszqgpqyqszqgpqyqszqgpqyqszqgpqyqszqgpqyqszqgpqyqszqgpqyqszqgpqyqszqgpqyqszqgpqyqszqgqpqqqqqqqqqqqqqqqqqqqqqqqqqqqzqgpqyqszqgpqyqszqgpqyqszqgpqyqszqgpqyqszqgpqyqszqgpqqzq3zyg3zyg3zyg3vggzamrjghtt05kvkvpcp0a79gmy3nt6jsn98ad2xs8de6sl9qmgvcvs';

const BECH32_CHARSET = 'qpzry9x8gf2tvdw0s3jn54khce6mua7l';

describe('decode helpers', () => {
  it('decodes BOLT11 invoices into keyed fields', () => {
    const decoded = decode(BOLT11);
    expect(decoded.paymentRequest).toBe(BOLT11);
    expect(decoded.type).toBe('bolt11_invoice');
    expect(decoded.amount).toBe('250000000');
    expect(decoded.amountMsats).toBe(250000000);
    expect(decoded.payment_hash).toBe(
      '0001020304050607080900010203040506070809000102030405060708090102'
    );
    expect(decoded.description).toBe('1 cup coffee');
    expect(decoded.expiry).toBe(60);
    expect(decoded.expiresAt).toBe(1496314718);
    expect('sections' in decoded).toBe(false);
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

  it('only sets amountMsats for bitcoin-denominated BOLT12 offer amounts', () => {
    const bitcoinOffer = decodeOffer(BOLT12_OFFER_WITH_BITCOIN_AMOUNT);
    expect(bitcoinOffer.currency).toBeUndefined();
    expect(bitcoinOffer.amount).toBe('10000');
    expect(bitcoinOffer.amountMsats).toBe(10000);

    const currencyOffer = decodeOffer(BOLT12_OFFER_WITH_CURRENCY_AMOUNT);
    expect(currencyOffer.currency).toBe('USD');
    expect(currencyOffer.amount).toBe('10000');
    expect(currencyOffer.amountMsats).toBeUndefined();
  });

  it('normalizes BOLT12 blinded paths', () => {
    const decoded = decodeOffer(BOLT12_OFFER_WITH_PATH);
    expect(decoded.paths?.length).toBeGreaterThan(0);
    expect(decoded.pathsRaw).toBeTruthy();
    expect(decoded.paths?.[0]?.introductionNode.type).toBe('node_id');
    expect(decoded.paths?.[0]?.blindingPoint).toMatch(/^02[0-9a-f]+$/);
    expect(decoded.paths?.[0]?.blindedHops.length).toBeGreaterThan(0);
    expect(decoded.paths?.[0]?.blindedHops[0]?.encryptedPayload).toMatch(/^[0-9a-f]+$/);
    expect(decoded.sections.find((section) => section.name === 'paths')?.value).toEqual(
      decoded.paths
    );
  });

  it('rejects BOLT12 blinded paths with too many hops', () => {
    expect(() => decodeOffer(makeOfferWithBlindedPathHopCount(21))).toThrow(/too many hops/);
  });

  it('rejects BOLT12 blinded path payload lengths above the safe integer range', () => {
    expect(() => decodeOffer(makeOfferWithOversizedPayloadLength())).toThrow(
      /collection length exceeds safe integer range/
    );
  });
});

function makeOfferWithBlindedPathHopCount(numHops: number): string {
  const path = [
    2,
    ...new Array(32).fill(0),
    2,
    ...new Array(32).fill(1),
    numHops,
    ...Array.from({ length: numHops }, () => [2, ...new Array(32).fill(2), 0, 0]).flat(),
  ];
  const tlv = [16, ...encodeBigSize(path.length), ...path];
  return `lno1${bytesToBech32Words(tlv)}`;
}

function makeOfferWithOversizedPayloadLength(): string {
  const path = [
    2,
    ...new Array(32).fill(0),
    2,
    ...new Array(32).fill(1),
    1,
    2,
    ...new Array(32).fill(2),
    0xff,
    0xff,
    ...new Array(8).fill(0xff),
  ];
  const tlv = [16, ...encodeBigSize(path.length), ...path];
  return `lno1${bytesToBech32Words(tlv)}`;
}

function encodeBigSize(value: number): number[] {
  if (value < 0xfd) {
    return [value];
  }
  if (value <= 0xffff) {
    return [0xfd, value >> 8, value & 0xff];
  }
  throw new Error('Test BigSize value is too large.');
}

function bytesToBech32Words(bytes: number[]): string {
  let acc = 0;
  let bits = 0;
  const words: number[] = [];

  for (const byte of bytes) {
    acc = (acc << 8) | byte;
    bits += 8;
    while (bits >= 5) {
      bits -= 5;
      words.push((acc >> bits) & 31);
    }
  }

  if (bits > 0) {
    words.push((acc << (5 - bits)) & 31);
  }

  return words.map((word) => BECH32_CHARSET[word]).join('');
}
