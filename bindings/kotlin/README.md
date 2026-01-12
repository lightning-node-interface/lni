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

## Building

### Prerequisites

- Rust toolchain (stable)
- Cargo

### Generate Kotlin bindings

```bash
./build.sh --release
```

This will:
1. Build the LNI library with UniFFI support
2. Generate Kotlin bindings in `src/main/kotlin/uniffi/lni/`

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
println("Invoice: ${transaction.invoice}")

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
./build.sh --release --no-android
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
