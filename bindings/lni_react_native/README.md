# lni_react_native

React Native bindings for the Lightning Node Interface (LNI) library.

## Installation

```sh
npm install lni_react_native
```

### Spark SDK Requirements

If you're using the Spark node integration, you'll also need to install `react-native-fs` for proper filesystem access on mobile:

```sh
npm install react-native-fs
```

For iOS, run pod install after adding the dependency:

```sh
cd ios && pod install
```

## Usage

### LND Example

```js
import {
  LndNode,
  LndConfig,
  PhoenixdNode,
  PhoenixdConfig,
  type OnInvoiceEventCallback,
  Transaction,
  BlinkConfig,
  BlinkNode,
} from 'lni_react_native';

const node = new LndNode(
  LndConfig.create({
    url: '',
    macaroon: '',
    socks5Proxy: undefined, // 'socks5h://127.0.0.1:9050',
  })
);
const info = await node.getInfo();
```

### Spark Example

```js
import { createSparkNode, SparkConfig } from 'lni_react_native';
import RNFS from 'react-native-fs';

// Spark requires a valid filesystem path for storage
const storageDir = `${RNFS.DocumentDirectoryPath}/spark_data`;

const config = SparkConfig.create({
  mnemonic: 'your twelve word mnemonic phrase here ...',
  passphrase: undefined,
  apiKey: 'your-breez-api-key', // Required for mainnet
  storageDir: storageDir,
  network: 'mainnet', // or 'regtest'
});

const sparkNode = await createSparkNode(config);
const info = await sparkNode.getInfo();
console.log(`Balance: ${info.sendBalanceMsat} msats`);
```


## Contributing

See the [contributing guide](CONTRIBUTING.md) to learn how to contribute to the repository and the development workflow.

## License

MIT

---

Made with [create-react-native-library](https://github.com/callstack/react-native-builder-bob)
