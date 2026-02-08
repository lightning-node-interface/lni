# Lightning Node Interface

Remote connect to major Lightning node implementations with one TypeScript interface.

- Supports major nodes: CLN, LND, Phoenixd
- Supports protocols: BOLT11, BOLT12, NWC
- Includes custodial APIs: Strike, Speed, Blink
- Includes Spark (`SparkNode`) with pure TypeScript signer patching (no UniFFI bindings)
- LNURL + Lightning Address support (`user@domain.com`, `lnurl1...`)
- Frontend-capable TypeScript runtime (`fetch`-based)

## Install

```bash
npm install @sunnyln/lni
```

## TypeScript Examples

### Node API

```ts
import {
  createNode,
  InvoiceType,
  type BackendNodeConfig,
} from '@sunnyln/lni';

const backend: BackendNodeConfig = {
  kind: 'lnd',
  config: {
    url: 'https://lnd.example.com',
    macaroon: '...',
  },
};

const node = createNode(backend);

const info = await node.getInfo();

const invoiceParams = {
  invoiceType: InvoiceType.Bolt11,
  amountMsats: 2000,
  description: 'your memo',
  expiry: 3600,
};

const invoice = await node.createInvoice(invoiceParams);

const payInvoiceParams = {
  invoice: invoice.invoice,
  feeLimitPercentage: 1,
  allowSelfPayment: true,
};

const payment = await node.payInvoice(payInvoiceParams);

const status = await node.lookupInvoice({ paymentHash: invoice.paymentHash });

const txs = await node.listTransactions({ from: 0, limit: 10 });
```

### Spark (browser + Expo Go oriented)

```ts
import { createNode, installSparkRuntime } from '@sunnyln/lni';

// One-time runtime setup for browser / React Native / Expo Go.
// - Adds Buffer + atob/btoa polyfills when missing
// - Wraps fetch for gRPC-web body reader compatibility
// - Optionally injects API key header into same-origin Spark HTTP calls
const sparkRuntime = installSparkRuntime({
  apiKey: 'optional-api-key',
  apiKeyHeader: 'x-api-key',
  apiKeySameOriginOnly: true, // default true; safer for browser CORS
});

const sparkNode = createNode({
  kind: 'spark',
  config: {
    mnemonic: 'abandon ...', // store securely in production
    network: 'mainnet', // or regtest/testnet/signet/local
    // optional:
    // passphrase: '...',
    // defaultMaxFeeSats: 20,
    // sparkOptions: { ...sdk options... },
    // sdkEntry: 'auto' | 'bare' | 'native' | 'default'
  },
});

const sparkInfo = await sparkNode.getInfo();
const sparkInvoice = await sparkNode.createInvoice({
  amountMsats: 25_000,
  description: 'Spark invoice',
});

// Optional cleanup (restores original global fetch if installSparkRuntime changed it)
sparkRuntime.restore();
```

Spark entrypoint behavior:
- `sdkEntry: 'auto'` (default) uses a browser-safe bundled Spark bare runtime in browser/Expo and falls back to the default SDK entry in Node.
- `sdkEntry: 'bare'` forces the browser-safe bundled no-WASM/no-native path.
- `sdkEntry: 'default'` uses the standard SDK export (may load WASM).
- `sdkEntry: 'native'` uses the React Native native SDK entry.

Node-only note: use `sdkEntry: 'default'` for Node environments.

For NWC specifically, `createNode` returns `NwcNode` when `kind: 'nwc'`, so you can close it:

```ts
const nwcNode = createNode({ kind: 'nwc', config: { nwcUri: 'nostr+walletconnect://...' } });
// ... use node
nwcNode.close();
```

### LNURL + Lightning Address

```ts
import { detectPaymentType, needsResolution, getPaymentInfo, resolveToBolt11 } from '@sunnyln/lni';

const destination = 'user@domain.com';

const type = detectPaymentType(destination);
const requiresResolution = needsResolution(destination);
const info = await getPaymentInfo(destination, 100_000);
const bolt11 = await resolveToBolt11(destination, 100_000);
```

### Invoice Event Polling

```ts
await node.onInvoiceEvents(
  {
    paymentHash: invoice.paymentHash,
    pollingDelaySec: 3,
    maxPollingSec: 60,
  },
  (status, tx) => {
    console.log('Invoice event:', status, tx);
  },
);
```

## Implemented in this package

- `PhoenixdNode`
- `ClnNode`
- `LndNode`
- `NwcNode`
- `StrikeNode`
- `SpeedNode`
- `BlinkNode`
- `SparkNode`
- LNURL helpers (`detectPaymentType`, `needsResolution`, `resolveToBolt11`, `getPaymentInfo`)

## Frontend Runtime Notes

- Uses `fetch`, no Node-native runtime dependency required.
- Spark no longer requires project-level Spark shims/vendor bundles; those are packaged in `@sunnyln/lni`.
- For local `file:` package development with Expo, build the package first (`bindings/typescript`: `npm run build`) and use the Expo example `metro.config.js` pattern for `./dist/*` resolution.
- If Expo shows `Invalid call ... import(specifier)` in `dist/nodes/spark.js`, rebuild `bindings/typescript` and restart Metro with cache clear (`npx expo start --clear`).
- You can inject custom fetch via constructor options:
  - `new LndNode(config, { fetch: customFetch })`
- Most backends require secrets (API keys, macaroons, runes, passwords). For production web apps, use a backend proxy/BFF to protect credentials.

## Build and Publish (package maintainers)

```bash
npm run prepack
npm run pack:dry-run
npm run publish:public
```

## Integration tests

```bash
npm run test:integration
```

These scripts set `NODE_TLS_REJECT_UNAUTHORIZED=0` because many local Lightning nodes use self-signed certs in test environments. Do not use this in production.
