/**
 * LNI Kotlin Example
 * 
 * This example demonstrates how to use the LNI Kotlin bindings to interact
 * with various Lightning Network node implementations.
 */
package com.lni.example

import uniffi.lni.*
import kotlinx.coroutines.runBlocking

fun main() = runBlocking {
    println("=== LNI Kotlin Example ===")
    println()
    
    // Example 1: Strike Node
    println("--- Strike Node Example ---")
    strikeExample()
    
    // Example 2: NWC Node
    println()
    println("--- NWC Node Example ---")
    nwcExample()
    
    // Example 3: Blink Node
    println()
    println("--- Blink Node Example ---")
    blinkExample()
    
    println()
    println("=== Examples Complete ===")
}

/**
 * Example using Strike Lightning API
 */
suspend fun strikeExample() {
    // Create Strike configuration
    // In a real application, you would use your actual API key
    val config = StrikeConfig(
        apiKey = "your-strike-api-key",
        baseUrl = "https://api.strike.me/v1",
        socks5Proxy = null,
        acceptInvalidCerts = false,
        httpTimeout = 60L
    )
    
    // Create the Strike node instance
    val node = StrikeNode(config)
    
    try {
        // Get node info
        println("Getting node info...")
        val info = node.getInfo()
        println("  Alias: ${info.alias}")
        println("  Pubkey: ${info.pubkey}")
        println("  Network: ${info.network}")
        println("  Send Balance (msat): ${info.sendBalanceMsat}")
        println("  Receive Balance (msat): ${info.receiveBalanceMsat}")
        
        // Create an invoice
        println()
        println("Creating invoice...")
        val invoiceParams = CreateInvoiceParams(
            invoiceType = InvoiceType.BOLT11,
            amountMsats = 21000L, // 21 sats
            offer = null,
            description = "Test invoice from LNI Kotlin",
            descriptionHash = null,
            expiry = 3600L,
            rPreimage = null,
            isBlinded = false,
            isKeysend = false,
            isAmp = false,
            isPrivate = false
        )
        val transaction = node.createInvoice(invoiceParams)
        println("  Invoice: ${transaction.invoice.take(50)}...")
        println("  Payment Hash: ${transaction.paymentHash}")
        println("  Amount (msat): ${transaction.amountMsats}")
        
        // List transactions
        println()
        println("Listing transactions...")
        val listParams = ListTransactionsParams(
            from = 0L,
            limit = 10L,
            paymentHash = null,
            search = null
        )
        val transactions = node.listTransactions(listParams)
        println("  Found ${transactions.size} transactions")
        for (tx in transactions.take(3)) {
            println("    - ${tx.type}: ${tx.amountMsats} msat (${tx.paymentHash.take(16)}...)")
        }
        
        // Test on_invoice_events callback
        println()
        println("Testing invoice event callback...")
        onInvoiceEventsExample(node, transaction.paymentHash)
        
    } catch (e: ApiException) {
        println("  API Error: ${e.message}")
        println("  (This is expected if you don't have a valid API key)")
    } finally {
        // Clean up resources
        node.close()
    }
}

/**
 * Example using Nostr Wallet Connect (NWC)
 */
suspend fun nwcExample() {
    // Create NWC configuration
    // In a real application, you would use your actual NWC URI
    val config = NwcConfig(
        nwcUri = "nostr+walletconnect://pubkey?relay=wss://relay.example.com&secret=...",
        socks5Proxy = null,
        acceptInvalidCerts = true,
        httpTimeout = 60L
    )
    
    // Create the NWC node instance
    val node = NwcNode(config)
    
    try {
        // Get node info
        println("Getting node info...")
        val info = node.getInfo()
        println("  Alias: ${info.alias}")
        println("  Pubkey: ${info.pubkey}")
        
        // Create an invoice
        println()
        println("Creating invoice...")
        val invoiceParams = CreateInvoiceParams(
            invoiceType = InvoiceType.BOLT11,
            amountMsats = 1000L, // 1 sat
            offer = null,
            description = "NWC test invoice",
            descriptionHash = null,
            expiry = 3600L,
            rPreimage = null,
            isBlinded = false,
            isKeysend = false,
            isAmp = false,
            isPrivate = false
        )
        val transaction = node.createInvoice(invoiceParams)
        println("  Invoice created: ${transaction.invoice.take(50)}...")
        
    } catch (e: ApiException) {
        println("  API Error: ${e.message}")
        println("  (This is expected if you don't have a valid NWC URI)")
    } finally {
        node.close()
    }
}

