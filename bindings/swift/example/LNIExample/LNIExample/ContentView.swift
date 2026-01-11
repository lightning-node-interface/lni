//
//  ContentView.swift
//  LNIExample
//
//  Lightning Node Interface (LNI) iOS Example Application
//  This example demonstrates how to use the LNI Swift bindings to interact
//  with various Lightning Network node implementations.
//

import SwiftUI

// MARK: - Main Content View

struct ContentView: View {
    @State private var output: String = "LNI Swift iOS Example\n\nThis app demonstrates the Lightning Node Interface (LNI) Swift bindings.\n\nEnter your Strike API key above and tap 'Get Balance' to test."
    @State private var isLoading: Bool = false
    @State private var strikeApiKey: String = ""
    @State private var showApiKey: Bool = false
    @State private var use24Words: Bool = false
    
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
