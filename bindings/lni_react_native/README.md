# lni_react_native

lni_react_native

## Installation

```sh
npm install lni_react_native
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
} from 'lni_react_native';

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
