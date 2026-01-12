//
//  ContentView.swift
//  LNIExample
//
//  Lightning Node Interface (LNI) iOS Example Application
//  This example demonstrates how to use the LNI Swift bindings to interact
//  with various Lightning Network node implementations.
//

import SwiftUI

// MARK: - Spark Invoice Event Callback

final class SparkInvoiceCallback: OnInvoiceEventCallback, @unchecked Sendable {
    private let onUpdate: @Sendable (String, Transaction?) -> Void
    
    init(onUpdate: @escaping @Sendable (String, Transaction?) -> Void) {
        self.onUpdate = onUpdate
    }
    
    func success(transaction: Transaction?) {
        onUpdate("success", transaction)
    }
    
    func pending(transaction: Transaction?) {
        onUpdate("pending", transaction)
    }
    
    func failure(transaction: Transaction?) {
        onUpdate("failure", transaction)
    }
}

// MARK: - Main Content View

struct ContentView: View {
    @State private var output: String = "LNI Swift iOS Example\n\nThis app demonstrates the Lightning Node Interface (LNI) Swift bindings.\n\nEnter your Strike API key above and tap 'Get Balance' to test."
    @State private var isLoading: Bool = false
    @State private var strikeApiKey: String = ""
    @State private var showApiKey: Bool = false
    @State private var use24Words: Bool = false
    @State private var sparkMnemonic: String = ""
    @State private var sparkApiKey: String = ""
    @State private var showSparkMnemonic: Bool = false
    @State private var showSparkApiKey: Bool = false
    
    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(alignment: .leading, spacing: 16) {
                    // Mnemonic Generation Section
                    GroupBox(label: Label("Wallet Utils", systemImage: "key.fill")) {
                        VStack(alignment: .leading, spacing: 12) {
                            Toggle("24 words (default: 12)", isOn: $use24Words)
                            
                            Button {
                                generateNewMnemonic()
                            } label: {
                                HStack {
                                    Image(systemName: "sparkles")
                                    Text("Generate Mnemonic")
                                }
                                .frame(maxWidth: .infinity)
                            }
                            .buttonStyle(.borderedProminent)
                            .disabled(isLoading)
                        }
                        .padding(.vertical, 8)
                    }
                    
                    // Strike API Section
                    GroupBox(label: Label("Strike API", systemImage: "bolt.fill")) {
                        VStack(alignment: .leading, spacing: 12) {
                            HStack {
                                if showApiKey {
                                    TextField("API Key", text: $strikeApiKey)
                                        .textFieldStyle(.roundedBorder)
                                } else {
                                    SecureField("API Key", text: $strikeApiKey)
                                        .textFieldStyle(.roundedBorder)
                                }
                                Button(showApiKey ? "Hide" : "Show") {
                                    showApiKey.toggle()
                                }
                                .buttonStyle(.bordered)
                            }
                            
                            Button {
                                Task {
                                    await getStrikeBalance()
                                }
                            } label: {
                                HStack {
                                    Image(systemName: "arrow.down.circle")
                                    Text("Get Balance")
                                }
                                .frame(maxWidth: .infinity)
                            }
                            .buttonStyle(.borderedProminent)
                            .disabled(isLoading || strikeApiKey.isEmpty)
                        }
                        .padding(.vertical, 8)
                    }
                    
                    // Spark API Section
                    GroupBox(label: Label("Spark (Breez)", systemImage: "sparkles")) {
                        VStack(alignment: .leading, spacing: 12) {
                            HStack {
                                if showSparkMnemonic {
                                    TextField("Mnemonic (12 words)", text: $sparkMnemonic)
                                        .textFieldStyle(.roundedBorder)
                                        .font(.system(.caption, design: .monospaced))
                                } else {
                                    SecureField("Mnemonic (12 words)", text: $sparkMnemonic)
                                        .textFieldStyle(.roundedBorder)
                                }
                                Button(showSparkMnemonic ? "Hide" : "Show") {
                                    showSparkMnemonic.toggle()
                                }
                                .buttonStyle(.bordered)
                            }
                            
                            HStack {
                                if showSparkApiKey {
                                    TextField("Breez API Key", text: $sparkApiKey)
                                        .textFieldStyle(.roundedBorder)
                                        .font(.system(.caption, design: .monospaced))
                                } else {
                                    SecureField("Breez API Key", text: $sparkApiKey)
                                        .textFieldStyle(.roundedBorder)
                                }
                                Button(showSparkApiKey ? "Hide" : "Show") {
                                    showSparkApiKey.toggle()
                                }
                                .buttonStyle(.bordered)
                            }
                            
                            Button {
                                Task {
                                    await testSpark()
                                }
                            } label: {
                                HStack {
                                    Image(systemName: "sparkles")
                                    Text("Test Spark")
                                }
                                .frame(maxWidth: .infinity)
                            }
                            .buttonStyle(.borderedProminent)
                            .disabled(isLoading || sparkMnemonic.isEmpty || sparkApiKey.isEmpty)
                        }
                        .padding(.vertical, 8)
                    }
                    
                    // Test Buttons Section
                    GroupBox(label: Label("Node Tests", systemImage: "testtube.2")) {
                        VStack(spacing: 12) {
                            HStack(spacing: 12) {
                                TestButton(title: "Strike", systemImage: "bolt.fill") {
                                    await testStrike()
                                }
                                .disabled(isLoading)
                                
                                TestButton(title: "Blink", systemImage: "lightbulb.fill") {
                                    await testBlink()
                                }
                                .disabled(isLoading)
                                
                                TestButton(title: "NWC", systemImage: "link") {
                                    await testNwc()
                                }
                                .disabled(isLoading)
                            }
                            
                            HStack(spacing: 12) {
                                TestButton(title: "CLN", systemImage: "server.rack") {
                                    await testCln()
                                }
                                .disabled(isLoading)
                                
                                TestButton(title: "LND", systemImage: "network") {
                                    await testLnd()
                                }
                                .disabled(isLoading)
                                
                                TestButton(title: "Phoenixd", systemImage: "flame.fill") {
                                    await testPhoenixd()
                                }
                                .disabled(isLoading)
                            }
                        }
                        .padding(.vertical, 8)
                    }
                    
                    // Output Section
                    GroupBox(label: Label("Output", systemImage: "terminal")) {
                        if isLoading {
                            HStack {
                                ProgressView()
                                    .padding(.trailing, 8)
                                Text("Loading...")
                            }
                            .frame(maxWidth: .infinity, alignment: .center)
                            .padding()
                        }
                        
                        Text(output)
                            .font(.system(.body, design: .monospaced))
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .padding()
                    }
                }
                .padding()
            }
            .navigationTitle("LNI Example")
        }
    }
    
    // MARK: - Strike Balance
    
    private func getStrikeBalance() async {
        isLoading = true
        output = "=== Strike Balance ===\n\n"
        
        do {
            let config = StrikeConfig(
                apiKey: strikeApiKey
            )
            
            // Use factory function for polymorphic access
            let node: LightningNode = createStrikeNode(config: config)
            
            let info = try await node.getInfo()
            
            let balanceSats = info.sendBalanceMsat / 1000
            let balanceBtc = Double(info.sendBalanceMsat) / 100_000_000_000.0
            
            output += "✓ Connected to Strike!\n\n"
            output += "Balance: \(balanceSats) sats\n"
            output += String(format: "         %.8f BTC\n\n", balanceBtc)
            output += "Network: \(info.network)\n"
            output += "Alias: \(info.alias)\n"
        } catch {
            output += "✗ Error: \(error)\n"
        }
        
        isLoading = false
    }
    
    // MARK: - Generate Mnemonic
    
    private func generateNewMnemonic() {
        output = "=== Generate Mnemonic ===\n\n"
        
        do {
            let wordCount: UInt8? = use24Words ? 24 : nil
            let mnemonic = try generateMnemonic(wordCount: wordCount)
            
            let words = mnemonic.split(separator: " ")
            output += "✓ Generated \(words.count)-word mnemonic:\n\n"
            
            // Display words in a numbered list
            for (index, word) in words.enumerated() {
                output += String(format: "%2d. %@\n", index + 1, String(word))
            }
            
            output += "\n⚠️ IMPORTANT: In a real app, never display\n"
            output += "   the mnemonic on screen. Store it securely!\n"
        } catch {
            output += "✗ Error: \(error)\n"
        }
    }
    
    // MARK: - Test Functions
    
    private func testSpark() async {
        isLoading = true
        output = "=== Spark Node Test ===\n\n"
        
        do {
            // Get the documents directory for storage
            guard let documentsDir = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask).first else {
                output += "✗ Error: Could not get documents directory\n"
                isLoading = false
                return
            }
            
            let storageDir = documentsDir.appendingPathComponent("spark_data").path
            output += "Storage: \(storageDir)\n\n"
            
            let config = SparkConfig(
                mnemonic: sparkMnemonic,
                passphrase: nil,
                apiKey: sparkApiKey,
                storageDir: storageDir,
                network: "mainnet"
            )
            
            output += "(1) Creating SparkNode...\n"
            let node: LightningNode = try await createSparkNode(config: config)
            output += "✓ SparkNode connected!\n\n"
            
            output += "(2) Getting node info...\n"
            let info = try await node.getInfo()
            output += "✓ Node Info:\n"
            output += "  • Alias: \(info.alias)\n"
            output += "  • Network: \(info.network)\n"
            output += "  • Balance: \(info.sendBalanceMsat / 1000) sats\n\n"
            
            output += "(3) Creating invoice...\n"
            let invoiceParams = CreateInvoiceParams(
                amountMsats: 1000,
                description: "test invoice from Spark iOS",
                expiry: 3600
            )
            let invoice = try await node.createInvoice(params: invoiceParams)
            let shortHash = String(invoice.paymentHash.prefix(20))
            output += "✓ Invoice created: \(shortHash)...\n\n"
            
            output += "(4) Testing onInvoiceEvents...\n"
            output += "  Polling for payment (will timeout after 6s)...\n"
            
            let eventParams = OnInvoiceEventParams(
                paymentHash: "d742679487d0f01b3f8d9c4a6ceea12b70c99c0965f732584f337bd172bb81cb",
                search: nil,
                pollingDelaySec: 2,
                maxPollingSec: 6
            )
            
            let callback = SparkInvoiceCallback { [self] status, tx in
                Task { @MainActor in
                    self.output += "  • Event: \(status)"
                    if let tx = tx {
                        self.output += " (amount: \(tx.amountMsats / 1000) sats)"
                    }
                    self.output += "\n"
                }
            }
            
            await node.onInvoiceEvents(params: eventParams, callback: callback)
            output += "✓ onInvoiceEvents completed\n\n"
            
            output += "(5) Listing transactions...\n"
            let listParams = ListTransactionsParams(
                from: 0,
                limit: 3,
                paymentHash: nil,
                search: nil
            )
            let txns = try await node.listTransactions(params: listParams)
            output += "✓ Found \(txns.count) transactions\n\n"
            
            output += "=== All tests passed! ===\n"
        } catch {
            output += "✗ Error: \(error)\n"
        }
        
        isLoading = false
    }
    
    private func testStrike() async {
        isLoading = true
        output = "=== Strike Node Test ===\n\n"
        
        // Placeholder implementation
        output += "Strike Node Configuration:\n"
        output += "  • API Key: Required\n"
        output += "  • Base URL: https://api.strike.me/v1\n"
        output += "  • SOCKS5 Proxy: Optional\n"
        output += "  • Accept Invalid Certs: false\n"
        output += "  • HTTP Timeout: 60s\n\n"
        output += "Available Methods:\n"
        output += "  • getInfo()\n"
        output += "  • createInvoice(params:)\n"
        output += "  • payInvoice(params:)\n"
        output += "  • lookupInvoice(params:)\n"
        output += "  • listTransactions(params:)\n"
        output += "  • onInvoiceEvents(params:callback:)\n"
        
        isLoading = false
    }
    
    private func testBlink() async {
        isLoading = true
        output = "=== Blink Node Test ===\n\n"
        
        output += "Blink Node Configuration:\n"
        output += "  • API Key: Required\n"
        output += "  • Base URL: https://api.blink.sv/graphql\n"
        output += "  • HTTP Timeout: 60s\n\n"
        output += "Available Methods:\n"
        output += "  • getInfo()\n"
        output += "  • createInvoice(params:)\n"
        output += "  • payInvoice(params:)\n"
        output += "  • decode(str:)\n"
        
        isLoading = false
    }
    
    private func testNwc() async {
        isLoading = true
        output = "=== NWC Node Test ===\n\n"
        
        output += "NWC (Nostr Wallet Connect) Configuration:\n"
        output += "  • NWC URI: Required\n"
        output += "    Format: nostr+walletconnect://pubkey?relay=...&secret=...\n\n"
        output += "Available Methods:\n"
        output += "  • getInfo()\n"
        output += "  • createInvoice(params:)\n"
        output += "  • payInvoice(params:)\n"
        output += "  • lookupInvoice(params:)\n"
        
        isLoading = false
    }
    
    private func testCln() async {
        isLoading = true
        output = "=== CLN Node Test ===\n\n"
        
        output += "Core Lightning (CLN) Configuration:\n"
        output += "  • Rune: Required\n"
        output += "  • Base URL: Required (REST API endpoint)\n"
        output += "  • HTTP Timeout: 60s\n\n"
        output += "Available Methods:\n"
        output += "  • getInfo()\n"
        output += "  • createInvoice(params:)\n"
        output += "  • payInvoice(params:)\n"
        output += "  • createOffer(params:)\n"
        output += "  • listOffers(search:)\n"
        output += "  • payOffer(offer:amountMsats:payerNote:)\n"
        
        isLoading = false
    }
    
    private func testLnd() async {
        isLoading = true
        output = "=== LND Node Test ===\n\n"
        
        output += "LND Configuration:\n"
        output += "  • Macaroon: Required (hex encoded)\n"
        output += "  • Base URL: Required (REST API endpoint)\n"
        output += "  • HTTP Timeout: 60s\n\n"
        output += "Available Methods:\n"
        output += "  • getInfo()\n"
        output += "  • createInvoice(params:)\n"
        output += "  • payInvoice(params:)\n"
        output += "  • lookupInvoice(params:)\n"
        output += "  • listTransactions(params:)\n"
        
        isLoading = false
    }
    
    private func testPhoenixd() async {
        isLoading = true
        output = "=== Phoenixd Node Test ===\n\n"
        
        output += "Phoenixd Configuration:\n"
        output += "  • Password: Required\n"
        output += "  • Base URL: Required (e.g., http://localhost:9740)\n"
        output += "  • HTTP Timeout: 60s\n\n"
        output += "Available Methods:\n"
        output += "  • getInfo()\n"
        output += "  • createInvoice(params:)\n"
        output += "  • payInvoice(params:)\n"
        output += "  • lookupInvoice(params:)\n"
        output += "  • listTransactions(params:)\n"
        
        isLoading = false
    }
}

// MARK: - Test Button Component

struct TestButton: View {
    let title: String
    let systemImage: String
    let action: () async -> Void
    
    var body: some View {
        Button {
            Task {
                await action()
            }
        } label: {
            VStack {
                Image(systemName: systemImage)
                    .font(.title2)
                Text(title)
                    .font(.caption)
            }
            .frame(maxWidth: .infinity)
            .padding(.vertical, 8)
        }
        .buttonStyle(.bordered)
    }
}

// MARK: - Preview

#Preview {
    ContentView()
}
