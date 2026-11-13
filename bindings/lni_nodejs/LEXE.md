# Lexe Node.js binding

`LexeNode` uses the same Rust Lexe implementation as the React Native binding.
It is available only in a Node.js process with a matching native `.node` addon.

```js
const { LexeNode, LexeConnect } = require('lni_js')

const node = new LexeNode(process.env.LEXE_CLIENT_CREDENTIALS, 'mainnet')
try {
  const info = await node.getInfo()
  const address = await node.getNextUnusedAddress()
  // Use info and address in your server response.
} finally {
  node.close()
}

const connect = new LexeConnect({
  scopes: ['read_info', 'receive'],
  requesterName: 'Your app',
})
try {
  const link = connect.connectionString()
  // Send the link to the browser or render it as a QR code. Keep it private.
  const result = await connect.waitForResponse()
  if (result.status === 'approved') {
    // Store result.clientCredentials in a server-side secret store.
  }
} finally {
  connect.dispose()
}
```

The full wallet surface includes invoices, offers, on-chain payments, transaction
lookup, and invoice event monitoring. `getOffer` and `listOffers` are present for
API parity, but the Lexe SDK does not currently support them. Use a persistent
`dataDir` as the constructor's third argument for payment history and invoice
monitoring; omit it for short-lived address and info requests. `close()` releases
the native wallet and rejects later calls.

One LexeConnect attempt owns a private response key in memory. Keep the same
`LexeConnect` instance through approval. A Vercel function cannot rely on a
later request reaching the same process, so a merchant Connect flow needs a
durable handoff design or an always-running component. The
[`examples/lexe-vercel-napi`](../../examples/lexe-vercel-napi/README.md) app
verifies native loading and read-only calls on Vercel.
