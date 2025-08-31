const { NwcNode } = require('../index.js');

// Example usage of the cancellable invoice events
async function main() {
  const config = {
    nwc_uri: process.env.NWC_URI || "nostr+walletconnect://...", // Your NWC URI here
    socks5_proxy: null,
    http_timeout: 60
  };

  const node = new NwcNode(config);

  const params = {
    payment_hash: process.env.TEST_PAYMENT_HASH, // Your test payment hash
    search: null,
    polling_delay_sec: 5,
    max_polling_sec: 60
  };

  console.log("Starting invoice event polling with cancellation support...");
  
  // Start polling with cancellation support
  const handle = node.on_invoice_events_cancel(params);
  
  // Set up a timer to cancel after 30 seconds
  const cancelTimer = setTimeout(() => {
    console.log("Cancelling invoice event polling...");
    handle.cancel();
  }, 30000);

  // Poll for events
  const pollInterval = setInterval(() => {
    const event = handle.poll_event();
    if (event) {
      console.log(`Received event: ${event.status}`, event.transaction ? event.transaction : 'No transaction data');
      
      if (event.status === 'success' || event.status === 'failure') {
        console.log("Final event received, stopping polling");
        clearInterval(pollInterval);
        clearTimeout(cancelTimer);
      }
    }
    
    // Check if cancelled
    if (handle.is_cancelled()) {
      console.log("Polling was cancelled");
      clearInterval(pollInterval);
      clearTimeout(cancelTimer);
    }
  }, 1000);

  // Alternative: wait for specific events with timeout
  setTimeout(async () => {
    console.log("Waiting for event with 10 second timeout...");
    const event = handle.wait_for_event(10000); // 10 second timeout
    if (event) {
      console.log("Event received via wait_for_event:", event);
    } else {
      console.log("No event received within timeout");
    }
  }, 5000);
}

if (require.main === module) {
  main().catch(console.error);
}

module.exports = { main };
