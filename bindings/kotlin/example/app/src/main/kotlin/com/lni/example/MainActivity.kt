package com.lni.example

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import uniffi.lni.*

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContent {
            MaterialTheme {
                Surface(
                    modifier = Modifier.fillMaxSize(),
                    color = MaterialTheme.colorScheme.background
                ) {
                    LniExampleScreen()
                }
            }
        }
    }
}

@Composable
fun LniExampleScreen() {
    var output by remember { mutableStateOf("LNI Kotlin Android Example\n\nEnter your Strike API key and tap 'Get Balance' to test.") }
    var isLoading by remember { mutableStateOf(false) }
    var strikeApiKey by remember { mutableStateOf("") }
    var showApiKey by remember { mutableStateOf(false) }
    
    // Invoice monitoring state
    var invoiceStatus by remember { mutableStateOf<InvoiceStatus?>(null) }
    var currentInvoice by remember { mutableStateOf<String?>(null) }
    var isMonitoring by remember { mutableStateOf(false) }
    
    val scope = rememberCoroutineScope()
    val scrollState = rememberScrollState()

    Column(
        modifier = Modifier
            .fillMaxSize()
            .padding(16.dp)
    ) {
        Text(
            text = "LNI Android Example",
            style = MaterialTheme.typography.headlineMedium,
            modifier = Modifier.padding(bottom = 16.dp)
        )

        // Strike API Key Section
        Card(
            modifier = Modifier
                .fillMaxWidth()
                .padding(bottom = 16.dp)
        ) {
            Column(
                modifier = Modifier.padding(16.dp)
            ) {
                Text(
                    text = "Strike API",
                    style = MaterialTheme.typography.titleMedium,
                    modifier = Modifier.padding(bottom = 8.dp)
                )

                OutlinedTextField(
                    value = strikeApiKey,
                    onValueChange = { strikeApiKey = it },
                    label = { Text("API Key") },
                    placeholder = { Text("Enter your Strike API key") },
                    modifier = Modifier.fillMaxWidth(),
                    singleLine = true,
                    visualTransformation = if (showApiKey) VisualTransformation.None else PasswordVisualTransformation(),
                    trailingIcon = {
                        TextButton(onClick = { showApiKey = !showApiKey }) {
                            Text(if (showApiKey) "Hide" else "Show")
                        }
                    }
                )

                Spacer(modifier = Modifier.height(12.dp))

                Button(
                    onClick = {
                        scope.launch {
                            isLoading = true
                            output = getStrikeBalance(strikeApiKey)
                            isLoading = false
                        }
                    },
                    enabled = !isLoading && strikeApiKey.isNotBlank(),
                    modifier = Modifier.fillMaxWidth()
                ) {
                    Text("Get Balance")
                }
            }
        }

        // Invoice Monitoring Section
        Card(
            modifier = Modifier
                .fillMaxWidth()
                .padding(bottom = 16.dp)
        ) {
            Column(
                modifier = Modifier.padding(16.dp)
            ) {
                Text(
                    text = "Invoice Monitor (Real-time)",
                    style = MaterialTheme.typography.titleMedium,
                    modifier = Modifier.padding(bottom = 8.dp)
                )

                // Status indicator
                invoiceStatus?.let { status ->
                    InvoiceStatusCard(status = status, invoice = currentInvoice)
                    Spacer(modifier = Modifier.height(8.dp))
                }

                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.spacedBy(8.dp)
                ) {
                    Button(
                        onClick = {
                            scope.launch {
                                isMonitoring = true
                                invoiceStatus = InvoiceStatus.Creating
                                createAndMonitorInvoice(
                                    apiKey = strikeApiKey,
                                    onInvoiceCreated = { invoice, paymentHash ->
                                        currentInvoice = invoice
                                        invoiceStatus = InvoiceStatus.Pending(paymentHash)
                                    },
                                    onStatusUpdate = { status ->
                                        invoiceStatus = status
                                        if (status is InvoiceStatus.Success || status is InvoiceStatus.Failed) {
                                            isMonitoring = false
                                        }
                                    },
                                    onError = { error ->
                                        invoiceStatus = InvoiceStatus.Error(error)
                                        isMonitoring = false
                                    }
                                )
                            }
                        },
                        enabled = !isMonitoring && strikeApiKey.isNotBlank(),
                        modifier = Modifier.weight(1f)
                    ) {
                        Text("Create & Monitor Invoice")
                    }

                    if (isMonitoring) {
                        OutlinedButton(
                            onClick = {
                                isMonitoring = false
                                invoiceStatus = InvoiceStatus.Cancelled
                            }
                        ) {
                            Text("Cancel")
                        }
                    }
                }

                if (isMonitoring) {
                    Spacer(modifier = Modifier.height(8.dp))
                    LinearProgressIndicator(
                        modifier = Modifier.fillMaxWidth()
                    )
                    Text(
                        text = "Monitoring for payment...",
                        style = MaterialTheme.typography.bodySmall,
                        modifier = Modifier.padding(top = 4.dp)
                    )
                }
            }
        }

        // Other Tests Section
        Text(
            text = "Other Tests",
            style = MaterialTheme.typography.titleSmall,
            modifier = Modifier.padding(bottom = 8.dp)
        )

        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(8.dp)
        ) {
            Button(
                onClick = {
                    scope.launch {
                        isLoading = true
                        output = testStrike()
                        isLoading = false
                    }
                },
                enabled = !isLoading
            ) {
                Text("Strike")
            }

            Button(
                onClick = {
                    scope.launch {
                        isLoading = true
                        output = testBlink()
                        isLoading = false
                    }
                },
                enabled = !isLoading
            ) {
                Text("Blink")
            }

            Button(
                onClick = {
                    scope.launch {
                        isLoading = true
                        output = testNwc()
                        isLoading = false
                    }
                },
                enabled = !isLoading
            ) {
                Text("NWC")
            }
        }

        Spacer(modifier = Modifier.height(16.dp))

        if (isLoading) {
            CircularProgressIndicator(
                modifier = Modifier.align(Alignment.CenterHorizontally)
            )
        }

        Spacer(modifier = Modifier.height(16.dp))

        Card(
            modifier = Modifier
                .fillMaxWidth()
                .weight(1f)
        ) {
            Text(
                text = output,
                modifier = Modifier
                    .padding(16.dp)
                    .verticalScroll(scrollState),
                style = MaterialTheme.typography.bodyMedium
            )
        }
    }
}

