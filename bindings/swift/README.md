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

### Build for iOS

```bash
./build.sh --release
```

This will:
1. Build the LNI library with UniFFI support
2. Generate Swift bindings in `Sources/LNI/`
3. Build static libraries for iOS Simulator (arm64 + x86_64)
4. Build static library for iOS devices (arm64)
5. Create a universal XCFramework

To skip iOS builds (only generate Swift bindings):

```bash
./build.sh --release --no-ios
```

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

### Using Swift Package Manager

Add LNI to your project using Swift Package Manager:

1. In Xcode, go to **File > Add Package Dependencies...**
2. Enter the repository URL: `https://github.com/lightning-node-interface/lni`
3. Select the version you want to use
4. Click **Add Package**

Or add it to your `Package.swift`:

```swift
dependencies: [
    .package(url: "https://github.com/lightning-node-interface/lni", from: "0.1.0")
]
```

And add it to your target:

```swift
.target(
    name: "YourTarget",
    dependencies: [
        .product(name: "LNI", package: "lni")
    ]
)
```

### Local Development with SPM

For local development, you can build and use the XCFramework locally:

1. Build the iOS targets:
   ```bash
   cd bindings/swift
   ./build.sh --release --ios
   ```

2. Rename the XCFramework for SPM (SPM requires it to match the binary target name `lniFFI`):
   ```bash
   mv LNI.xcframework lniFFI.xcframework
   ```

3. Modify `Package.swift` to use the local binary target instead of the remote URL:
   ```swift
   // Comment out the remote binary target:
   // .binaryTarget(
   //     name: "lniFFI",
   //     url: "https://github.com/lightning-node-interface/lni/releases/download/v0.1.0/lniFFI.xcframework.zip",
   //     checksum: "SHA256_CHECKSUM_HERE"
   // )
   
   // Add local binary target:
   .binaryTarget(name: "lniFFI", path: "lniFFI.xcframework")
   ```

## Publishing a Release

To publish a new release for SPM distribution:

### 1. Build and Package

```bash
cd bindings/swift
./build.sh --release --ios --package
```

This will:
- Build the XCFramework for iOS devices and simulators
- Create `lniFFI.xcframework.zip`
- Calculate the SHA256 checksum
- Automatically update `Package.swift` with the new checksum

### 2. Update Version (if needed)

If releasing a new version, update the URL in `Package.swift`:

```swift
.binaryTarget(
    name: "lniFFI",
    url: "https://github.com/lightning-node-interface/lni/releases/download/vX.Y.Z/lniFFI.xcframework.zip",
    checksum: "..."
)
```

### 3. Create GitHub Release and Upload

```bash
# Create a new release and upload the zip file
gh release create vX.Y.Z lniFFI.xcframework.zip --title "vX.Y.Z" --notes "Release notes here"

# Or upload to an existing release
gh release upload vX.Y.Z lniFFI.xcframework.zip
```

### 4. Commit and Push

```bash
git add Package.swift
git commit -m "Release vX.Y.Z"
git push
```

### Manual Integration (without SPM)

If you prefer not to use SPM:

1. Copy the generated Swift files from `Sources/LNI/` to your project
2. Add the static library or XCFramework to your project
3. Link against the library in your build settings

## License

Same license as the main LNI project.
