# @sunnyln/react-native-lni-lexe

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
npm install @sunnyln/lni @sunnyln/react-native-lni-lexe
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

`LexeLniNode` implements the shared `LightningNode` interface from
`@sunnyln/lni`. Supply an app-writable directory; the package does not choose or
create a filesystem location.

```ts
import { LexeLniNode } from '@sunnyln/react-native-lni-lexe';

const node = new LexeLniNode({
  clientCredentials,
  dataDir: appWritableDataDirectory,
  network: 'mainnet',
});

const info = await node.getInfo();
const payment = await node.payInvoice({ invoice, timeoutSeconds: 60 });

node.close();

// payment contains paymentHash, preimage, and feeMsats.
```

The adapter accepts the shared LNI `number` fields and safely converts them to
the native binding's 64-bit `bigint` fields. Native response values outside
JavaScript's safe-integer range are rejected with `LniError` instead of being
silently rounded. Native UniFFI errors are also exposed as the consumer's
`@sunnyln/lni` `LniError` class.

The generated `LexeNode`, record factories, and native `bigint` API remain
available as lower-level exports from this package for applications that need
them.

For an Expo development-build application, create the directory in application
code and pass its path to the adapter. For example, ZapriteP2P should create
`Documents/lexe` with Expo FileSystem, construct `LexeLniNode` with that path,
and pass the node directly to `payLniLightningQuote`. Expo FileSystem is not a
dependency of this package.

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

`yarn release:dry-run` rebuilds the iOS and Android libraries, runs the adapter
tests, lint, and type checking, links the React Native Android example for
`arm64-v8a`, verifies that both iOS libraries and all four Android ABIs are
present, and shows the contents of the npm package without publishing it.
Generated TypeScript and C++ bindings remain versioned so their changes can be
reviewed.

The scoped npm name is intentionally separate from the internal native identity.
`ubrn.config.yaml` pins that identity to `react-native-lni-lexe`, which generates
`NativeLniLexe`, `LniLexeModule`, the `lnilexe` C++ namespace, and the
`react-native-lni-lexe` Android shared library.

## Publishing

The package version is declared in `package.json` and is currently `0.2.16`.
For a local release, authenticate with npm and inspect the tarball before
publishing:

```sh
cd bindings/react-native-lexe
npm login
npm whoami
corepack yarn install --immutable
corepack yarn release:dry-run
corepack yarn release:pack
corepack yarn release:public
```

`release:pack` creates `sunnyln-react-native-lni-lexe-0.2.16.tgz` in this
directory.
`release:public` repeats the native build and validation before running
`npm publish --access public`. An npm version can only be published once, so
bump `package.json` before the next release.

## License

MIT
