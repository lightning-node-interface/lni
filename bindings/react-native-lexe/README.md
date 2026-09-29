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

`LexeLniNode` implements the shared `LightningNode` and `OnchainPayments` interfaces from
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
const address = await node.getHumanBitcoinAddress();
const payment = await node.payInvoice({ invoice, timeoutSeconds: 60 });

node.close();

// address contains humanBitcoinAddress, lightningAddress, offer, and updatable.
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

The package version is declared in `package.json` and is currently `0.2.18`.
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

`release:pack` creates `sunnyln-react-native-lni-lexe-0.2.18.tgz` in this
directory.
`release:public` repeats the native build and validation before running
`npm publish --access public`. An npm version can only be published once, so
bump `package.json` before the next release.

## License

MIT

### Authenticated credential inspection

`await node.getClientInfo()` calls Lexe's authenticated `client-info` API.
It returns `kind`, `clientPubkey`, `createdAtMs`, optional `expiresAtMs`,
`scopes`, `permissions`, and `effectivePermissions`. Timestamps are Unix
milliseconds; an absent expiry means the credential does not expire.

Use this API to validate an app-to-app credential before saving it: require
`client_credentials`, match the client public key and the exact requested
scopes, reject unexpected explicit permissions, and check expiry. Inspection
errors must fail closed. `getPermissions()` only describes the adapter's
supported operations; it is not proof of this credential's authorization.

This requires Lexe SDK 0.1.23 or newer and a rebuilt native application. Updating the
JavaScript package alone does not add the native method. No budget enforcement
or authenticated budget inspection is exposed by this bridge.

### On-chain payments

`LexeLniNode` implements LNI's `OnchainPayments` interface using Lexe SDK
`0.1.24`. Preparation validates the address/network, amount, fee preference,
notes, and payment ID locally. It does **not** broadcast or reserve funds.

```ts
const transaction = await node.prepareOnchainTransaction({
  address: recipientAddress,
  amountSats: 10_000,
  fee: { type: 'speed', speed: 'normal' },
  // Optional: a fresh 64-character hex ID for this payment.
  idempotencyKey: paymentId,
});

// Persist transaction before sending; reuse it for retries.
const payment = await node.payOnchain(transaction);
```

Lexe does not expose fee estimation or a maximum fee through this SDK.
`feeSats` and `totalAmountSats` are therefore absent from the prepared request.
Prepared transactions explicitly report `feeLimitSupported: false`. Ordinary sends use Lexe's provider-determined fees;
no override flag is needed. Passing any `feeGuardrail` (including an empty one)
is rejected before sending, even if `dangerouslyDisableFeeGuardrail` is also set.
Supplying `feeSats` in the prepared object cannot enforce a cap. The actual fee
is returned after execution. An app can display: “Network fee determined by
Lexe. Normal priority. The exact fee is available after sending.”

Supported speeds are `fast`, `normal`, and `slow`, mapped to Lexe's `high`,
`normal`, and `background` priorities. The default is `normal`. Recipient-paid
fees, `free`, confirmation targets, custom fee rates, and backend fee settings
are unsupported. `description` becomes a personal note visible only to the sender.

A generated payment ID is stored in `transaction.id` when no `idempotencyKey`
is provided. Persist and reuse the same prepared transaction after a timeout or
unknown outcome. Preparing again without the same ID creates a new payment.
Lexe returns the existing payment for repeated IDs, including failed payments.
Execution returns without waiting for confirmations, usually with state
`pending`; this is not confirmation that the transaction has settled.

Rust exposes the same flow through `LexeNode::prepare_onchain_transaction` and
`LexeNode::pay_onchain`. Use `pay_onchain_with_options` only when options are
needed; explicit `fee_guardrail` values are unsupported.

Across the shared LNI interface, omitted fee options use the adapter's provider
policy. Strike and Blink retain their default quote/estimate checks. Lexe uses
provider-determined fees. Applications requiring a fee check should pass an
explicit `feeGuardrail` and handle an unsupported error. Capability fields are
optional for older adapters; missing values mean unknown, not supported.

The ignored Rust test `lexe::lib::tests::test_pay_onchain_e2e` exercises a real
send. Run it only deliberately with `LEXE_ONCHAIN_SEND_CONFIRM=YES`,
`LEXE_CLIENT_CREDENTIALS`, `LEXE_ONCHAIN_ADDRESS`, `LEXE_ONCHAIN_AMOUNT_SATS`, and
`LEXE_ONCHAIN_IDEMPOTENCY_KEY` set. It uses mainnet and spends real funds.

API reference: [Lexe pay-onchain](https://docs.lexe.tech/cli/#pay-onchain).
