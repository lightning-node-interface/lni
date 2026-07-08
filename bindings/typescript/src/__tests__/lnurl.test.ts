import { bech32 } from '@scure/base';
import { describe, expect, it, vi } from 'vitest';
import {
  getPaymentInfo,
  LnurlVerifyUnsupportedError,
  resolveToBolt11,
  verifyLightningAddressPayRequest,
} from '../lnurl.js';
import type { FetchLike } from '../types.js';

const BOLT11_250_000_000_MSATS =
  'lnbc2500u1pvjluezsp5zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zygspp5qqqsyqcyq5rqwzqfqqqsyqcyq5rqwzqfqqqsyqcyq5rqwzqfqypqdq5xysxxatsyp3k7enxv4jsxqzpu9qrsgquk0rl77nj30yxdy8j9vdx85fkpmdla2087ne0xh8nhedh8w27kyke0lp53ut353s06fv3qfegext0eh0ymjpf39tuven09sam30g4vgpfna3rh';

function encodeLnurl(url: string): string {
  const bytes = new TextEncoder().encode(url);
  return bech32.encode('lnurl', bech32.toWords(bytes), false);
}

function jsonResponse(body: unknown): Response {
  return new Response(JSON.stringify(body), {
    headers: {
      'content-type': 'application/json',
    },
  });
}

function lnurlPayResponse(callback: string) {
  return {
    callback,
    maxSendable: 500_000_000,
    minSendable: 1,
    metadata: '[["text/plain","test"]]',
    tag: 'payRequest',
  };
}

describe('LNURL SSRF protections', () => {
  it('rejects decoded LNURLs that target localhost before fetching', async () => {
    const fetchMock = vi.fn<FetchLike>();
    const lnurl = encodeLnurl('http://127.0.0.1:8080/lnurl');

    await expect(
      resolveToBolt11(lnurl, 1000, {
        fetch: fetchMock,
      })
    ).rejects.toThrow('LNURL endpoints must use HTTPS.');

    expect(fetchMock).not.toHaveBeenCalled();
  });

  it('rejects private callback URLs returned by public LNURL endpoints', async () => {
    const fetchMock = vi.fn<FetchLike>(async () =>
      jsonResponse(lnurlPayResponse('https://127.0.0.1/callback'))
    );
    const lnurl = encodeLnurl('https://pay.example/lnurl');

    await expect(
      resolveToBolt11(lnurl, 1000, {
        fetch: fetchMock,
      })
    ).rejects.toThrow('LNURL endpoints must use a public hostname.');

    expect(fetchMock).toHaveBeenCalledTimes(1);
    expect(fetchMock.mock.calls[0]?.[0]).toBe('https://pay.example/lnurl');
  });

  it('rejects Lightning Address localhost domains before fetching', async () => {
    const fetchMock = vi.fn<FetchLike>();

    await expect(
      getPaymentInfo('alice@localhost', undefined, {
        fetch: fetchMock,
      })
    ).rejects.toThrow('LNURL endpoints must use a public hostname.');

    expect(fetchMock).not.toHaveBeenCalled();
  });

  it('allows unsafe local LNURL URLs only when explicitly opted in', async () => {
    const fetchMock = vi
      .fn<FetchLike>()
      .mockResolvedValueOnce(jsonResponse(lnurlPayResponse('http://127.0.0.1:8080/callback')))
      .mockResolvedValueOnce(jsonResponse({ pr: BOLT11_250_000_000_MSATS }));
    const lnurl = encodeLnurl('http://127.0.0.1:8080/lnurl');

    await expect(
      resolveToBolt11(lnurl, 250_000_000, {
        allowUnsafeUrls: true,
        fetch: fetchMock,
      })
    ).resolves.toBe(BOLT11_250_000_000_MSATS);

    expect(fetchMock).toHaveBeenCalledTimes(2);
    expect(fetchMock.mock.calls[0]?.[0]).toBe('http://127.0.0.1:8080/lnurl');
    expect(fetchMock.mock.calls[1]?.[0]).toBe('http://127.0.0.1:8080/callback?amount=250000000');
  });

  it('rejects callback invoices with an amount that does not match the request', async () => {
    const fetchMock = vi
      .fn<FetchLike>()
      .mockResolvedValueOnce(jsonResponse(lnurlPayResponse('https://pay.example/callback')))
      .mockResolvedValueOnce(jsonResponse({ pr: BOLT11_250_000_000_MSATS }));
    const lnurl = encodeLnurl('https://pay.example/lnurl');

    await expect(
      resolveToBolt11(lnurl, 1000, {
        fetch: fetchMock,
      })
    ).rejects.toThrow('does not match requested amount 1000 msats');

    expect(fetchMock).toHaveBeenCalledTimes(2);
  });

  it('treats empty LNURL verify fields as unsupported verify', async () => {
    const fetchMock = vi
      .fn<FetchLike>()
      .mockResolvedValueOnce(jsonResponse(lnurlPayResponse('https://pay.example/callback')))
      .mockResolvedValueOnce(jsonResponse({ pr: '', verify: '' }));

    await expect(
      verifyLightningAddressPayRequest('alice@example.com', {
        fetch: fetchMock,
      })
    ).rejects.toBeInstanceOf(LnurlVerifyUnsupportedError);

    expect(fetchMock).toHaveBeenCalledTimes(2);
    expect(fetchMock.mock.calls[0]?.[0]).toBe('https://example.com/.well-known/lnurlp/alice');
    expect(fetchMock.mock.calls[1]?.[0]).toBe('https://pay.example/callback?amount=100000');
  });
});
