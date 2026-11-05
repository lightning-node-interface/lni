import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
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
const recurringOfferFixture = JSON.parse(readFileSync(
  new URL('../../../../crates/lni/testdata/bolt12-recurring-offer.json', import.meta.url), 'utf8'
));

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

  // Payload layout: BOLT #1240 revision 941b20d and Core Lightning's
  // wire/bolt12_wire.csv (experimental types add 1_000_000_000).
  it.each([0, 1_000_000_000])('decodes compulsory recurrence using type offset %i', (offset) => {
    const offer = makeRecurrenceOffer([
      [offset + 24, [2, 3]], // every three months
      [offset + 26, [1, 0x65, 0x92, 0, 0x80]], // proportional, 1704067200
      [offset + 27, [0, 0, 0x0e, 0x10, 0x1c, 0x20]], // 3600 before, 7200 after
      [offset + 29, [12]],
    ]);
    const decoded = JSON.parse(decodeOfferToJson(offer));
    expect(decoded.recurrence).toEqual({
      timeUnit: 2, timeUnitName: 'months', period: 3, compulsoryField: true,
      baseTime: '1704067200', proportionalAmount: true,
      payWindow: { secondsBefore: 3600, secondsAfter: 7200 }, limit: 12, maxPayments: 13,
    });
    expect(decoded.sections.find((s: { name: string }) => s.name === 'recurrence_base'))
      .toMatchObject({ type: offset + 26, raw: '0165920080' });
  });

  it.each([0, 1_000_000_000])('decodes optional recurrence using type offset %i', (offset) => {
    expect(decodeOffer(makeRecurrenceOffer([[offset + 25, [1, 7]]])).recurrence).toEqual({
      timeUnit: 1, timeUnitName: 'days', period: 7, compulsoryField: false,
    });
  });

  it('preserves full u64 recurrence base time and u32 period precision', () => {
    const recurrence = decodeOffer(makeRecurrenceOffer([
      [24, [0, 255, 255, 255, 255]],
      [26, [0, ...new Array(8).fill(255)]],
      [27, [255, 255, 255, 255]], // fixed u32 before; truncated zero after
    ])).recurrence;
    expect(recurrence).toMatchObject({ period: 4294967295, baseTime: '18446744073709551615',
      proportionalAmount: false, payWindow: { secondsBefore: 4294967295, secondsAfter: 0 } });
  });

  it('preserves unknown fields alongside recurrence', () => {
    const decoded = decodeOffer(makeRecurrenceOffer([[25, [0, 1]], [31, [0xab]]]));
    expect(decoded.sections).toContainEqual({ name: 'unknown', type: 31, value: 'ab' });
  });

  it('decodes blinded paths and recurrence together without changing path output', () => {
    const original = decodeOffer(BOLT12_OFFER_WITH_PATH);
    const pathBytes = original.pathsRaw!.match(/../g)!.map((byte) => parseInt(byte, 16));
    const decoded = decodeOffer(makeRecurrenceOffer([[16, pathBytes], [25, [0, 60]]]));
    expect(decoded.paths).toEqual(original.paths);
    expect(decoded.recurrence).toMatchObject({ period: 60, timeUnitName: 'seconds' });
  });

  it('keeps recurrence absent on ordinary offers', () => {
    expect(decodeOffer(BOLT12_OFFER).recurrence).toBeUndefined();
  });

  it('decodes the supplied monthly offer including its blinded route and twelve-payment limit', () => {
    const decoded = decodeOffer(recurringOfferFixture.offer);
    expect(decoded.description).toBe(recurringOfferFixture.description);
    expect(decoded.amount).toBe(recurringOfferFixture.amount_msat);
    expect(decoded.amountMsats).toBe(5000);
    expect(decoded.chains).toEqual(recurringOfferFixture.chains);
    expect(decoded.issuerSigningPubkey).toBe(recurringOfferFixture.issuer_signing_pubkey);
    expect(decoded.rawTlvHex).toBe(recurringOfferFixture.raw_tlv_hex);
    expect(decoded.paths).toHaveLength(recurringOfferFixture.blinded_path_count);
    expect(decoded.paths?.[0]?.introductionNode).toEqual({
      type: 'node_id', nodeId: '039fdcef926ccf2f7f3a48e9e9542217c727a3c31e389d08b742848132ae5e0c5a',
    });
    expect(decoded.paths?.[0]?.blindingPoint).toBe(
      '02a4f10ffa7b2fd8b8c5e9595de8396ebbb5e884a2443429bfd668e67f86b015cb'
    );
    expect(decoded.paths?.[0]?.blindedHops).toHaveLength(1);
    expect(decoded.paths?.[0]?.blindedHops[0]?.encryptedPayload).toHaveLength(88);
    expect(decoded.recurrence).toEqual({
      timeUnit: 2, timeUnitName: 'months', period: 1, compulsoryField: false,
      limit: 11, maxPayments: 12,
    });
    expect(decoded.absoluteExpiry).toBeUndefined();
    expect(decoded.metadata).toBeUndefined();
    expect(decoded.issuer).toBeUndefined();
  });

  it.each([
    ['missing time unit', [[24, []]]],
    ['unknown time unit', [[24, [3, 1]]]],
    ['zero period', [[24, [0]]]],
    ['non-canonical period', [[24, [0, 0, 1]]]],
    ['oversized period', [[24, [0, 1, 0, 0, 0, 0]]]],
    ['both variants', [[24, [0, 1]], [25, [0, 1]]]],
    ['mixed encodings', [[24, [0, 1]], [1_000_000_029, [1]]]],
    ['orphan base', [[26, [0, 1]]]],
    ['orphan window', [[27, [0, 0, 0, 0]]]],
    ['orphan limit', [[29, [1]]]],
    ['optional base', [[25, [0, 1]], [26, [0, 1]]]],
    ['missing base flag', [[24, [0, 1]], [26, []]]],
    ['invalid base flag', [[24, [0, 1]], [26, [2, 1]]]],
    ['non-canonical base', [[24, [0, 1]], [26, [0, 0, 1]]]],
    ['oversized base', [[24, [0, 1]], [26, [0, ...new Array(9).fill(1)]]]],
    ['truncated window', [[24, [0, 1]], [27, [0, 0, 0]]]],
    ['non-canonical window', [[24, [0, 1]], [27, [0, 0, 0, 0, 0]]]],
    ['oversized window', [[24, [0, 1]], [27, new Array(9).fill(1)]]],
    ['zero limit', [[24, [0, 1]], [29, []]]],
    ['non-canonical limit', [[24, [0, 1]], [29, [0, 1]]]],
    ['oversized limit', [[24, [0, 1]], [29, new Array(5).fill(1)]]],
  ] as Array<[string, Array<[number, number[]]>]>)('rejects %s', (_name, fields) => {
    expect(() => decodeOffer(makeRecurrenceOffer(fields))).toThrow(/Invalid BOLT12 recurrence/);
  });
});

function makeRecurrenceOffer(fields: Array<[number, number[]]>): string {
  // The issuer key from the upstream minimal-offer vector.
  const issuer = '02eec7245d6b7d2ccb30380bfbe2a3648cd7a942653f5aa340edcea1f283686619';
  const records: Array<[number, number[]]> = [
    [22, [...Uint8Array.from(issuer.match(/../g)!.map((byte) => parseInt(byte, 16)))]],
    ...fields,
  ];
  const bytes = records.sort(([a], [b]) => a - b).flatMap(([type, value]) => [
    ...encodeBigSize(type), ...encodeBigSize(value.length), ...value,
  ]);
  return `lno1${bytesToBech32Words(bytes)}`;
}

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
  if (value <= 0xffffffff) {
    return [0xfe, (value >>> 24) & 0xff, (value >>> 16) & 0xff, (value >>> 8) & 0xff, value & 0xff];
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
