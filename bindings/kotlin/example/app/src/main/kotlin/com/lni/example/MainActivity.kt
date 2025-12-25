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
    sb.appendLine("=== Strike Balance ===\n")

    try {
        val config = StrikeConfig(
            apiKey = apiKey,
            baseUrl = null,
            httpTimeout = null,
            socks5Proxy = null,
            acceptInvalidCerts = null
        )
        val node = StrikeNode(config)

        sb.appendLine("Fetching balance...")

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
    sb.appendLine("=== Strike Node Test ===\n")

    try {
        val config = StrikeConfig(
            apiKey = "test_api_key",
            baseUrl = null,
            httpTimeout = null,
            socks5Proxy = null,
            acceptInvalidCerts = null
        )
        val node = StrikeNode(config)

        sb.appendLine("Strike node created successfully!")
        sb.appendLine("Getting node info...")

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
    sb.appendLine("=== Blink Node Test ===\n")

    try {
        val config = BlinkConfig(
            apiKey = "test_api_key",
            baseUrl = null,
            httpTimeout = null
        )
        val node = BlinkNode(config)

        sb.appendLine("Blink node created successfully!")
        sb.appendLine("Getting node info...")

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
    sb.appendLine("=== NWC Node Test ===\n")

    try {
        val config = NwcConfig(
            nwcUri = "nostr+walletconnect://test"
        )
        val node = NwcNode(config)

        sb.appendLine("NWC node created successfully!")
        sb.appendLine("Getting node info...")

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
