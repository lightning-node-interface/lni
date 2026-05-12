import { bech32 } from '@scure/base';
import { describe, expect, it, vi } from 'vitest';
import { getPaymentInfo, resolveToBolt11 } from '../lnurl.js';
import type { FetchLike } from '../types.js';

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
    maxSendable: 100_000,
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
      }),
    ).rejects.toThrow('LNURL endpoints must use HTTPS.');

    expect(fetchMock).not.toHaveBeenCalled();
  });

  it('rejects private callback URLs returned by public LNURL endpoints', async () => {
    const fetchMock = vi.fn<FetchLike>(async () =>
      jsonResponse(lnurlPayResponse('https://127.0.0.1/callback')),
    );
    const lnurl = encodeLnurl('https://pay.example/lnurl');

    await expect(
      resolveToBolt11(lnurl, 1000, {
        fetch: fetchMock,
      }),
    ).rejects.toThrow('LNURL endpoints must use a public hostname.');

    expect(fetchMock).toHaveBeenCalledTimes(1);
    expect(fetchMock.mock.calls[0]?.[0]).toBe('https://pay.example/lnurl');
  });

  it('rejects Lightning Address localhost domains before fetching', async () => {
    const fetchMock = vi.fn<FetchLike>();

    await expect(
      getPaymentInfo('alice@localhost', undefined, {
        fetch: fetchMock,
      }),
    ).rejects.toThrow('LNURL endpoints must use a public hostname.');

    expect(fetchMock).not.toHaveBeenCalled();
  });

  it('allows unsafe local LNURL URLs only when explicitly opted in', async () => {
    const fetchMock = vi
      .fn<FetchLike>()
      .mockResolvedValueOnce(jsonResponse(lnurlPayResponse('http://127.0.0.1:8080/callback')))
      .mockResolvedValueOnce(jsonResponse({ pr: 'lnbc1dummyinvoice' }));
    const lnurl = encodeLnurl('http://127.0.0.1:8080/lnurl');

    await expect(
      resolveToBolt11(lnurl, 1000, {
        allowUnsafeUrls: true,
        fetch: fetchMock,
      }),
    ).resolves.toBe('lnbc1dummyinvoice');

    expect(fetchMock).toHaveBeenCalledTimes(2);
    expect(fetchMock.mock.calls[0]?.[0]).toBe('http://127.0.0.1:8080/lnurl');
    expect(fetchMock.mock.calls[1]?.[0]).toBe('http://127.0.0.1:8080/callback?amount=1000');
  });
});
