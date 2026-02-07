# LNI TypeScript (Frontend-first)

Pure TypeScript port of `crates/lni` HTTP adapters for frontend runtimes.

## Scope

Implemented in this package:

- `PhoenixdNode`
- `ClnNode`
- `LndNode`
- `NwcNode`
- `StrikeNode`
- `SpeedNode`
- `BlinkNode`
- LNURL helpers (`detectPaymentType`, `needsResolution`, `resolveToBolt11`, `getPaymentInfo`)

Not included yet:

- `SparkNode` (intentionally deferred)

## Install

```bash
npm install @sunnyln/lni
```

From source:

```bash
cd bindings/typescript
npm install
npm run typecheck
npm run build
```

## Install from this GitHub repo (without publishing)

Because this package lives in a monorepo subfolder, install it from a local path after cloning:

```bash
git clone https://github.com/lightning-node-interface/lni.git
npm install ./lni/bindings/typescript
```

One-liner:

```bash
TMP_DIR=$(mktemp -d) && git clone --depth 1 https://github.com/lightning-node-interface/lni.git "$TMP_DIR/lni" && npm install "$TMP_DIR/lni/bindings/typescript"
```

From another project:

```bash
npm install /absolute/path/to/lni/bindings/typescript
```

## Packaging and publish

```bash
cd bindings/typescript
npm install
npm run prepack
npm run pack:dry-run
```

To publish:

```bash
npm login
npm run publish:public
```

Per-node real integration test scripts are available before publishing:

- `npm run test:integration:phoenixd`
- `npm run test:integration:cln`
- `npm run test:integration:lnd`
- `npm run test:integration:strike`
- `npm run test:integration:speed`
- `npm run test:integration:blink`
- `npm run test:integration:nwc`

## Basic usage

```ts
import { LndNode } from '@sunnyln/lni';

const node = new LndNode({
  url: 'https://127.0.0.1:8080',
  macaroon: '...'
});

const info = await node.getInfo();
console.log(info.sendBalanceMsat);
```

## Frontend runtime notes

- This package uses `fetch` and does not depend on Node native modules.
- You can inject a custom fetch implementation via constructor options:
  - `new LndNode(config, { fetch: customFetch })`
- `socks5Proxy` and `acceptInvalidCerts` are config-compatible with Rust structs, but not applied in browser fetch runtimes.
- Most backends require secrets (API keys, macaroons, runes, passwords). For production web apps, use a backend proxy/BFF to protect secrets.

## API parity

The exported TypeScript interfaces mirror `bindings/lni_nodejs/index.d.ts` shapes:

- `NodeInfo`
- `Transaction`
- `CreateInvoiceParams`
- `PayInvoiceParams`
- `ListTransactionsParams`
- `LookupInvoiceParams`
- `OnInvoiceEventParams`

Methods are class-based (`getInfo`, `createInvoice`, `payInvoice`, `lookupInvoice`, etc.), with polling callback support via `onInvoiceEvents`.
