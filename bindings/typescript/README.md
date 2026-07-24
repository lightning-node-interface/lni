# Lightning Node Interface

Remote connect to major Lightning node implementations with one TypeScript interface.

- Supports major nodes: CLN, LND, Phoenixd
- Supports protocols: BOLT11, BOLT12, NWC
- Includes custodial / hosted APIs: Strike, Speed, Blink
- Experimental Arkade Boltz support lives in the optional `@sunnyln/lni-arkade` package
- Experimental Spark support lives in the optional `@sunnyln/lni-spark` package
- LNURL + Lightning Address support (`user@domain.com`, `lnurl1...`)
- Frontend-capable TypeScript runtime (`fetch`-based)

## Install

```bash
npm install @sunnyln/lni
```

## TypeScript Examples

### Node API

```ts
import { createNode, InvoiceType, type BackendNodeConfig } from '@sunnyln/lni';

const backend: BackendNodeConfig = {
  kind: 'lnd',
  config: {
    url: 'https://lnd.example.com',
    macaroon: '...',
  },
};

const node = createNode(backend);

const info = await node.getInfo();
const permissions = await node.getPermissions();

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

`StrikeNode.payInvoice` supports `feeLimitMsat` and `feeLimitPercentage`. Strike does not
accept a maximum fee in the payment request, so LNI enforces the configured limit by
validating the returned Lightning quote before executing it. Percentage limits use the
payment amount before fees, and no fee limit is applied when neither field is provided.
When a valid quote exceeds the configured limit, `payInvoice` throws a `FeeError` before
execution. Its friendly message displays exact sat amounts and explains which
`feeLimitMsat` or `feeLimitPercentage` setting can be increased to allow the payment.
Invalid limit configuration or an unsafe/malformed quote remains an `InvalidInput`
`LniError`.

```ts
import { FeeError } from '@sunnyln/lni';

try {
  await node.payInvoice({ invoice: invoice.invoice, feeLimitMsat: 1_000 });
} catch (error) {
  if (error instanceof FeeError) {
    console.error(error.message);
  }
}
```

### On-chain Bitcoin Payments

On-chain payments use a prepare-then-pay flow so apps can show fees before executing a payment. This is currently implemented for `StrikeNode` and `BlinkNode`.

```ts
import { StrikeNode } from '@sunnyln/lni';

const node = new StrikeNode({ apiKey: '...' });

const transaction = await node.prepareOnchainTransaction({
  address: 'bc1q...',
  amountSats: 100_000,
  fee: { type: 'speed', speed: 'normal' },
  feePayer: 'sender',
  description: 'cold storage',
});

// Show transaction.feeSats, transaction.totalAmountSats, and transaction.expiresAt to the user.

const payment = await node.payOnchain(transaction);
```

`feePayer: 'sender'` means the recipient receives the full requested amount and the sender pays fees on top. `feePayer: 'recipient'` means fees are deducted from the requested amount.

On-chain amounts are expressed in sats. Lightning invoice and offer APIs continue to use msats.

Blink maps `fast`, `normal`, and `slow` to Blink's `FAST`, `MEDIUM`, and `SLOW` payout speeds. Blink does not support `free`, target-confirmation, sats/vbyte, backend fee preferences, or recipient-paid fees for on-chain sends.

`payOnchain` enforces the shared `DEFAULT_ONCHAIN_FEE_GUARDRAIL`: `25_000` sats and `25%` of the send amount. It fails closed when `feeSats` is unknown, such as a recovered duplicate quote that only includes a quote id.

```ts
await node.payOnchain(transaction, {
  feeGuardrail: {
    maxFeeSats: 5_000,
    maxFeePercent: 10,
  },
});

await node.payOnchain(transaction, {
  dangerouslyDisableFeeGuardrail: true,
});
```

For Strike, LNI maps `fast` to `tier_fast`, `normal` to `tier_standard`, and `slow` / `free` to `tier_free`. Use `fee: { type: 'backend', value: 'tier_...' }` to pass a Strike tier id directly.

### BOLT11 Decode

```ts
import { decode } from '@sunnyln/lni';

const decoded = decode(invoice);
console.log(decoded.paymentRequest);
console.log(decoded.payment_hash);
console.log(decoded.amountMsats);
```

`decode` is exported from the package root and returns a normalized keyed object. BOLT11 tags are exposed by name, such as `payment_hash`, `payment_secret`, `description`, `expiry`, `feature_bits`, and `route_hints`. The raw invoice is available as `paymentRequest`; `amount` is a millisatoshi string, `amountMsats` is a number when safely representable, and `expiresAt` is the absolute expiry timestamp.
Node adapters also expose `await node.decode(invoice)`, which returns the decoded BOLT11 object serialized as JSON.

### BOLT12 Offer Decode

```ts
import { decodeOffer } from '@sunnyln/lni';

