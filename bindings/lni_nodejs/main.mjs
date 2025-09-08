import { PhoenixdNode, ClnNode, LndNode, StrikeNode, InvoiceType, BlinkNode, SpeedNode, NwcNode } from "./index.js";
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
      await new Promise(resolve => setTimeout(resolve, 2000));

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

async function blink() {
  const config = {
    apiKey: process.env.BLINK_API_KEY,
  };
  
  if (!config.apiKey) {
    console.log("Skipping Blink test - BLINK_API_KEY not set");
    return;
  }

  const node = new BlinkNode(config);
  await testAsyncNode("Blink", node, process.env.BLINK_TEST_PAYMENT_HASH);
}

async function speed() {
  const config = {
    apiKey: process.env.SPEED_API_KEY,
  };

  if (!config.apiKey) {
    console.log("Skipping Speed test - SPEED_API_KEY not set");
    return;
  }

  const node = new SpeedNode(config);
  await testAsyncNode("Speed", node, process.env.SPEED_TEST_PAYMENT_HASH);
}

async function nwc() {
  const config = {
    nwcUri: process.env.NWC_URI,
  };

  if (!config.nwcUri) {
    console.log("Skipping Nwc test - NWC_URI not set");
    return;
  }

  const node = new NwcNode(config);
  await testAsyncNode("Nwc", node, process.env.NWC_TEST_PAYMENT_HASH);
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
  
  // await lnd();
  // await strike();
  // await cln();
  // await phoenixd();
  await blink();
  // await speed();
  // await nwc();
  
  console.log("\n=== All tests completed ===");
}

main();
