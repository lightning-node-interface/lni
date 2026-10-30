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

type HumanAddresses = {
  lightningAddress: string;
  humanBitcoinAddress: string;
};

type QrCode = {
  toDataURL(text: string, options: {
    errorCorrectionLevel: 'M';
    margin: number;
    width: number;
  }): Promise<string>;
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
    getHumanBitcoinAddress(): Promise<HumanAddresses>;
    getNextUnusedAddress(): Promise<string>;
    close(): void;
  };
};

declare const process: {
  platform: string;
  arch: string;
  env: Record<string, string | undefined>;
};
declare function require(path: string): unknown;

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

  let qrCode: QrCode;
  try {
    qrCode = require('qrcode') as QrCode;
  } catch {
    response.statusCode = 500;
    return response.end('QR renderer unavailable');
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

  let connectionString: string;
  let qrDataUrl: string;
  try {
    connectionString = session.connectionString();
    qrDataUrl = await qrCode.toDataURL(connectionString, {
      errorCorrectionLevel: 'M',
      margin: 2,
      width: 320,
    });
  } catch {
    session.cancel();
    response.statusCode = 500;
    return response.end('Could not render LexeConnect QR');
  }

  response.setHeader('Content-Type', 'text/event-stream; charset=utf-8');
  response.statusCode = 200;
  let disconnected = false;
  let finished = false;
  let stage = 'mailbox';
  response.on('close', () => {
    disconnected = true;
    session.cancel();
    if (!finished) console.info('lexe_connect: client_disconnected');
  });

  try {
    // The response remains open with this session. No key is serialized or
    // shared across Vercel invocations, and no credential is sent to the client.
    send(response, 'ready', { connectionString, qrDataUrl });
    console.info('lexe_connect: ready');
    const deadline = Date.now() + REQUEST_TTL_SECONDS * 1000;
    let mailboxFailures = 0;
    while (!disconnected && Date.now() < deadline) {
      let result: ConnectResult | null | undefined;
      try {
        result = await session.pollMailbox();
        if (mailboxFailures > 0) {
          send(response, 'waiting', {});
          mailboxFailures = 0;
        }
      } catch (error) {
        if (disconnected) break;
        const message = error instanceof Error ? error.message : '';
        if (message.includes('mailbox request failed') && Date.now() < deadline) {
          mailboxFailures += 1;
          if (mailboxFailures === 1) send(response, 'retrying', {});
          await delay(Math.min(1000 * 2 ** Math.min(mailboxFailures, 3), 5000));
          continue;
        }
        throw error;
      }
      if (disconnected) break;
      if (result) {
        console.info('lexe_connect: mailbox_response');
        if (!result.clientCredentials) {
          send(response, 'rejected', { reason: result.error === 'user_rejected' ? 'rejected' : 'error' });
          break;
        }

        stage = 'wallet_init';
        const node = new binding.LexeNode(
          result.clientCredentials,
          process.env.LEXE_NETWORK || 'mainnet',
        );
        try {
          stage = 'wallet_addresses';
          let human: HumanAddresses | undefined;
          let bitcoinAddress: string | undefined;
          const lookupDeadline = Date.now() + 25_000;
          let attempts = 0;
          while (!disconnected && Date.now() < lookupDeadline && (!human || !bitcoinAddress)) {
            const [humanResult, bitcoinResult] = await Promise.allSettled([
              human ? Promise.resolve(human) : node.getHumanBitcoinAddress(),
              bitcoinAddress ? Promise.resolve(bitcoinAddress) : node.getNextUnusedAddress(),
            ]);
            if (humanResult.status === 'fulfilled') human = humanResult.value;
            if (bitcoinResult.status === 'fulfilled' && bitcoinResult.value) {
              bitcoinAddress = bitcoinResult.value;
            }
            if (human && bitcoinAddress) break;
            if (!disconnected && Date.now() < lookupDeadline) {
              send(response, 'lookup_retrying', {});
              attempts += 1;
              await delay(Math.min(1000 * 2 ** Math.min(attempts, 2), 4000));
            }
          }
          if (!disconnected && (!human || !bitcoinAddress)) {
            throw new Error('Wallet address lookup failed after retries');
          }
          if (!disconnected) {
            send(response, 'approved', {
              lightningAddress: human.lightningAddress || null,
              bitcoinAddress,
              humanBitcoinAddress: human.humanBitcoinAddress || null,
            });
            console.info('lexe_connect: addresses_returned');
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
      console.info('lexe_connect: expired');
    }
  } catch {
    console.warn(`lexe_connect: ${stage}_failed`);
    if (!disconnected) send(response, 'error', { stage });
  } finally {
    session.cancel();
    if (!disconnected) {
      finished = true;
      response.end();
    }
  }
}
