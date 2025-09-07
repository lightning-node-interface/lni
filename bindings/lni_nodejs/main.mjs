import { PhoenixdNode, ClnNode, LndNode, StrikeNode, NwcNode, InvoiceType, sayAfterWithTokio } from "./index.js";
import dotenv from "dotenv";
dotenv.config();

// Shared test logic for async node implementations (LND, Strike)
async function testAsyncNode(nodeName, node, testInvoiceHash) {
  console.log(`\n=== Testing ${nodeName} ===`);
  
  try {
    // Test 1: Get node info
    console.log(`(1) ${nodeName} - Testing getInfo...`);
    const info = await node.getInfo();
    console.log(`${nodeName} Node info:`, info);

    // Test 2: Create invoice
    console.log(`(2) ${nodeName} - Testing createInvoice...`);
    const invoice = await node.createInvoice({
      amountMsats: 1000,
      description: `test invoice from ${nodeName}`,
      invoiceType: InvoiceType.Bolt11,
    });
    console.log(`${nodeName} Invoice:`, invoice);

    // Test 3: Lookup invoice (if test hash provided)
    if (testInvoiceHash) {
      console.log(`(3) ${nodeName} - Testing lookupInvoice...`);
      try {
        const lookupInvoice = await node.lookupInvoice({
          paymentHash: testInvoiceHash,
          search: "",
        });
        console.log(`${nodeName} lookupInvoice:`, lookupInvoice);
      } catch (error) {
        console.log(`${nodeName} lookupInvoice failed (expected if hash doesn't exist):`, error.message);
      }
    }

    // Test 4: List transactions
    console.log(`(4) ${nodeName} - Testing listTransactions...`);
    const txns = await node.listTransactions({
      from: 0,
      limit: 5,
    });
    console.log(`${nodeName} Transactions (${txns.length} found):`, txns);

    // Test 5: Decode invoice (if we have one)
    if (invoice && invoice.invoice) {
      console.log(`(5) ${nodeName} - Testing decode...`);
      try {
        const decoded = await node.decode(invoice.invoice);
        console.log(`${nodeName} Decoded:`, decoded);
      } catch (error) {
        console.log(`${nodeName} decode failed:`, error.message);
      }
    }

    // Test 6: Invoice Events (callback-style like lib)
    console.log(`(6) ${nodeName} - Testing onInvoiceEvents...`);
    try {
      const params = {
        paymentHash: testInvoiceHash || "test",
        search: "",
        pollingDelaySec: 2,
        maxPollingSec: 4
      };

      console.log(`${nodeName} - Starting onInvoiceEvents with callback...`);
      await node.onInvoiceEvents(params, (status, transaction) => {
        console.log(`${nodeName} - Invoice event: ${status}`, transaction);
      });
      console.log(`${nodeName} - onInvoiceEvents started successfully`);

    } catch (error) {
      console.log(`${nodeName} - onInvoiceEvents test failed:`, error.message);
    }

    // Test 7: BOLT12 functions (these should return "not implemented" errors)
    console.log(`(7) ${nodeName} - Testing BOLT12 functions (should fail with 'not implemented')...`);
    try {
      const offer = await node.getOffer("");
      console.log(`${nodeName} getOffer:`, offer);
    } catch (error) {
      console.log(`${nodeName} getOffer failed (expected):`, error.message);
    }

    try {
      const offers = await node.listOffers("");
      console.log(`(8) ${nodeName} listOffers:`, offers);
    } catch (error) {
      console.log(`${nodeName} listOffers failed (expected):`, error.message);
    }

    console.log(`${nodeName} - All tests completed successfully!`);

  } catch (error) {
    console.error(`${nodeName} - Test error:`, error.message);
  }
}

async function phoenixd() {
  const config = {
    url: process.env.PHOENIXD_URL,
    password: process.env.PHOENIXD_PASSWORD,
  };

  if (!config.url || !config.password) {
    console.log("Skipping PhoenixD test - PHOENIXD_URL or PHOENIXD_PASSWORD not set");
    return;
  }

  const node = new PhoenixdNode(config);
  await testAsyncNode("PhoenixD", node, process.env.PHOENIXD_TEST_PAYMENT_HASH);

}

async function cln() {
  const config = {
    url: process.env.CLN_URL,
    rune: process.env.CLN_RUNE,
  };

  if (!config.url || !config.rune) {
    console.log("Skipping CLN test - CLN_URL or CLN_RUNE not set");
    return;
  }

  const node = new ClnNode(config);
  await testAsyncNode("CLN", node, process.env.CLN_TEST_PAYMENT_HASH);
  
}

async function lnd() {
  const config = {
    url: process.env.LND_URL,
    macaroon: process.env.LND_MACAROON,
    // socks5Proxy: process.env.LND_SOCKS5_PROXY || "",
    acceptInvalidCerts: true,
    httpTimeout: 40
  };
  
  if (!config.url || !config.macaroon) {
    console.log("Skipping LND test - LND_URL or LND_MACAROON not set");
    return;
  }

  const node = new LndNode(config);
  await testAsyncNode("LND", node, process.env.LND_TEST_PAYMENT_HASH);
}

