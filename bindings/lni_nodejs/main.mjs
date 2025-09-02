import { PhoenixdNode, ClnNode, LndNode, NwcNode, InvoiceType, sayAfterWithTokio } from "./index.js";
import dotenv from "dotenv";
dotenv.config();

async function phoenixd() {
  const config = {
    url: process.env.PHOENIXD_URL,
    password: process.env.PHOENIXD_PASSWORD,
    test_hash: process.env.PHOENIXD_TEST_PAYMENT_HASH,
  };
  const node = new PhoenixdNode(config);
  const info = await node.getInfo();
  console.log("Node info:", info);

  const configRes = await node.getConfig();
  console.log("Config:", configRes.url);

  const invoice = await node.createInvoice({
    amountMsats: 1000,
    description: "test invoice",
    invoiceType: InvoiceType.Bolt11,
  });
  console.log("Invoice:", invoice);

  const lookupInvoice = await node.lookupInvoice(
    process.env.PHOENIXD_TEST_PAYMENT_HASH
  );
  console.log("lookupInvoice:", lookupInvoice);

  const payOffer = await node.payOffer(
    process.env.TEST_RECEIVER_OFFER,
    3000,
    "payment from lni nodejs"
  );
  console.log("payOffer:", payOffer);

  const txns = await node.listTransactions({
    from: 0,
    limit: 10,
  });
  console.log("Transactions:", txns);

  const offer = await node.getOffer();
  console.log("Get Offer:", offer);

  // const pay_invoice_resp = await node.payInvoice({
  //   invoice: ""
  // })
  // console.log("pay_invoice_resp:", pay_invoice_resp);
}

async function cln() {
  const config = {
    url: process.env.CLN_URL,
    rune: process.env.CLN_RUNE,
  };
  const node = new ClnNode(config);
  const info = await node.getInfo();
  console.log("Node info:", info);

  const invoice = await node.createInvoice({
    amountMsats: 1000,
    description: "test invoice",
    invoiceType: InvoiceType.Bolt11,
  });
  console.log("Invoice:", invoice);

  const bolt11Invoice = await node.createInvoice({
    amountMsats: 3000,
    description: "test invoice",
    invoiceType: InvoiceType.Bolt11,
  });
  console.log("CLN bolt11 Invoice:", bolt11Invoice);

  const offer = await node.getOffer();
  console.log("CLN Bolt12 Offer:", offer);

  const lookupInvoice = await node.lookupInvoice(
    process.env.CLN_TEST_PAYMENT_HASH
  );
  console.log("lookupInvoice:", lookupInvoice);

  // TODO not working (cln <=> phoneixd issue?)
  // const payOffer = await node.payOffer(
  //   process.env.TEST_RECEIVER_OFFER,
  //   3000,
  //   "payment from lni nodejs"
  // );
  // console.log("payOffer:", payOffer);

  const txns = await node.listTransactions({
    from: 0,
    limit: 10,
  });
  console.log("Transactions:", txns);
}

async function lnd() {
  const config = {
    url: process.env.LND_URL,
    macaroon: process.env.LND_MACAROON,
    socks5Proxy: process.env.LND_SOCKS5_PROXY || "",
    acceptInvalidCerts: true
  };
  const node = new LndNode(config);
  
  // Test both sync and async versions
  const info = await node.getInfo();
  console.log("Node info (sync):", info);

  const infoAsync = await node.getInfoAsync();
  console.log("Node info (async):", infoAsync);

  const invoice = await node.createInvoice({
    amountMsats: 1000,
    description: "test invoice",
    invoiceType: InvoiceType.Bolt11,
  });
  console.log("LND Invoice:", invoice);

  const bolt11Invoice = await node.createInvoice({
    amountMsats: 3000,
    description: "test invoice",
    invoiceType: InvoiceType.Bolt11,
  });
  console.log("LND bolt11 Invoice:", bolt11Invoice);


  const lookupInvoice = await node.lookupInvoice(
    process.env.LND_TEST_PAYMENT_HASH
  );
  console.log("lookupInvoice:", lookupInvoice);

  const txns = await node.listTransactions({
    from: 0,
    limit: 10,
  });
  console.log("LND Transactions:", txns);
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

async function testSayAfterWithTokio() {
  console.log("\n=== Testing sayAfterWithTokio function ===");
  
  try {
    // Test 1: Basic HTTP request without proxy
    console.log("Test 1: Basic HTTP request to ipify.org");
    const result1 = await sayAfterWithTokio(
      1000,                                    // 1 second delay
      "Nick",                                  // Name
      "https://api.ipify.org?format=json",     // URL
      null,                                    // No proxy
      null,                                    // No header key
      null                                     // No header value
    );
    console.log("Result 1:", result1);

    // Test 2: Request with SOCKS5 proxy (will likely fail unless you have a proxy running)
    console.log("\nTest 3: Request with SOCKS5 proxy (may fail if no proxy available)");
    try {
      const result3 = await sayAfterWithTokio(
        2000,                                  // 2 second delay
        "ProxyUser",                           // Name
        "https://api.ipify.org?format=json",   // URL
        "socks5h://127.0.0.1:9150",            // SOCKS5 proxy (common default)
        "X-Test-Header",                       // Header key
        "proxy-test"                           // Header value
      );
      console.log("Result 3:", result3);
    } catch (proxyError) {
      console.log("Result 3: Proxy test failed (expected if no proxy running):", proxyError.message);
    }

    console.log("\n=== sayAfterWithTokio tests completed ===\n");

  } catch (error) {
    console.error("Test error:", error.message);
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

async function main() {
  // Test the HTTP function
  await testSayAfterWithTokio();
  
  // Uncomment these to test other functionality
  // phoenixd();
  // cln();
  // await lnd();
  // await nwc();
  // test();
}

main();
