# LNI Kotlin Bindings

Kotlin bindings for the Lightning Node Interface (LNI) library, generated using UniFFI.

## Overview

This package provides Kotlin bindings for LNI, allowing you to interact with various Lightning Network node implementations from Kotlin/Android applications.

## Supported Nodes

- **BlinkNode** - Blink Lightning service
- **StrikeNode** - Strike Lightning service
- **PhoenixdNode** - Phoenixd daemon
- **LndNode** - LND (Lightning Network Daemon)
- **ClnNode** - Core Lightning (CLN)
- **NwcNode** - Nostr Wallet Connect
- **SpeedNode** - Speed Lightning service
- **GaloyNode**, **FlashNode** - Galoy-compatible services
- **SparkNode** - Breez Spark (default `spark` feature)
- **LexeNode** - revocable Lexe credentials, Lightning and on-chain methods

Receive-only support is separate from spending: `createLnurlReceiveInvoice` and
`verifyLnurlReceiveInvoice` support Lightning Address/LNURL accounts; watch-only
Bitcoin APIs derive external addresses from account public keys or ranged public
descriptors and inspect a
user-selected Esplora backend. Watch-only configurations never contain private keys.
Eight branded XPUB wallets share these APIs; branding does not change derivation.
Generic Bitcoin addresses use `validateBitcoinAddress` and the same observer.

Do not infer payment settlement from successful submission or a remote `settled`
flag. Lightning settlement requires the exact invoice's preimage. Esplora results
are observations from a trusted backend, not independently verified consensus.
The host must persist allocated indices, retain transaction outputs by txid/vout,
apply its confirmation threshold, and reconcile reorganizations.

## Building

### Prerequisites

- Rust toolchain (stable)
- Cargo from PATH (including any security wrapper)
- Python 3 for API/artifact verification (standard library only)
- Existing locked dependencies; obtain approval before installing missing tools
- For Android: `cargo-ndk`, Android NDK, and requested Rust Android targets
  already installed. The build never installs targets or package tools.

### Generate Kotlin bindings

```bash
./build.sh --release
```

The build uses `--locked`, preserves PATH package-manager wrappers, generates
Kotlin from the matching native library, and builds Android libraries with 16 KiB
page alignment (verified in each ELF load segment). It writes a generated
`build-manifest.json` pairing feature flags, source hashes, and artifact hashes. To generate host bindings only, use `./build.sh --no-android`.
To build only ARM64, set `ANDROID_ABIS=arm64-v8a`.

`LNI_FEATURES=uniffi,rustls-tls` excludes the optional Spark implementation when
an application only needs the other providers. The default preserves all existing
providers. Always generate Kotlin and native libraries with the same feature list;
never pair generated code from a different build. Generated sources and binaries
are intentionally ignored by Git. No version bump or publication is needed for
local integration.

## Usage

### Basic Example

```kotlin
import uniffi.lni.*

// Create a Strike node
val config = StrikeConfig(
    apiKey = "your-api-key",
    baseUrl = "https://api.strike.me/v1"
)
val node = StrikeNode(config)

// Get node info
val info = node.getInfo()
println("Node alias: ${info.alias}")

// Create an invoice
val invoiceParams = CreateInvoiceParams(
    invoiceType = InvoiceType.BOLT11,
    amountMsats = 21000L, // 21 sats
    description = "Test invoice"
)
val transaction = node.createInvoice(invoiceParams)
// Render the invoice privately; never log invoices, credentials or preimages.

// Don't forget to clean up
node.close()
```

### Using NWC (Nostr Wallet Connect)

```kotlin
import uniffi.lni.*

val config = NwcConfig(
    nwcUri = "nostr+walletconnect://pubkey?relay=wss://relay.example.com&secret=..."
)
val node = NwcNode(config)

val info = node.getInfo()
println("Connected to: ${info.alias}")

node.close()
```

## Integration with Android

See the `example/` directory for a complete Android example project.

### Building for Android

```bash
./build.sh --release
```

This builds native libraries for all Android targets (arm64-v8a, armeabi-v7a, x86_64, x86) and copies them to the example project's `jniLibs` directory.

To skip Android builds (only generate Kotlin bindings):

```bash
./build.sh --no-android
```

## Publishing a Release

The build script can automatically create a GitHub release with pre-built Android native libraries.

### Prerequisites

1. **Install GitHub CLI:**
   ```bash
   brew install gh
   ```

2. **Authenticate with GitHub:**
   ```bash
   gh auth login
   ```
   Follow the prompts to authenticate with your GitHub account.

3. **Ensure version is updated:**
   The release version is read from `crates/lni/Cargo.toml`. Update the version there before publishing:
   ```toml
   [package]
   version = "0.2.0"  # Update this
   ```

### Create a Release

```bash
./build.sh --publish
```

This will:
1. Build release binaries for all Android architectures
2. Create a zip archive containing all `.so` files
3. Create a GitHub release tagged `v{version}` (e.g., `v0.2.0`)
4. Upload the archive as a release asset

If the release already exists, the script will update the existing asset.

### What Gets Published

The release includes `lni-android-{version}.zip` containing:
- `arm64-v8a/` - ARM64 devices (most modern Android phones)
- `armeabi-v7a/` - ARM32 devices (older phones)
- `x86_64/` - 64-bit emulators
- `x86/` - 32-bit emulators

### Using Pre-built Binaries

Users can download the release and extract to their project:

```bash
# Download from GitHub releases
curl -L https://github.com/lightning-node-interface/lni/releases/download/v0.2.0/lni-android-0.2.0.zip -o lni-android.zip

# Extract to jniLibs
unzip lni-android.zip -d app/src/main/jniLibs/
```

### Important: Invalidate Caches

After updating native libraries, you may need to invalidate Android Studio caches:

**File → Invalidate Caches → Invalidate and Restart**

This ensures Android Studio picks up the updated native libraries.

### Adding to your Android project

1. Copy the generated `lni.kt` file to your project
2. Add the native library (`.so` file) to your `jniLibs` directory
3. Add required dependencies:

```gradle
dependencies {
    implementation "net.java.dev.jna:jna:5.13.0@aar"
    implementation "org.jetbrains.kotlinx:kotlinx-coroutines-core:1.7.3"
}
```

## License

Same license as the main LNI project.

## NWC relay dialing

NWC uses a restricted process-local SOCKS5 dial gate because its upstream pool
has no transport injection hook. The gate only accepts the configured WSS relay
hosts and ports, validates every DNS answer, and opens a concrete public socket.
The original WSS hostname remains the TLS/SNI identity. Request cancellation drops
the gate and its bounded sessions. External SOCKS proxies are rejected instead
of silently bypassing these checks; this path does not support Tor-only relays.

`pollAuthorization` performs a bounded NWA kind-13194 response lookup on the
fixed Alby relays through the same pinned dial gate. It verifies the signed
event, intended application key and authorization time window. The application
must persist the pending wallet-only key privately, enforce one-use completion,
validate the returned relay and authenticate the resulting wallet connection
before offering payments. Hosted callback state and Coinos native origin checks
belong to the integrating application; a relay response never authorizes a payment.

The official Blink GraphQL endpoint uses the same public-address DNS pinning and
verified HTTPS discipline, disables environment proxies and redirects, and bounds
response bodies to 2 MiB. It rejects insecure TLS/proxy overrides and unknown
transaction directions. Configurable generic Galoy endpoints retain their existing
transport options; applications must explicitly constrain those if exposing them.
