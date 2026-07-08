import http from 'node:http';
import { bech32 } from '@scure/base';
import { describe, expect, it } from 'vitest';
import { getPaymentInfo, resolveToBolt11 } from '../../lnurl.js';
import { itIf, timeout } from './helpers.js';

const realLnurlAddress = process.env.LNI_REAL_LNURL_ADDRESS?.trim() || 'bluerobin15@primal.net';
const runRealLnurlTest =
  process.env.LNI_REAL_LNURL === '1' || Boolean(process.env.LNI_REAL_LNURL_ADDRESS?.trim());
const BOLT11_250_000_000_MSATS =
  'lnbc2500u1pvjluezsp5zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zygspp5qqqsyqcyq5rqwzqfqqqsyqcyq5rqwzqfqqqsyqcyq5rqwzqfqypqdq5xysxxatsyp3k7enxv4jsxqzpu9qrsgquk0rl77nj30yxdy8j9vdx85fkpmdla2087ne0xh8nhedh8w27kyke0lp53ut353s06fv3qfegext0eh0ymjpf39tuven09sam30g4vgpfna3rh';

function encodeLnurl(url: string): string {
  const bytes = new TextEncoder().encode(url);
  return bech32.encode('lnurl', bech32.toWords(bytes), false);
}

function listen(server: http.Server): Promise<number> {
  return new Promise((resolve, reject) => {
    const onError = (error: Error) => {
      reject(error);
    };

    server.once('error', onError);
    server.listen(0, '127.0.0.1', () => {
      server.off('error', onError);
      const address = server.address();
      if (!address || typeof address === 'string') {
        reject(new Error('Expected local HTTP server to listen on a TCP port.'));
        return;
      }
      resolve(address.port);
    });
  });
}

function close(server: http.Server): Promise<void> {
  return new Promise((resolve, reject) => {
    server.close((error) => {
      if (error) {
        reject(error);
        return;
      }
      resolve();
    });
  });
}

function jsonResponse(response: http.ServerResponse, body: unknown): void {
  response.writeHead(200, {
    'content-type': 'application/json',
  });
  response.end(JSON.stringify(body));
}

function lnurlPayResponse(callback: string) {
  return {
    callback,
    maxSendable: 500_000_000,
    minSendable: 1,
    metadata: '[["text/plain","real lnurl test"]]',
    tag: 'payRequest',
  };
}

describe('real LNURL SSRF protections', () => {
  it(
    'blocks local decoded LNURLs before making a network request',
    async () => {
      let hits = 0;
      const server = http.createServer((_request, response) => {
        hits += 1;
        jsonResponse(response, lnurlPayResponse('http://127.0.0.1/callback'));
      });
      const port = await listen(server);

      try {
        const lnurl = encodeLnurl(`http://127.0.0.1:${port}/lnurl`);

        await expect(resolveToBolt11(lnurl, 1000)).rejects.toThrow(
          'LNURL endpoints must use HTTPS.'
        );
        expect(hits).toBe(0);
      } finally {
        await close(server);
      }
    },
    timeout
  );

  it(
    'can still resolve a local LNURL when unsafe URLs are explicitly enabled',
    async () => {
      const requests: string[] = [];
      let port = 0;
      const server = http.createServer((request, response) => {
        requests.push(request.url ?? '');

        if (request.url === '/lnurl') {
          jsonResponse(response, lnurlPayResponse(`http://127.0.0.1:${port}/callback`));
          return;
        }

        if (request.url === '/callback?amount=250000000') {
          jsonResponse(response, { pr: BOLT11_250_000_000_MSATS });
          return;
        }

        response.writeHead(404);
        response.end();
      });
      port = await listen(server);

      try {
        const lnurl = encodeLnurl(`http://127.0.0.1:${port}/lnurl`);

        await expect(
          resolveToBolt11(lnurl, 250_000_000, {
            allowUnsafeUrls: true,
          })
        ).resolves.toBe(BOLT11_250_000_000_MSATS);
        expect(requests).toEqual(['/lnurl', '/callback?amount=250000000']);
      } finally {
        await close(server);
      }
    },
    timeout
  );
});

describe('real LNURL resolution', () => {
  itIf(runRealLnurlTest)(
    `resolves ${realLnurlAddress}`,
    async () => {
      const info = await getPaymentInfo(realLnurlAddress);
      expect(info.destinationType).toBe('lightning_address');
      expect(info.minSendableMsats).toBeGreaterThan(0);
      expect(info.maxSendableMsats).toBeGreaterThanOrEqual(info.minSendableMsats ?? 0);

      const amountMsats = info.minSendableMsats ?? 1000;
      const invoice = await resolveToBolt11(realLnurlAddress, amountMsats);

      expect(invoice.toLowerCase()).toMatch(/^ln(bc|tb|bcrt)/);
    },
    timeout
  );
});
