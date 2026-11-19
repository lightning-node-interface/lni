# Lexe Node.js binding

`LexeNode` uses the same Rust Lexe implementation as the React Native binding.
It is available only in a Node.js process with a matching native `.node` addon.

```js
const { LexeNode, LexeConnect } = require('@sunnyln/lni-nodejs')

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
monitoring; omit it for short-lived address and info requests.

`close()` releases the native wallet and rejects later calls; calls already in
progress may still settle. It also stops every invoice monitor on that node:
polling is cancelled, events already queued for JavaScript are discarded, and no
`onInvoiceEvents` callback runs after `close()` returns. The promise of each
stopped monitor resolves without a further callback. Starting a monitor on a
closed node rejects. Calling `close()` again does nothing.

`onInvoiceEvents(params, callback)` follows the LNI-wide status rules in
[docs/invoice-events.md](../../docs/invoice-events.md): a failed Lexe sync or
lookup reports `pending` and polling continues, so one network error never ends
a monitor. `failure` means `maxPollingSec` elapsed, the parameters cannot be
monitored, or Lexe reports the payment failed. If `callback` throws, the promise
rejects with that error and the native monitor stops.

## Errors

Every error from `LexeNode`, `LexeConnect` and `LexeConnectSession` has a stable
`error.code`. Branch on the code, not on `error.message`, which may change. The
exceptions are an aborted signal, which rejects with `name === 'AbortError'`, and
an error thrown by your own `onInvoiceEvents` callback, which is passed through.

| Code | Meaning | What to do |
| --- | --- | --- |
| `LEXE_INVALID_INPUT` | The request was rejected before anything was sent: bad credentials, network, address, amount or option. The message says which. | Fix the input; retrying the same request fails again. |
| `LEXE_PAYMENT_INDETERMINATE` | A payment may have been sent: a Lightning payment timed out, or an on-chain send failed after submission or returned an unreadable response. | Do not prepare a new payment. Call `lookupInvoice` with the payment hash, or check `listTransactions`; an on-chain retry must reuse the same prepared transaction (same payment id). |
| `LEXE_REQUEST_FAILED` | Lexe or the network failed. No payment outcome is in doubt. | Retry later. The message omits upstream detail, which may be sensitive. |
| `LEXE_NODE_CLOSED` | `close()` was already called. | Create a new `LexeNode`. |
| `LEXE_CONNECT_UNAVAILABLE` | A LexeConnect mailbox request failed; the attempt is still live. | Poll again. `waitForResponse()` retries with backoff. |
| `LEXE_CONNECT_EXPIRED` | The LexeConnect attempt passed its deadline. | Start a new attempt. |
| `LEXE_CONNECT_CANCELLED` | The LexeConnect attempt was cancelled. | Start a new attempt. |
| `LEXE_CONNECT_COMPLETED` | The LexeConnect attempt already delivered its response. | Use the response already returned. |
| `LEXE_CONNECT_DISPOSED` | `dispose()` was already called on this `LexeConnect`. | Start a new attempt. |

One LexeConnect attempt owns a private response key in memory. Keep the same
`LexeConnect` instance through approval. A Vercel function cannot rely on a
later request reaching the same process, so a merchant Connect flow needs a
durable handoff design or an always-running component.

`pollMailbox(signal)` shares one native mailbox request between concurrent
callers. Aborting `signal` rejects only that call with `AbortError`; the request
continues, and a verified response it receives is kept and returned by the next
`pollMailbox()` or `waitForResponse()`, because the native session is single-use.
Aborting the `waitForResponse` signal, or calling `cancel()`, ends the attempt; a
response verified before cancellation took effect is still returned by the next
`pollMailbox()`. `dispose()` cancels and discards everything, including a kept
response. Signals may be any `AbortSignal`-like object, so Node versions without
a global `AbortController` are supported. The
[`examples/lexe-vercel-napi`](../../examples/lexe-vercel-napi/README.md) app
verifies native loading and read-only calls on Vercel.

## Generated bindings and stateless Node helpers

The package build writes the napi-rs loader to `native.js`, declarations to
`generated/`, and the platform addon beside the loader. Keep these declarations generated from Rust. The root entry
point preserves the JavaScript Connect and invoice-event helpers when rebuilding.

`@sunnyln/lni-nodejs/lexe` is a lazy entry point for stateless servers. Importing it does not
load the addon. `restoreLexeSession(options, seed, expiresAtMs)` restores an
expiring request and provides `connectionString`, `acceptRedirect`,
`pollMailbox`, and `close`. Every instance restored from the same seed can accept
the same wallet response again, so the caller must enforce single use: before
acting on a response, atomically claim the attempt in durable storage (for
example, a compare-and-set on its attempt ID) and reject any second claim. Keep
the seed encrypted, bound to the initiating user and organization, and delete it
after the claim or expiry.
`withLexeNode(credentials, callback, network)` closes the native wallet after the
callback resolves or rejects. Wallet permissions and receiving-network policy
remain the caller's responsibility.

`helpers/lexe.js` and `helpers/lexe.d.ts` are committed build output, like
`native.js` and `generated/`, so a fresh checkout packs a complete package.
Run `build` to compile both the native bindings and TypeScript helpers, then
commit the regenerated files. `check:package` fails if the helper output is stale
(when TypeScript is installed) or if the npm manifest lacks an entry point or
includes tests; the publish workflow runs it. `typecheck:helpers` checks the
helper and generated declaration contracts.

`test:lexe` runs the offline JavaScript tests. `test:lexe-monitor` builds a debug
addon with the `lexe_monitor_test` feature, which adds synthetic monitor hooks
(never enable it for releases), and runs the native monitor lifecycle tests;
extra arguments go to cargo, for example `npm run test:lexe-monitor -- --offline`.
It replaces the debug library in the Cargo target directory, not the packaged
`.node` file.

## Exclusive receiving addresses

`getReceivingCapabilities()` uses authenticated node info and returns
`{ nodeVersion, exclusiveAddresses }`. It requires `read_info` and does not
allocate an address. Check this before saving a connection that requires
exclusive addresses. Unsupported nodes return `exclusiveAddresses: false`;
connection or authorization failures reject the call.

`getExclusiveAddress()` issues a new Bitcoin address on every call. It requires
Lexe node v0.10.9. It uses the public wallet method from the published Rust
SDK v0.1.29, including that release's trusted node measurements.

Unlike `getNextUnusedAddress()`, exclusive addresses are not watched by the
node's regular sync. After detecting a payment externally, call
`checkAddress(address, syncedBlockHeight?)`. It records receives in Lexe and
returns `{ receivedTxids, syncedBlockHeight }`. The chain source can lag an
external mempool watcher, so retry if the expected transaction is absent.
Both methods validate the address network. No shared-address fallback is used.

`receivedTxids` lists the address's receives that are unconfirmed, or confirmed
at or after the `syncedBlockHeight` you passed in. Omit the height on the first
check, and pass the returned `syncedBlockHeight` to the next check to skip
receives already confirmed before it. Save the txids before you save the new height: if
the process stops in between, the next check with the old height returns them
again, while a check with the new height would skip any that confirmed earlier.
`receivedTxids` records receives in Lexe; it is not proof of payment. Verify the
amount and confirmations from your own chain source before crediting an order.

The address check uses the typed gateway client from released `lexe-api` v0.1.29
through the SDK's `unstable` client access. The public wallet check also fetches
payment history and requires `read_payments`; the gateway-only check records
receives without that broader grant. `read_info` and `receive` remain sufficient.

Contracts: [released wallet methods](https://github.com/lexe-app/lexe-public/blob/lexe-sdk-rust-v0.1.29/lexe/src/wallet.rs) and [gateway API definitions](https://github.com/lexe-app/lexe-public/blob/lexe-sdk-rust-v0.1.29/lexe-api-core/src/def.rs).