const decodedOffer = decodeOffer(offer);
console.log(decodedOffer.description);
console.log(decodedOffer.paths?.[0]?.blindedHops);
```

`decodeOffer` decodes BOLT12 offers without requiring node config. Blinded paths are normalized when present:

If an offer amount is denominated in bitcoin, `amountMsats` is set. If the offer includes `currency`, `amount` is denominated in that ISO-4217 currency's minor unit and `amountMsats` is omitted; the payable msats come from the fetched BOLT12 invoice.

```ts
type DecodedBlindedPath = {
  introductionNode:
    | { type: 'node_id'; nodeId: string }
    | {
        type: 'directed_short_channel_id';
        direction: 'node_one' | 'node_two';
        shortChannelId: string;
      };
  blindingPoint: string;
  blindedHops: Array<{
    blindedNodeId: string;
    encryptedPayload: string;
  }>;
};
```

Node adapters expose `await node.decodeOffer(offer)`, which returns the decoded BOLT12 offer serialized as JSON.

### Invoice Event Polling

Poll for invoice settlement after creating an invoice. The callback fires with `'success'`, `'pending'`, or `'failure'`.

```ts
await node.onInvoiceEvents(
  {
    paymentHash: invoice.paymentHash,
    pollingDelaySec: 3,
    maxPollingSec: 60,
  },
  (status, tx) => {
    if (status === 'success') {
      // Invoice was paid and settled
      console.log('Paid!', tx.amountMsats, 'msats');
      console.log('Preimage:', tx.preimage);
    } else if (status === 'pending') {
      // Still waiting — fires each poll interval
      console.log('Waiting for payment...');
    } else if (status === 'failure') {
      // maxPollingSec exceeded without settlement
      console.log('Invoice was not paid within the timeout');
    }
  }
);
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

## Experimental Adapters

`@sunnyln/lni-arkade` and `@sunnyln/lni-spark` are currently experimental packages. Expect API and packaging changes while the adapter split settles.

### Spark

Install:

```bash
npm install @sunnyln/lni @sunnyln/lni-spark
```

Use:

```ts
import { SparkNode, installSparkRuntime } from '@sunnyln/lni-spark';

const runtime = installSparkRuntime({
  apiKey: 'optional-api-key',
  apiKeyHeader: 'x-api-key',
});

const sparkNode = new SparkNode({
  mnemonic: 'abandon ...',
  network: 'mainnet',
  sdkEntry: 'bare',
});

const info = await sparkNode.getInfo();
const invoice = await sparkNode.createInvoice({
  amountMsats: 25_000,
  description: 'Spark invoice',
});

runtime.restore();
```

### Arkade Boltz

Install:

```bash
npm install @sunnyln/lni @sunnyln/lni-arkade
```

Use:

```ts
import { ArkadeBoltzNode } from '@sunnyln/lni-arkade';

const arkadeNode = new ArkadeBoltzNode({
  mnemonic: 'abandon ...',
  arkServerUrl: 'https://mutinynet.arkade.sh',
  network: 'mutinynet',
});

const info = await arkadeNode.getInfo();
const invoice = await arkadeNode.createInvoice({
  amountMsats: 10_000,
  description: 'Arkade invoice',
});
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
- Decode helpers (`decode` for BOLT11, `decodeOffer` for BOLT12 offers)

## Frontend Runtime Notes

- Uses `fetch`, no Node-native runtime dependency required.
- Use `@sunnyln/lni-spark` when Spark support is needed.
- Use `@sunnyln/lni-arkade` when Arkade Boltz support is needed.
- Expo / React Native apps using NWC can install the package's pollyfill `expo-crypto` fallback before using LNI:
  ```ts
  import '@sunnyln/lni/expo-polyfills';
  import { createNode } from '@sunnyln/lni';
  ```
- For local `file:` package development with Expo, build the package first (`bindings/typescript`: `npm run build`) and use the Expo example `metro.config.js` pattern for `./dist/*` resolution.
- You can inject custom fetch via constructor options:
  - `new LndNode(config, { fetch: customFetch })`
- Most backends require secrets (API keys, macaroons, runes, passwords). For production web apps, use a backend proxy/BFF to protect credentials.

## Security Scanner Notes

Socket may report `networkAccess` for this package because `@sunnyln/lni` is intentionally a network client. The TypeScript runtime resolves `globalThis.fetch` in `dist/internal/http.js` and uses it to call configured Lightning node APIs, LNURL / Lightning Address endpoints, and supported hosted provider APIs.

This network access is expected package behavior. Consumers should still review which backend URLs and credentials they configure, and browser applications should avoid shipping node credentials directly to untrusted clients.

## Example App

From `bindings/typescript`, run:

```bash
npm run example
```

This builds `@sunnyln/lni`, `@sunnyln/lni-arkade`, and `@sunnyln/lni-spark`, installs the web example dependencies, and starts the Vite app from `bindings/typescript-spark/examples/spark-web`.

## Build and Publish (package maintainers)

```bash
npm run prepack
npm run pack:dry-run
npm run publish:public
```

To dry-run all published TypeScript packages in release order from `bindings/typescript`:

```bash
npm run release:dry-run
```

To publish all three packages in order (`@sunnyln/lni`, `@sunnyln/lni-arkade`, `@sunnyln/lni-spark`):

```bash
npm run release:public
```

## Integration tests

```bash
npm run test:integration
```

These scripts set `NODE_TLS_REJECT_UNAUTHORIZED=0` because many local Lightning nodes use self-signed certs in test environments. Do not use this in production.
