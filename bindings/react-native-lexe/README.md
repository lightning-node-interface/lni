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
first build the native libraries for the platforms you use:

```sh
cd /path/to/lni/bindings/react-native-lexe
corepack yarn install --immutable
corepack yarn ubrn:ios
corepack yarn ubrn:android
corepack yarn build
```

Then return to the consuming app and pass npm the package directory:

```sh
cd /path/to/your-react-native-app
npm install /path/to/lni/bindings/react-native-lexe
```

Adjust the relative paths for your checkout layout. A published npm package
already contains the native libraries; only source-checkout development needs
the Rust, Xcode, and Android NDK build steps. Rebuild and reinstall the local
package whenever its Rust or generated native code changes.

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

The npm package contains prebuilt Rust libraries, but those binaries are not
stored in Git. Maintainers can generate them from the repository root with:

```sh
cd bindings/react-native-lexe
yarn install
yarn ubrn:ios
yarn ubrn:android
yarn build
```

`yarn pack:dry-run` verifies that both iOS libraries and all four Android ABIs
exist before creating a package. Generated TypeScript and C++ bindings remain
versioned so their changes can be reviewed.

## Publishing

The `Build react-native-lni-lexe` GitHub Actions workflow builds the native
libraries on macOS, verifies the generated source is current, creates the npm
tarball, and uploads it as a workflow artifact.

To publish, update the version in `package.json` and push a matching tag such
as `react-native-lni-lexe-v0.1.0`. Publishing requires the repository's
`NPM_TOKEN` secret. The workflow can also be started manually without
publishing to produce a testable npm tarball.

## License

MIT