async function strike() {
  const config = {
    apiKey: process.env.STRIKE_API_KEY,
  };
  
  if (!config.apiKey) {
    console.log("Skipping Strike test - STRIKE_API_KEY not set");
    return;
  }

  const node = new StrikeNode(config);
  await testAsyncNode("Strike", node, process.env.STRIKE_TEST_PAYMENT_HASH);
}

async function nwc() {
  const config = {
    nwcUri: process.env.NWC_URI,
    socks5Proxy: process.env.NWC_SOCKS5_PROXY || "", // Use empty string instead of null
    httpTimeout: 60
  };

  if (!config.nwcUri) {
    console.log("Skipping NWC test - NWC_URI not set");
    return;
  }

  const node = new NwcNode(config);
  
  try {
    const info = await node.getInfo();
    console.log("NWC Node info:", info);

    const invoice = await node.createInvoice({
      amountMsats: 1000,
      description: "test invoice from NWC",
      invoiceType: InvoiceType.Bolt11,
    });
    console.log("NWC Invoice:", invoice);

    // Test cancellation functionality
    console.log("Testing NWC invoice events with cancellation...");
    
    const params = {
      paymentHash: process.env.NWC_TEST_PAYMENT_HASH,
      search: "", // Use empty string instead of null
      pollingDelaySec: 3,
      maxPollingSec: 60
    };

    // Start polling with cancellation support
    const handle = node.onInvoiceEventsCancel(params);
    console.log("Started cancellable invoice polling");

    // Poll for events for 15 seconds
    let eventCount = 0;
    const startTime = Date.now();
    const maxTestTime = 15000; // 15 seconds

    const pollInterval = setInterval(() => {
      const event = handle.pollEvent();
      if (event) {
        eventCount++;
        console.log(`NWC Event ${eventCount}: ${event.status}`, event.transaction ? `Payment Hash: ${event.transaction.paymentHash}` : 'No transaction data');
        
        if (event.status === 'success' || event.status === 'failure') {
          console.log("Final event received, stopping polling");
          clearInterval(pollInterval);
          return;
        }
      }
      
      // Check if cancelled
      if (handle.isCancelled()) {
        console.log("NWC Polling was cancelled");
        clearInterval(pollInterval);
        return;
      }

      // Stop after max test time
      if (Date.now() - startTime > maxTestTime) {
        console.log("Test timeout reached, cancelling...");
        handle.cancel();
        clearInterval(pollInterval);
      }
    }, 1000);

    // Test the wait_for_event method after 5 seconds
    setTimeout(() => {
      console.log("Testing wait_for_event with 5 second timeout...");
      const event = handle.waitForEvent(5000);
      if (event) {
        console.log("Event received via waitForEvent:", event);
      } else {
        console.log("No event received within waitForEvent timeout");
      }
    }, 2000);

    // Cancel after 10 seconds to test cancellation
    setTimeout(() => {
      if (!handle.isCancelled()) {
        console.log("Cancelling NWC invoice event polling...");
        handle.cancel();
      }
    }, 10000);

    const txns = await node.listTransactions({
      from: 0,
      limit: 5,
    });
    console.log("NWC Transactions:", txns);

  } catch (error) {
    console.error("NWC test error:", error.message);
  }
}

async function test() {
  const config = {
    url: process.env.PHOENIXD_URL,
    password: process.env.PHOENIXD_PASSWORD,
    test_hash: process.env.PHOENIXD_TEST_PAYMENT_HASH,
  };
  const node = new PhoenixdNode(config);
  // const config = {
  //   url: process.env.LND_URL,
  //   macaroon: process.env.LND_MACAROON,
  //   // socks5Proxy: "socks5h://127.0.0.1:9150",
  //   acceptInvalidCerts: true,
  // };
  // const node = new LndNode(config);


  console.log("Node info:", await node.getInfo());

  // await node.onInvoiceEvents(
  //   {
  //     paymentHash: config.test_hash,
  //     pollingDelaySec: 4,
  //     maxPollingSec: 60,
  //   }, 
  //   (status, tx) => {
  //     console.log("Invoice event:", status, tx);
  //   }
  // );
}

// Helper function to show required environment variables
function showEnvironmentHelp() {
  console.log("\n=== Environment Variables Required ===");
  console.log("For LND testing:");
  console.log("  LND_URL=https://your-lnd-node:8080");
  console.log("  LND_MACAROON=your_base64_macaroon");
  console.log("  LND_SOCKS5_PROXY=socks5h://127.0.0.1:9150 (optional)");
  console.log("  LND_TEST_PAYMENT_HASH=existing_payment_hash (optional)");
  console.log("");
  console.log("For Strike testing:");
  console.log("  STRIKE_API_KEY=your_strike_api_key");
  console.log("  STRIKE_BASE_URL=https://api.strike.me (optional, defaults to this)");
  console.log("  STRIKE_SOCKS5_PROXY=socks5h://127.0.0.1:9150 (optional)");
  console.log("  STRIKE_TEST_PAYMENT_HASH=existing_payment_hash (optional)");
  console.log("=======================================\n");
}

async function main() {
  console.log("=== Lightning Node Interface (LNI) Tests ===");
  
  // Show environment help
  showEnvironmentHelp();
  
  await lnd();
  // await strike();
  // await cln();
  // await phoenixd();
  // await nwc();
  // await test();
  
  console.log("\n=== All tests completed ===");
}

main();
