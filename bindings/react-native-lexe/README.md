# react-native-lni-lexe

React Native bindings for the Lexe Lightning wallet, generated from Rust with
`uniffi-bindgen-react-native`.

## Requirements

- React Native with Hermes and the New Architecture enabled
- iOS 13 or newer
- Android API 23 or newer
- An app-writable directory for Lexe state

This is a native package, so it does not run in Expo Go. Expo apps must use a
development build or a prebuilt native project.

## Installation

```sh
npm install react-native-lni-lexe
```

For iOS, install the CocoaPods dependencies after adding the package:

```sh
cd ios && pod install
```

To install the package from a local LNI checkout while developing another app,
pass npm the package directory:

```sh
npm install ../lni/bindings/react-native-lexe
```

Adjust the relative path for your checkout layout. Re-run the command after
rebuilding the package when native binaries change.

## Usage

```ts
import {
  LexeConfig,
  LexeNode,
  PayInvoiceParams,
} from 'react-native-lni-lexe';

const config = LexeConfig.create({
  clientCredentials,
  dataDir: appWritableDataDirectory,
  network: 'mainnet',
});

const node = new LexeNode(config);
const info = await node.getInfo();

const payment = await node.payInvoice(
  PayInvoiceParams.create({
    invoice,
    timeoutSeconds: 60n,
  })
);

// payment contains paymentHash, preimage, and feeMsats.
```

Integer amounts and timestamps use JavaScript `bigint` because the Rust API
uses 64-bit integers.

Keep client credentials and returned payment preimages in secure app storage.
Do not include them in source control or production logs.

## Building the bindings

The npm package contains prebuilt Rust libraries. Maintainers can regenerate
them from the repository root with:

```sh
cd bindings/react-native-lexe
yarn install
yarn ubrn:ios
yarn ubrn:android
yarn build
```

The prebuilt iOS and Android libraries are stored with Git LFS. Install Git LFS
and run `git lfs pull` after cloning before building the example or publishing
the npm package. npm consumers do not need Git LFS.

## License

MIT
