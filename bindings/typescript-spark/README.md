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
