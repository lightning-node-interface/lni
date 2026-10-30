type ConnectRequest = {
  method?: string;
};

type ConnectResponse = {
  statusCode: number;
  setHeader(name: string, value: string): void;
  write(chunk: string): void;
  end(body?: string): void;
  on(event: 'close', listener: () => void): void;
};

type ConnectResult = {
  clientCredentials?: string | null;
  error?: string | null;
};

type LexeBinding = {
  LexeConnectSession?: new (options: {
    scopes: string[];
    requesterName: string;
    requestTtlSecs: number;
  }) => {
    connectionString(): string;
    pollMailbox(): Promise<ConnectResult | null | undefined>;
    cancel(): void;
  };
  LexeNode?: new (credentials: string, network: string) => {
    getHumanBitcoinAddress(): Promise<{
      lightningAddress: string;
      humanBitcoinAddress: string;
    }>;
    getNextUnusedAddress(): Promise<string>;
    close(): void;
  };
};

declare const process: {
  platform: string;
  arch: string;
  env: Record<string, string | undefined>;
};
declare function require(path: string): LexeBinding;

const REQUEST_TTL_SECONDS = 240;

function send(response: ConnectResponse, event: string, data: Record<string, unknown>) {
  response.write(`event: ${event}\ndata: ${JSON.stringify(data)}\n\n`);
}

function delay(ms: number) {
  return new Promise<void>((resolve) => setTimeout(resolve, ms));
}

export default async function handler(request: ConnectRequest, response: ConnectResponse) {
  response.setHeader('Cache-Control', 'no-store, no-transform');
  response.setHeader('X-Content-Type-Options', 'nosniff');
  response.setHeader('Referrer-Policy', 'no-referrer');

  if (request.method !== 'POST') {
    response.statusCode = 405;
    return response.end('POST required');
  }
  if (process.platform !== 'linux' || process.arch !== 'x64') {
    response.statusCode = 500;
    return response.end('Native runtime unavailable');
  }

  let binding: LexeBinding;
  try {
    binding = require('../native/lni_js.linux-x64-gnu.node') as LexeBinding;
  } catch {
    response.statusCode = 500;
    return response.end('Native addon unavailable');
  }
  if (!binding.LexeConnectSession || !binding.LexeNode) {
    response.statusCode = 500;
    return response.end('Lexe binding unavailable');
  }

  let session: InstanceType<NonNullable<LexeBinding['LexeConnectSession']>>;
  try {
    session = new binding.LexeConnectSession({
      scopes: ['read_info', 'receive'],
      requesterName: 'LNI Vercel demo',
      requestTtlSecs: REQUEST_TTL_SECONDS,
    });
  } catch {
    response.statusCode = 500;
    return response.end('Could not start LexeConnect');
  }

  response.setHeader('Content-Type', 'text/event-stream; charset=utf-8');
  response.statusCode = 200;
  let disconnected = false;
  response.on('close', () => {
    disconnected = true;
    session.cancel();
  });

  try {
    // The response remains open with this session. No key is serialized or
    // shared across Vercel invocations, and no credential is sent to the client.
    send(response, 'ready', { connectionString: session.connectionString() });
    const deadline = Date.now() + REQUEST_TTL_SECONDS * 1000;
    while (!disconnected && Date.now() < deadline) {
      const result = await session.pollMailbox();
      if (disconnected) break;
      if (result) {
        if (!result.clientCredentials) {
          send(response, 'rejected', { reason: result.error === 'user_rejected' ? 'rejected' : 'error' });
          break;
        }

        const node = new binding.LexeNode(
          result.clientCredentials,
          process.env.LEXE_NETWORK || 'mainnet',
        );
        try {
          const [human, bitcoinAddress] = await Promise.all([
            node.getHumanBitcoinAddress(),
            node.getNextUnusedAddress(),
          ]);
          if (!disconnected) {
            send(response, 'approved', {
              lightningAddress: human.lightningAddress || null,
              bitcoinAddress,
              humanBitcoinAddress: human.humanBitcoinAddress || null,
            });
          }
        } finally {
          node.close();
        }
        break;
      }
      response.write(': waiting\n\n');
      await delay(1500);
    }
    if (!disconnected && Date.now() >= deadline) {
      send(response, 'expired', {});
    }
  } catch {
    if (!disconnected) send(response, 'error', { message: 'LexeConnect request failed. Please try again.' });
  } finally {
    session.cancel();
    if (!disconnected) response.end();
  }
}
