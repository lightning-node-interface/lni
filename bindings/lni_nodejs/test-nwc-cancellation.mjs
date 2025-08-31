#!/usr/bin/env node

// Test script for NWC cancellation functionality
import { NwcNode, InvoiceType } from "./index.js";
import dotenv from "dotenv";
dotenv.config();

async function testNwcCancellation() {
  console.log("Testing NWC Cancellation Functionality");
  console.log("=====================================");

  // Check for required environment variables
  if (!process.env.NWC_URI) {
    console.error("Error: NWC_URI environment variable is required");
    console.log("Please set your NWC_URI in .env file or environment");
    console.log("Example: NWC_URI=nostr+walletconnect://pubkey?relay=...&secret=...");
    process.exit(1);
  }

  const config = {
    nwcUri: process.env.NWC_URI,
    socks5Proxy: process.env.NWC_SOCKS5_PROXY || "", // Use empty string instead of null
    httpTimeout: 60
  };

  try {
    const node = new NwcNode(config);
    
    // Test basic functionality first
    console.log("1. Testing NWC node connection...");
    const info = await node.getInfo();
    console.log("✓ Node info:", {
      alias: info.alias,
      pubkey: info.pubkey.substring(0, 16) + "...",
      network: info.network
    });
   
    // Test cancellation functionality
    console.log("\n3. Testing cancellable invoice polling...");
    
    const params = {
      paymentHash: process.env.NWC_TEST_PAYMENT_HASH, 
      search: "", // Use empty string instead of null
      pollingDelaySec: 2,
      maxPollingSec: 30
    };

    const handle = node.onInvoiceEventsCancel(params);
    console.log("✓ Started cancellable polling for payment hash:", params.paymentHash);

    let eventCount = 0;
    let testCompleted = false;

    // Poll for events
    const pollInterval = setInterval(() => {
      if (testCompleted) return;

      const event = handle.pollEvent();
      if (event) {
        eventCount++;
        console.log(`📥 Event ${eventCount}:`, {
          status: event.status,
          hasTransaction: !!event.transaction,
          txn: event.transaction,
        });
        
        if (event.status === 'success' || event.status === 'failure') {
          console.log("✓ Final event received");
          testCompleted = true;
          clearInterval(pollInterval);
        }
      }
      
      // Check cancellation status
      if (handle.isCancelled()) {
        console.log("✓ Polling was cancelled as expected");
        testCompleted = true;
        clearInterval(pollInterval);
      }
    }, 1000);

    // Test immediate cancellation after 5 seconds
    setTimeout(() => {
      if (!testCompleted) {
        console.log("\n4. Testing cancellation...");
        handle.cancel();
        console.log("✓ Cancellation requested");
        
        // Verify cancellation status
        setTimeout(() => {
          if (handle.isCancelled()) {
            console.log("✓ Cancellation confirmed");
          } else {
            console.log("⚠ Cancellation not confirmed");
          }
          testCompleted = true;
          clearInterval(pollInterval);
        }, 1000);
      }
    }, 5000);

    // Test wait_for_event method
    setTimeout(() => {
      if (!testCompleted) {
        console.log("\n5. Testing waitForEvent method...");
        const event = handle.waitForEvent(3000); // 3 second timeout
        if (event) {
          console.log("✓ Event received via waitForEvent:", event.status);
        } else {
          console.log("⚠ No event received within waitForEvent timeout (expected for unpaid invoice)");
        }
      }
    }, 2000);

    // Cleanup and final status after 8 seconds
    setTimeout(() => {
      if (!testCompleted) {
        console.log("\n6. Test cleanup...");
        handle.cancel();
        testCompleted = true;
        clearInterval(pollInterval);
      }
      
      console.log("\n=== Test Summary ===");
      console.log("Events received:", eventCount);
      console.log("Cancellation status:", handle.isCancelled());
      console.log("✓ NWC cancellation test completed");
      
    }, 8000);

  } catch (error) {
    console.error("❌ Test failed:", error.message);
    process.exit(1);
  }
}

// Run the test
testNwcCancellation().catch(console.error);
