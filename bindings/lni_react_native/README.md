# react-native-lni

React Native bindings for LNI (Lightning Node Interface) - a unified interface for interacting with multiple Lightning Network node implementations.

## Features

- 🔌 **Multi-Node Support**: Works with LND, Core Lightning (CLN), Phoenixd, Strike, Blink, Speed, and NWC
- ⚡ **Lightning Operations**: Create invoices, pay invoices/offers, manage transactions
- 🔐 **Secure**: Built with Rust using UniFFI for type-safe native bindings
- 🎯 **TypeScript**: Full TypeScript support with generated type definitions
- 🌐 **Tor Support**: Built-in SOCKS5 proxy support for privacy
- 🏗️ **New Architecture**: Built for React Native's new architecture with TurboModules

## Installation

```sh
npm install react-native-lni
# or
yarn add react-native-lni
```

### Prerequisites

This package requires React Native's New Architecture to be enabled:

**iOS** - Add to your Podfile:
```ruby
ENV['RCT_NEW_ARCH_ENABLED'] = '1'
```

**Android** - Add to `gradle.properties`:
```properties
newArchEnabled=true
hermesEnabled=true
```

Then install iOS dependencies:
```sh
cd ios && pod install && cd ..
```

## Usage

### LND (Lightning Network Daemon)

```js
import { LndNode, LndConfig } from 'react-native-lni';

const node = new LndNode(
  LndConfig.create({
    url: 'https://your-lnd-node:8080',
    macaroon: 'your-macaroon-hex',
    socks5Proxy: undefined, // Optional: 'socks5h://127.0.0.1:9050' for Tor
    acceptInvalidCerts: false, // Optional: true for self-signed certs
  })
);

// Get node information
const info = await node.getInfo();
console.log('Node pubkey:', info.pubkey);

// Create an invoice
const invoice = await node.createInvoice({
  amountSats: 1000,
  description: 'Test payment',
});
console.log('Payment request:', invoice.bolt11);

// Pay an invoice
const payment = await node.payInvoice({
  bolt11: 'lnbc...',
  amountSats: undefined, // Use invoice amount
});
```

### Phoenixd

```js
import { PhoenixdNode, PhoenixdConfig } from 'react-native-lni';

const node = new PhoenixdNode(
  PhoenixdConfig.create({
    url: 'https://your-phoenixd-node:9740',
    password: 'your-password',
  })
);

const info = await node.getInfo();
```

### Core Lightning (CLN)

```js
import { ClnNode, ClnConfig } from 'react-native-lni';

const node = new ClnNode(
  ClnConfig.create({
    url: 'https://your-cln-node:3010',
    rune: 'your-rune',
  })
);

const info = await node.getInfo();
```

### Strike

```js
import { StrikeNode, StrikeConfig } from 'react-native-lni';

const node = new StrikeNode(
  StrikeConfig.create({
    apiKey: 'your-api-key',
    baseUrl: 'https://api.strike.me/v1', // Optional
  })
);

const info = await node.getInfo();
```

### Blink (Bitcoin Beach Wallet)

```js
import { BlinkNode, BlinkConfig } from 'react-native-lni';

const node = new BlinkNode(
  BlinkConfig.create({
    apiKey: 'your-api-key',
    baseUrl: 'https://api.blink.sv/graphql', // Optional
  })
);

const info = await node.getInfo();
```

### Event Polling

Subscribe to invoice events:

```js
const eventHandle = await node.onInvoiceEvent(
  {
    type: 'ALL', // or 'BOLT11', 'BOLT12'
  },
  {
    success: (transaction) => {
      console.log('Invoice paid!', transaction);
    },
    pending: (transaction) => {
      console.log('Invoice pending', transaction);
    },
    failure: (transaction) => {
      console.log('Payment failed', transaction);
    },
  }
);

// Later, stop polling
await eventHandle.stop();
```

## API Overview

All node implementations share a common interface:

- `getInfo()` - Get node information
- `createInvoice(params)` - Create a Lightning invoice
- `payInvoice(params)` - Pay a Lightning invoice
- `lookupInvoice(params)` - Look up invoice details
- `createOffer(params)` - Create a BOLT12 offer (where supported)
- `fetchOffer(params)` - Fetch offer details
- `payOffer(params)` - Pay a BOLT12 offer
- `onInvoiceEvent(params, callbacks)` - Subscribe to invoice events

## TypeScript Support

This package includes full TypeScript definitions. All types are automatically generated from the Rust implementation, ensuring type safety across the FFI boundary.

```typescript
import type { 
  NodeInfo, 
  Transaction, 
  CreateInvoiceParams,
  OnInvoiceEventCallback 
} from 'react-native-lni';
```


## Contributing

See the [contributing guide](CONTRIBUTING.md) to learn how to contribute to the repository and the development workflow.

## License

MIT

---

Made with [create-react-native-library](https://github.com/callstack/react-native-builder-bob)
