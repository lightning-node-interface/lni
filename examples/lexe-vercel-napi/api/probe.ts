type ProbeRequest = {
  method?: string;
  url?: string;
  headers: Record<string, string | string[] | undefined>;
};

type ProbeResponse = {
  statusCode: number;
  setHeader(name: string, value: string): void;
  end(body: string): void;
};

type LexeBinding = {
  LexeNode?: new (credentials: string, network: string) => {
    close(): void;
    getInfo(): Promise<unknown>;
    getPermissions(): Promise<{ getInfo: boolean; createInvoice: boolean }>;
    getClientInfo(): Promise<{ kind: string; scopes: string[] }>;
    getHumanBitcoinAddress(): Promise<{ humanBitcoinAddress: string }>;
    getNextUnusedAddress(): Promise<string>;
    [method: string]: unknown;
  };
  LexeConnectSession?: new (options: { scopes: string[] }) => {
    connectionString(): string;
    cancel(): void;
  };
};

// Keep this example dependency-free while still checking the function's types.
declare const process: {
  platform: string;
  arch: string;
  version: string;
  env: Record<string, string | undefined>;
  report?: { getReport(): { header: { glibcVersionRuntime?: string } } };
};
declare function require(path: string): LexeBinding;

export default async function handler(request: ProbeRequest, response: ProbeResponse) {
  response.setHeader('Cache-Control', 'no-store');
  response.setHeader('Content-Type', 'application/json');

  if (request.method !== 'GET') {
    response.statusCode = 405;
    return response.end(JSON.stringify({ ok: false, stage: 'method' }));
  }

  const url = new URL(request.url || '/', 'https://example.invalid');
  const mode = url.searchParams.get('mode') || 'platform';
  if (mode === 'platform') {
    return response.end(JSON.stringify({
      ok: true,
      platform: process.platform,
      arch: process.arch,
      node: process.version,
      glibc: process.report?.getReport().header.glibcVersionRuntime || null,
    }));
  }

  if (mode === 'wallet') {
    const probeToken = process.env.LEXE_PROBE_TOKEN;
    if (!probeToken || request.headers['x-probe-token'] !== probeToken) {
      response.statusCode = 401;
      return response.end(JSON.stringify({ ok: false }));
    }
  }

  if (process.platform !== 'linux' || process.arch !== 'x64') {
    response.statusCode = 500;
    return response.end(JSON.stringify({ ok: false, stage: 'platform' }));
  }

  let binding: LexeBinding;
  try {
    binding = require('../native/lni_js.linux-x64-gnu.node') as LexeBinding;
  } catch {
    response.statusCode = 500;
    return response.end(JSON.stringify({ ok: false, stage: 'addon_load' }));
  }

  if (typeof binding.LexeNode !== 'function') {
    response.statusCode = 500;
    return response.end(JSON.stringify({ ok: false, stage: 'addon_export' }));
  }

  if (mode === 'connect') {
    try {
      if (typeof binding.LexeConnectSession !== 'function') throw new Error('missing export');
      const session = new binding.LexeConnectSession({ scopes: ['receive'] });
      const connectionString = session.connectionString();
      if (connectionString.length < 20) throw new Error('invalid connection string');
      session.cancel();
      return response.end(JSON.stringify({ ok: true, stage: 'connect_session' }));
    } catch {
      response.statusCode = 500;
      return response.end(JSON.stringify({ ok: false, stage: 'connect_session' }));
    }
  }

  if (mode !== 'wallet' && mode !== 'address' && mode !== 'capabilities') {
    return response.end(JSON.stringify({ ok: true, stage: 'addon_load' }));
  }

  const credentials = process.env.LEXE_CLIENT_CREDENTIALS;
  if (!credentials) {
    response.statusCode = 503;
    return response.end(JSON.stringify({ ok: false, stage: 'credential_missing' }));
  }

  let node: InstanceType<NonNullable<LexeBinding['LexeNode']>>;
  try {
    node = new binding.LexeNode(credentials, process.env.LEXE_NETWORK || 'mainnet');
  } catch {
    response.statusCode = 500;
    return response.end(JSON.stringify({ ok: false, stage: 'wallet_init' }));
  }

  try {
    if (mode === 'capabilities') {
      const methods = [
        'createInvoice', 'payInvoice', 'prepareOnchainTransaction', 'payOnchain',
        'createOffer', 'getOffer', 'listOffers', 'payOffer', 'lookupInvoice',
        'listTransactions', 'decode', 'decodeOffer', 'onInvoiceEventsRaw',
      ];
      if (methods.some((method) => typeof node[method] !== 'function')) {
        throw new Error('missing wallet export');
      }
      const [info, permissions, clientInfo, humanAddress] = await Promise.all([
        node.getInfo(), node.getPermissions(), node.getClientInfo(), node.getHumanBitcoinAddress(),
      ]);
      if (!info || !permissions.getInfo || !permissions.createInvoice ||
          !clientInfo.kind || !Array.isArray(clientInfo.scopes) ||
          typeof humanAddress.humanBitcoinAddress !== 'string') {
        throw new Error('invalid capability result');
      }
      node.close();
      let rejectedAfterClose = false;
      try { await node.getInfo(); } catch { rejectedAfterClose = true; }
      if (!rejectedAfterClose) throw new Error('closed node still accepts requests');
      return response.end(JSON.stringify({ ok: true, stage: 'wallet_capabilities' }));
    }
    if (mode === 'wallet') await node.getInfo();
    const address = await node.getNextUnusedAddress();
    if (typeof address !== 'string' || address.length < 20) {
      throw new Error('Invalid address result');
    }
    node.close();
    return response.end(JSON.stringify(mode === 'address'
      ? { ok: true, address }
      : { ok: true, stage: 'wallet_address' }));
  } catch {
    response.statusCode = 502;
    return response.end(JSON.stringify({ ok: false, stage: 'wallet_request' }));
  }
}
