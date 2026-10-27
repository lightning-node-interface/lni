# Lexe on-chain preflight fee estimates

LNI uses the stable `LexeWallet::pay_onchain_preflight` API introduced in
[Lexe Rust SDK 0.1.27](https://github.com/lexe-app/lexe-public/releases/tag/lexe-sdk-rust-v0.1.27).
No `lexe/unstable` feature or direct `lexe-api` dependency is required.
The underlying node still requires spend-authorized credentials.

`LexeNode::prepare_onchain_transaction` validates locally and calls preflight.
It returns genuine wallet-specific miner fees computed from wallet UTXOs and
coin selection. No assumed transaction size is used. Preparing or refreshing
does not submit a payment, sign a transaction, or reserve funds.

| LNI speed | UI label | Lexe response field |
| --- | --- | --- |
| `slow` | Slow | `background_fees` |
| `normal` (default) | Medium | `normal_fees` |
| `fast` | Fast | `high_fees` |

The selected fee populates `fee_sats`; `recipient_amount_sats` is the requested
amount, and `total_amount_sats` is amount plus miner fee. Conversion from Lexe's
`Amount` uses integer millisatoshis, rejects fractional sats, and checks total
overflow. React Native rejects values outside JavaScript's safe integer range.
The existing UniFFI/React Native fields carry the estimates without an ABI change;
native libraries must be rebuilt.

## Estimates and payment safety

`fee_limit_supported` / `feeLimitSupported` remains `false`: sending cannot
enforce a maximum fee. Explicit fee guardrails are rejected, including when
`dangerouslyDisableFeeGuardrail` is set. The actual fee after submission may
differ from the estimate.

Generate one payment ID per intended payment and persist it. Supply that same
ID on every preparation for speed selection or refresh, keeping the same
address, amount, and personal note. Preparing without an ID generates a fresh
one; never do that to retry an uncertain send.

Lexe rejects preflight for an ID that has already been submitted, including a
failed attempt. Sending does not call preflight: retry the original persisted
preparation directly, including older preparations without estimates. Sending
with the same ID returns the existing payment according to Lexe's idempotency
behavior. Do not generate a fresh ID to work around a preflight error.

## Failure behavior

- A missing `high_fees` rejects fast preparation. Slow and normal can still be
  selected; LNI never substitutes another priority's fee.
- Lexe requires the normal-priority transaction to be affordable before it
  returns estimates, so slow can fail even when slow alone would be affordable.
- Preflight failures reject preparation without silently returning a previous
  estimate or submitting a payment. Show the failure and do not label stale
  values as refreshed.
- Stable SDK errors are opaque `anyhow` errors; insufficient funds, an existing
  payment ID, authorization failure, an unsupported endpoint/node version, and
  network failure cannot all be reliably distinguished through its public API.
  LNI reports a sanitized API error identifying these checks, without parsing
  human error text or exposing private upstream payloads. A dedicated funds
  error remains an upstream improvement.

## React Native example

```ts
// Persist a fresh 64-character hex paymentId for this intended payment.
// Reuse it across speed selection and refresh, before submission.
const prepared = await node.prepareOnchainTransaction({
  address: recipientAddress,
  amountSats: 10_000,
  fee: { type: 'speed', speed: 'normal' }, // slow / normal / fast
  idempotencyKey: paymentId,
  description: 'Personal note',
});

const estimatedMinerFeeSats = prepared.feeSats;
const recipientSats = prepared.recipientAmountSats;
const estimatedDebitSats = prepared.totalAmountSats;
// prepared.feeLimitSupported === false

// Persist prepared before submission; reuse it for retries after a timeout.
// Submit only after the user confirms the payment.
const payment = await node.payOnchain(prepared);
const actualMinerFeeSats = payment.feeSats;
```

To show all three speed choices, prepare each with the same payment ID and
handle each failure independently. The node returns all estimates per request;
LNI exposes the selected one through the existing interface. Separate requests
may reflect different wallet states. App UI implementation and live payments
are outside this change.