suspend fun getStrikeBalance(apiKey: String): String = withContext(Dispatchers.IO) {
    val sb = StringBuilder()
    sb.appendLine("=== Strike Balance (Polymorphic) ===\n")

    try {
        val config = StrikeConfig(
            apiKey = apiKey,
            baseUrl = null,
            httpTimeout = null,
            socks5Proxy = null,
            acceptInvalidCerts = null
        )
        
        // Use the factory function to get a polymorphic LightningNode
        // This allows you to treat all node types uniformly!
        val node: LightningNode = createStrikeNode(config)

        sb.appendLine("Fetching balance via LightningNode interface...")

        val info = node.getInfo()
        
        // Convert from millisats to sats for display
        val balanceSats = info.sendBalanceMsat / 1000
        val balanceBtc = info.sendBalanceMsat / 100_000_000_000.0
        
        sb.appendLine("✓ Connected to Strike!\n")
        sb.appendLine("Balance: $balanceSats sats")
        sb.appendLine("         ${String.format("%.8f", balanceBtc)} BTC")
        sb.appendLine("\nNetwork: ${info.network}")
        sb.appendLine("Alias: ${info.alias}")

    } catch (e: ApiException) {
        sb.appendLine("✗ API Error: ${e.message}")
    } catch (e: Exception) {
        sb.appendLine("✗ Error: ${e.message}")
    }

    sb.toString()
}

suspend fun testStrike(): String = withContext(Dispatchers.IO) {
    val sb = StringBuilder()
    sb.appendLine("=== Strike Node Test (Polymorphic) ===\n")

    try {
        val config = StrikeConfig(
            apiKey = "test_api_key",
            baseUrl = null,
            httpTimeout = null,
            socks5Proxy = null,
            acceptInvalidCerts = null
        )
        
        // Use factory function for polymorphic access
        val node: LightningNode = createStrikeNode(config)

        sb.appendLine("Strike node created via createStrikeNode()!")
        sb.appendLine("Using LightningNode interface...")

        val info = node.getInfo()
        sb.appendLine("Pubkey: ${info.pubkey}")
        sb.appendLine("Alias: ${info.alias}")

    } catch (e: ApiException) {
        sb.appendLine("API Error: ${e.message}")
        sb.appendLine("(Expected if no valid API key)")
    } catch (e: Exception) {
        sb.appendLine("Error: ${e.message}")
    }

    sb.toString()
}

suspend fun testBlink(): String = withContext(Dispatchers.IO) {
    val sb = StringBuilder()
    sb.appendLine("=== Blink Node Test (Polymorphic) ===\n")

    try {
        val config = BlinkConfig(
            apiKey = "test_api_key",
            baseUrl = null,
            httpTimeout = null
        )
        
        // Use factory function for polymorphic access
        val node: LightningNode = createBlinkNode(config)

        sb.appendLine("Blink node created via createBlinkNode()!")
        sb.appendLine("Using LightningNode interface...")

        val info = node.getInfo()
        sb.appendLine("Pubkey: ${info.pubkey}")
        sb.appendLine("Alias: ${info.alias}")

    } catch (e: ApiException) {
        sb.appendLine("API Error: ${e.message}")
        sb.appendLine("(Expected if no valid API key)")
    } catch (e: Exception) {
        sb.appendLine("Error: ${e.message}")
    }

    sb.toString()
}

