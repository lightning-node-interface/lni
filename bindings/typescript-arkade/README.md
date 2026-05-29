# @sunnyln/lni-arkade

Optional Arkade Boltz adapter for `@sunnyln/lni`.

```ts
import { ArkadeBoltzNode } from '@sunnyln/lni-arkade';

const node = new ArkadeBoltzNode({
  mnemonic: 'abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about',
  arkServerUrl: 'https://mutinynet.arkade.sh',
  network: 'mutinynet',
});

const info = await node.getInfo();
```

Install this package only when you need Arkade Boltz support. The core `@sunnyln/lni` package contains the shared Lightning interfaces and smaller non-Arkade node adapters.

## Decode

Arkade Boltz uses the shared decode helpers from `@sunnyln/lni`. BOLT11 decode and BOLT12 offer decode are pure local operations.

```ts
import { decode, decodeOffer } from '@sunnyln/lni';

const decodedBolt11 = decode('lnbc...');
const decodedOffer = decodeOffer('lno1...');

console.log(decodedOffer.paths?.[0]?.blindedHops);
```

`ArkadeBoltzNode` also implements the shared node methods:

```ts
const bolt11Json = await node.decode('lnbc...');
const offerJson = await node.decodeOffer('lno1...');
```
