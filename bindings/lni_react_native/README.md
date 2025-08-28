# react-native-lni

react-native-lni

## Installation

```sh
npm install react-native-lni
```

## Usage


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
} from 'react-native-lni';

// ...

const node = new LndNode(
    LndConfig.create({
    url: '',
    macaroon: '',
    socks5Proxy: undefined, // 'socks5h://127.0.0.1:9050',
    })
);
const info = await node.getInfo();
```


## Contributing

See the [contributing guide](CONTRIBUTING.md) to learn how to contribute to the repository and the development workflow.

## License

MIT

---

Made with [create-react-native-library](https://github.com/callstack/react-native-builder-bob)