suspend fun testNwc(): String = withContext(Dispatchers.IO) {
    val sb = StringBuilder()
    sb.appendLine("=== NWC Node Test (Polymorphic) ===\n")

    try {
        val config = NwcConfig(
            nwcUri = "nostr+walletconnect://test"
        )
        
        // Use factory function for polymorphic access
        val node: LightningNode = createNwcNode(config)

        sb.appendLine("NWC node created via createNwcNode()!")
        sb.appendLine("Using LightningNode interface...")

        val info = node.getInfo()
        sb.appendLine("Pubkey: ${info.pubkey}")
        sb.appendLine("Alias: ${info.alias}")

    } catch (e: ApiException) {
        sb.appendLine("API Error: ${e.message}")
        sb.appendLine("(Expected if no valid NWC URI)")
    } catch (e: Exception) {
        sb.appendLine("Error: ${e.message}")
    }

    sb.toString()
}

/**
 * Demonstrates the power of polymorphism - this function works with ANY node type!
 * You can pass a Strike, Blink, NWC, Phoenixd, CLN, LND, or Speed node.
 */
suspend fun getBalanceFromAnyNode(node: LightningNode): String = withContext(Dispatchers.IO) {
    val sb = StringBuilder()
    sb.appendLine("=== Polymorphic Balance Check ===\n")

    try {
        val info = node.getInfo()
        
        val balanceSats = info.sendBalanceMsat / 1000
        sb.appendLine("Node: ${info.alias}")
        sb.appendLine("Balance: $balanceSats sats")
        sb.appendLine("Network: ${info.network}")
        
    } catch (e: Exception) {
        sb.appendLine("Error: ${e.message}")
    }

    sb.toString()
}

/**
 * Example: Creating multiple nodes and treating them uniformly
 */
suspend fun demonstratePolymorphism(): String = withContext(Dispatchers.IO) {
    val sb = StringBuilder()
    sb.appendLine("=== Polymorphism Demo ===\n")
    sb.appendLine("Creating nodes of different types,")
    sb.appendLine("all as LightningNode interface:\n")

    // Create different node types using factory functions
    val nodes: List<Pair<String, () -> LightningNode>> = listOf(
        "Strike" to {
            createStrikeNode(StrikeConfig(
                apiKey = "demo_key",
                baseUrl = null,
                httpTimeout = null,
                socks5Proxy = null,
                acceptInvalidCerts = null
            ))
        },
        "Blink" to {
            createBlinkNode(BlinkConfig(
                apiKey = "demo_key",
                baseUrl = null,
                httpTimeout = null
            ))
        },
        "Speed" to {
            createSpeedNode(SpeedConfig(
                apiKey = "demo_key",
                baseUrl = null,
                httpTimeout = null
            ))
        }
    )

    for ((name, createNode) in nodes) {
        sb.appendLine("✓ Created $name as LightningNode")
    }

    sb.appendLine("\nAll nodes share the same interface!")
    sb.appendLine("Methods available on all nodes:")
    sb.appendLine("  • getInfo()")
    sb.appendLine("  • createInvoice(params)")
    sb.appendLine("  • payInvoice(params)")
    sb.appendLine("  • lookupInvoice(params)")
    sb.appendLine("  • listTransactions(params)")
    sb.appendLine("  • onInvoiceEvents(params, callback)")
    sb.appendLine("  • decode(str)")
    sb.appendLine("  • createOffer(params)")
    sb.appendLine("  • getOffer(search)")
    sb.appendLine("  • listOffers(search)")
    sb.appendLine("  • payOffer(...)")

    sb.toString()
}

// Invoice status sealed class for UI state
sealed class InvoiceStatus {
    object Creating : InvoiceStatus()
    data class Pending(val paymentHash: String) : InvoiceStatus()
    data class Success(val transaction: Transaction?) : InvoiceStatus()
    data class Failed(val transaction: Transaction?) : InvoiceStatus()
    data class Error(val message: String) : InvoiceStatus()
    object Cancelled : InvoiceStatus()
}

