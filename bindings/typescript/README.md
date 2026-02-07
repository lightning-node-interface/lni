# Lightning Node Interface

Remote connect to major Lightning node implementations with one TypeScript interface.

- Supports major nodes: CLN, LND, Phoenixd
- Supports protocols: BOLT11, BOLT12, NWC
- Includes custodial APIs: Strike, Speed, Blink
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
- LNURL helpers (`detectPaymentType`, `needsResolution`, `resolveToBolt11`, `getPaymentInfo`)

Not included yet:
- `SparkNode` (planned)

## Frontend Runtime Notes

- Uses `fetch`, no Node-native runtime dependency required.
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
