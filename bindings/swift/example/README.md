# LNI iOS Example

This is an example iOS application demonstrating how to use the Lightning Node Interface (LNI) Swift bindings with SwiftUI.

## Overview

This example app provides a simple UI for testing various Lightning Network node implementations:

- **Strike** - Strike Lightning API
- **Blink** - Blink Lightning service
- **NWC** - Nostr Wallet Connect
- **CLN** - Core Lightning
- **LND** - Lightning Network Daemon
- **Phoenixd** - Phoenix daemon

## Building & Running

### Prerequisites

- macOS with Xcode 15.0 or later
- Rust toolchain (stable)
- iOS Simulator or physical iOS device

### Step 1: Generate Swift Bindings

From the `bindings/swift` directory:

```bash
./build.sh --release --ios
```

This will:
1. Build the LNI library with UniFFI support
2. Generate Swift bindings in `Sources/LNI/`
3. Build static libraries for iOS Simulator and Device
4. Create an XCFramework

### Step 2: Open in Xcode

```bash
open ./example/LNIExample/LNIExample.xcodeproj
```

### Step 3: Link the LNI Library

1. Drag the generated `LNI.xcframework` from `bindings/swift/` into your Xcode project
2. Add the Swift bindings from `bindings/swift/Sources/LNI/lni.swift` to your project
3. Ensure the framework is set to "Embed & Sign" in Target > General > Frameworks

### Step 4: Run on iOS Simulator

1. Select an iOS Simulator target (e.g., iPhone 15 Pro)
2. Press Cmd+R or click the Run button

## Project Structure

```
LNIExample/
├── LNIExample.xcodeproj/    # Xcode project
└── LNIExample/
    ├── LNIExampleApp.swift  # App entry point
    ├── ContentView.swift    # Main UI
    └── Assets.xcassets/     # App assets
```

## Using the App

1. **Strike Balance**: Enter your Strike API key and tap "Get Balance" to fetch your balance
2. **Node Tests**: Tap on any node button (Strike, Blink, NWC, etc.) to see its configuration options

## Example Code

### Creating a Strike Node

```swift
import LNI

let config = StrikeConfig(
    apiKey: "your-api-key",
    baseUrl: nil,
    httpTimeout: nil,
    socks5Proxy: nil,
    acceptInvalidCerts: nil
)

// Create node using factory function (polymorphic)
let node: LightningNode = createStrikeNode(config: config)

// Get balance
Task {
    do {
        let info = try await node.getInfo()
        print("Balance: \(info.sendBalanceMsat / 1000) sats")
    } catch {
        print("Error: \(error)")
    }
}
```

### Creating an Invoice

```swift
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
```

## Notes

- This example uses placeholder implementations until the LNI library is built and linked
- The actual LNI library usage is commented out in ContentView.swift
- Once the library is linked, uncomment the implementation code

## License

Same license as the main LNI project.