@Composable
fun InvoiceStatusCard(status: InvoiceStatus, invoice: String?) {
    val (icon, text, color) = when (status) {
        is InvoiceStatus.Creating -> Triple("⏳", "Creating invoice...", MaterialTheme.colorScheme.primary)
        is InvoiceStatus.Pending -> Triple("💰", "Waiting for payment", MaterialTheme.colorScheme.tertiary)
        is InvoiceStatus.Success -> Triple("✅", "Payment received!", MaterialTheme.colorScheme.primary)
        is InvoiceStatus.Failed -> Triple("❌", "Invoice expired or failed", MaterialTheme.colorScheme.error)
        is InvoiceStatus.Error -> Triple("⚠️", "Error: ${status.message}", MaterialTheme.colorScheme.error)
        is InvoiceStatus.Cancelled -> Triple("🚫", "Monitoring cancelled", MaterialTheme.colorScheme.outline)
    }

    Card(
        colors = CardDefaults.cardColors(
            containerColor = color.copy(alpha = 0.1f)
        ),
        modifier = Modifier.fillMaxWidth()
    ) {
        Column(
            modifier = Modifier.padding(12.dp)
        ) {
            Row(
                verticalAlignment = Alignment.CenterVertically
            ) {
                Text(
                    text = icon,
                    style = MaterialTheme.typography.headlineMedium,
                    modifier = Modifier.padding(end = 12.dp)
                )
                Column {
                    Text(
                        text = text,
                        style = MaterialTheme.typography.titleMedium,
                        color = color
                    )
                    if (status is InvoiceStatus.Pending) {
                        Text(
                            text = "Hash: ${status.paymentHash.take(16)}...",
                            style = MaterialTheme.typography.bodySmall
                        )
                    }
                    if (status is InvoiceStatus.Success && status.transaction != null) {
                        Text(
                            text = "Amount: ${status.transaction.amountMsats / 1000} sats",
                            style = MaterialTheme.typography.bodySmall
                        )
                    }
                }
            }
            
            // Show invoice for pending state
            if (status is InvoiceStatus.Pending && invoice != null) {
                Spacer(modifier = Modifier.height(8.dp))
                Text(
                    text = "Invoice (tap to copy):",
                    style = MaterialTheme.typography.labelSmall
                )
                Text(
                    text = invoice.take(60) + "...",
                    style = MaterialTheme.typography.bodySmall,
                    modifier = Modifier.padding(top = 4.dp)
                )
            }
        }
    }
}

suspend fun createAndMonitorInvoice(
    apiKey: String,
    onInvoiceCreated: (invoice: String, paymentHash: String) -> Unit,
    onStatusUpdate: (InvoiceStatus) -> Unit,
    onError: (String) -> Unit
) = withContext(Dispatchers.IO) {
    try {
        val config = StrikeConfig(
            apiKey = apiKey,
            baseUrl = null,
            httpTimeout = null,
            socks5Proxy = null,
            acceptInvalidCerts = null
        )
        
        // Use factory function for polymorphic LightningNode access
        val node: LightningNode = createStrikeNode(config)

        // Create an invoice for 21 sats
        val invoiceParams = CreateInvoiceParams(
            invoiceType = InvoiceType.BOLT11,
            amountMsats = 21000L, // 21 sats
            offer = null,
            description = "LNI Android Demo - Pay me!",
            descriptionHash = null,
            expiry = 300L, // 5 minutes
            rPreimage = null,
            isBlinded = false,
            isKeysend = false,
            isAmp = false,
            isPrivate = false
        )

        val transaction = node.createInvoice(invoiceParams)
        
        // Notify UI that invoice was created
        withContext(Dispatchers.Main) {
            onInvoiceCreated(transaction.invoice, transaction.paymentHash)
        }

        // Create callback for real-time status updates
        val callback = object : OnInvoiceEventCallback {
            override fun success(transaction: Transaction?) {
                kotlinx.coroutines.runBlocking {
                    withContext(Dispatchers.Main) {
                        onStatusUpdate(InvoiceStatus.Success(transaction))
                    }
                }
            }

            override fun pending(transaction: Transaction?) {
                kotlinx.coroutines.runBlocking {
                    withContext(Dispatchers.Main) {
                        onStatusUpdate(InvoiceStatus.Pending(transaction?.paymentHash ?: ""))
                    }
                }
            }

            override fun failure(transaction: Transaction?) {
                kotlinx.coroutines.runBlocking {
                    withContext(Dispatchers.Main) {
                        onStatusUpdate(InvoiceStatus.Failed(transaction))
                    }
                }
            }
        }

        // Monitor the invoice for payment using LightningNode interface
        val params = OnInvoiceEventParams(
            paymentHash = transaction.paymentHash,
            search = null,
            pollingDelaySec = 2L,
            maxPollingSec = 300L // 5 minutes timeout
        )

        node.onInvoiceEvents(params, callback)

    } catch (e: ApiException) {
        withContext(Dispatchers.Main) {
            onError("API Error: ${e.message}")
        }
    } catch (e: Exception) {
        withContext(Dispatchers.Main) {
            onError("Error: ${e.message}")
        }
    }
}