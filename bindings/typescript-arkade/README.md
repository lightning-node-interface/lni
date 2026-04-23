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