/**
 * Example using Blink Lightning API
 */
suspend fun blinkExample() {
    // Create Blink configuration
    val config = BlinkConfig(
        apiKey = "your-blink-api-key",
        baseUrl = "https://api.blink.sv/graphql",
        socks5Proxy = null,
        acceptInvalidCerts = true,
        httpTimeout = 60L
    )
    
    // Create the Blink node instance
    val node = BlinkNode(config)
    
    try {
        // Get node info
        println("Getting node info...")
        val info = node.getInfo()
        println("  Alias: ${info.alias}")
        println("  Network: ${info.network}")
        println("  Block Height: ${info.blockHeight}")
        
        // Decode a BOLT11 invoice
        println()
        println("Decoding invoice...")
        val testInvoice = "lnbc1..."  // Replace with a real invoice
        val decoded = node.decode(testInvoice)
        println("  Decoded: ${decoded.take(100)}...")
        
    } catch (e: ApiException) {
        println("  API Error: ${e.message}")
        println("  (This is expected if you don't have a valid API key)")
    } finally {
        node.close()
    }
}

/**
 * Example showing how to pay an invoice
 */
suspend fun payInvoiceExample(node: StrikeNode, invoice: String) {
    println("Paying invoice...")
    
    val payParams = PayInvoiceParams(
        invoice = invoice,
        feeLimitMsat = 1000L,  // Max 1 sat fee
        feeLimitPercentage = null,
        timeoutSeconds = 60L,
        amountMsats = null,  // Use amount from invoice
        maxParts = null,
        firstHopPubkey = null,
        lastHopPubkey = null,
        allowSelfPayment = false,
        isAmp = false
    )
    
    val response = node.payInvoice(payParams)
    println("  Payment successful!")
    println("  Payment Hash: ${response.paymentHash}")
    println("  Preimage: ${response.preimage}")
    println("  Fee (msat): ${response.feeMsats}")
}

/**
 * Example showing callback interface usage for invoice events
 */
suspend fun onInvoiceEventsExample(node: StrikeNode, paymentHash: String) {
    println("Listening for invoice events...")
    
    // Create a callback implementation
    val callback = object : OnInvoiceEventCallback {
        override fun success(transaction: Transaction?) {
            println("  ✓ Invoice SUCCESS!")
            transaction?.let {
                println("    Payment Hash: ${it.paymentHash}")
                println("    Amount (msat): ${it.amountMsats}")
                println("    Settled At: ${it.settledAt}")
            }
        }
        
        override fun pending(transaction: Transaction?) {
            println("  ⏳ Invoice PENDING...")
            transaction?.let {
                println("    Payment Hash: ${it.paymentHash}")
                println("    Amount (msat): ${it.amountMsats}")
            }
        }
        
        override fun failure(transaction: Transaction?) {
            println("  ✗ Invoice FAILURE")
            transaction?.let {
                println("    Payment Hash: ${it.paymentHash}")
            }
        }
    }
    
    val params = OnInvoiceEventParams(
        paymentHash = paymentHash,
        search = null,
        pollingDelaySec = 2L,
        maxPollingSec = 10L
    )
    
    // This will poll for invoice status and call the appropriate callback method
    node.onInvoiceEvents(params, callback)
    println("  Invoice event monitoring complete.")
}

/**
 * Example showing how to lookup an invoice by payment hash
 */
suspend fun lookupInvoiceExample(node: StrikeNode, paymentHash: String) {
    println("Looking up invoice...")
    
    val lookupParams = LookupInvoiceParams(
        paymentHash = paymentHash,
        search = null
    )
    
    val transaction = node.lookupInvoice(lookupParams)
    println("  Type: ${transaction.type}")
    println("  Amount (msat): ${transaction.amountMsats}")
    println("  Settled: ${if (transaction.settledAt > 0) "Yes" else "No"}")
    if (transaction.settledAt > 0) {
        println("  Settled At: ${transaction.settledAt}")
    }
}
