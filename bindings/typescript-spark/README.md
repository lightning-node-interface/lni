# @sunnyln/lni-spark

Optional Spark adapter for `@sunnyln/lni`.

```ts
import { SparkNode, installSparkRuntime } from '@sunnyln/lni-spark';

const runtime = installSparkRuntime();
const node = new SparkNode({
  mnemonic: 'abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about',
  network: 'mainnet',
});

const info = await node.getInfo();
runtime.restore();
```

Install this package only when you need Spark support. The core `@sunnyln/lni` package contains the shared Lightning interfaces and non-Spark node adapters.

## Decode

Spark uses the shared decode helpers from `@sunnyln/lni`. BOLT11 decode and BOLT12 offer decode are pure local operations and do not require a connected Spark wallet.

```ts
import { decode, decodeOffer } from '@sunnyln/lni';

const decodedBolt11 = decode('lnbc...');
const decodedOffer = decodeOffer('lno1...');

console.log(decodedBolt11.payment_hash);
console.log(decodedOffer.paths?.[0]?.blindingPoint);
```

`SparkNode` also implements the shared node methods:

```ts
const bolt11Json = await node.decode('lnbc...');
const offerJson = await node.decodeOffer('lno1...');
```
