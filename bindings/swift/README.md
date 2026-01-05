# LNI Swift Bindings

Swift bindings for the Lightning Node Interface (LNI) library, generated using UniFFI.

## Overview

This package provides Swift bindings for LNI, allowing you to interact with various Lightning Network node implementations from Swift/iOS applications.

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
- Xcode (for iOS builds)
- iOS SDK (comes with Xcode)

### Generate Swift bindings

```bash
./build.sh --release
```

This will:
1. Build the LNI library with UniFFI support
2. Generate Swift bindings in `Sources/LNI/`

### Build for iOS

```bash
./build.sh --release --ios
```

This will additionally:
1. Build static libraries for iOS Simulator (arm64 + x86_64)
2. Build static library for iOS devices (arm64)
3. Create a universal XCFramework

## Usage

### Basic Example

```swift
import LNI

// Create a Strike node
let config = StrikeConfig(
    apiKey: "your-api-key",
    baseUrl: "https://api.strike.me/v1",
    socks5Proxy: nil,
    acceptInvalidCerts: false,
    httpTimeout: 60
)

do {
    let node = StrikeNode(config: config)
    
    // Get node info
    let info = try await node.getInfo()
    print("Node alias: \(info.alias)")
    
    // Create an invoice
    let invoiceParams = CreateInvoiceParams(
        invoiceType: .bolt11,
        amountMsats: 21000, // 21 sats
        offer: nil,
        description: "Test invoice",
        descriptionHash: nil,
        expiry: 3600,
        rPreimage: nil,
        isBlinded: false,
        isKeysend: false,
        isAmp: false,
        isPrivate: false
    )
    let transaction = try await node.createInvoice(params: invoiceParams)
    print("Invoice: \(transaction.invoice)")
    
} catch {
    print("Error: \(error)")
}
```

### Using NWC (Nostr Wallet Connect)

```swift
import LNI

let config = NwcConfig(
    nwcUri: "nostr+walletconnect://pubkey?relay=wss://relay.example.com&secret=..."
)

do {
    let node = NwcNode(config: config)
    let info = try await node.getInfo()
    print("Connected to: \(info.alias)")
} catch {
    print("Error: \(error)")
}
```

### Polymorphic Usage

You can use the factory functions to create nodes as the `LightningNode` protocol:

```swift
import LNI

// Create different node types using factory functions
let strikeNode: LightningNode = createStrikeNode(config: strikeConfig)
let blinkNode: LightningNode = createBlinkNode(config: blinkConfig)
let nwcNode: LightningNode = createNwcNode(config: nwcConfig)

// All nodes share the same interface
for node in [strikeNode, blinkNode, nwcNode] {
    let info = try await node.getInfo()
    print("Balance: \(info.sendBalanceMsat) msats")
}
```

## Integration with iOS

See the `example/` directory for a complete iOS example project that runs on the iOS Simulator.

### Adding to your iOS project

1. Copy the generated Swift files from `Sources/LNI/` to your project
2. Add the static library or XCFramework to your project
3. Link against the library in your build settings

### Using Swift Package Manager (future)

We plan to add SPM support in a future release.

## License

Same license as the main LNI project.
