LNI Remote - Lightning Node Interface Remote
============================================

Remote connect to all the major lightning node implementations with a standard interface. 

- Supports all major nodes - CLN, LND, Phoenixd, *LNDK (WIP) 
- Supports the main protocols - BOLT 11, BOLT 12, and NWC
- **LNURL & Lightning Address support** - Pay to `user@domain.com` or `lnurl1...`
- Also popular REST APIs (Custodial) - Strike, Speed, Blink
- Language Binding support for kotlin, swift, react-native, nodejs (typescript/javascript). No support for WASM (yet)
- Tor support
- Runs on Android, iOS, Linux, Windows and Mac

<img src="./assets/logo.jpg" alt="logo" style="max-height: 300px;">

### Interface API Examples

#### Rust
```rust
let lnd_node = LndNode::new(LndConfig { url, macaroon });
let cln_node = ClnNode::new(ClnConfig { url, rune });

let lnd_node_info = lnd_node.get_info();
let cln_node_info = cln_node.get_info();

let invoice_params = CreateInvoiceParams {
    invoice_type: InvoiceType::Bolt11,
    amount_msats: Some(2000),
    description: Some("your memo"),
    expiry: Some(1743355716),
    ..Default::default()
});

let lnd_invoice = lnd_node.create_invoice(invoice_params).await;
let cln_invoice = cln_node.create_invoice(invoice_params).await;

let pay_invoice_params = PayInvoiceParams{
    invoice: "{lnbc1***}", // BOLT 11 payment request
    fee_limit_percentage: Some(1.0), // 1% fee limit
    allow_self_payment: Some(true), // This setting works with LND, but is simply ignored for CLN etc...
    ..Default::default(),
});

let lnd_pay_invoice = lnd_node.pay_invoice(pay_invoice_params);
let cln_pay_invoice = cln_node.pay_invoice(pay_invoice_params);

let lnd_invoice_status = lnd_node.lookup_invoice("{PAYMENT_HASH}");
let cln_invoice_status = cln_node.lookup_invoice("{PAYMENT_HASH}");

let list_txn_params = ListTransactionsParams {
    from: 0,
    limit: 10,
    payment_hash: None, // Optionally pass in the payment hash, or None to search all
};

let lnd_txns = lnd_node.list_transactions(list_txn_params).await;
let cln_txns = cln_node.list_transactions(list_txn_params).await;

// See the tests for more examples
// LND - https://github.com/lightning-node-interface/lni/blob/master/crates/lni/lnd/lib.rs#L96
// CLN - https://github.com/lightning-node-interface/lni/blob/master/crates/lni/cln/lib.rs#L113
// Phoenixd - https://github.com/lightning-node-interface/lni/blob/master/crates/lni/phoenixd/lib.rs#L100
```

#### Typescript
```typescript
import { createNode, InvoiceType, type BackendNodeConfig } from '@sunnyln/lni';

const backend: BackendNodeConfig = {
    kind: 'lnd',
    config: {
        url: 'https://lnd.example.com',
        macaroon: '...',
    },
};

const node = createNode(backend);
const nodeInfo = await node.getInfo();

const invoiceParams = {
    invoiceType: InvoiceType.Bolt11,
    amountMsats: 2000,
    description: "your memo",
    expiry: 1743355716,
};

const invoice = await node.createInvoice(invoiceParams);

const payInvoiceParams = {
    invoice: invoice.invoice, // BOLT 11 payment request
    feeLimitPercentage: 1, // 1% fee limit
    allowSelfPayment: true, // Used by LND; ignored by nodes that do not support it
};

const payInvoice = await node.payInvoice(payInvoiceParams);

const invoiceStatus = await node.lookupInvoice({ paymentHash: invoice.paymentHash });

const listTxnParams = {
    from: 0,
    limit: 10,
    paymentHash: undefined, // Optionally pass a payment hash to filter
};

const txns = await node.listTransactions(listTxnParams);
```

#### Decode

Decode helpers are pure local functions. They do not require node config.

**TypeScript**
```typescript
import { decode, decodeOffer } from '@sunnyln/lni';

const decodedBolt11 = decode('lnbc...');
const decodedOffer = decodeOffer('lno1...');

console.log(decodedBolt11.paymentRequest);
console.log(decodedBolt11.payment_hash);
console.log(decodedBolt11.amountMsats);
console.log(decodedOffer.paths?.[0]?.blindedHops);
```

BOLT11 decode returns a normalized keyed object instead of a `sections` array. Common fields include `payment_hash`, `payment_secret`, `description`, `amount`, `amountMsats`, `expiry`, `expiresAt`, `feature_bits`, and `route_hints`.

TypeScript node adapters also expose:

```typescript
await node.decode('lnbc...'); // BOLT11 JSON string
await node.decodeOffer('lno1...'); // BOLT12 offer JSON string
```

**Rust**
```rust
let decoded_bolt11 = node.decode("lnbc...".to_string()).await?;
let decoded_offer = node.decode_offer("lno1...".to_string()).await?;
```

BOLT12 offer decode includes normalized blinded paths when present:

If an offer includes `currency`, the decoded `amount` is in that ISO-4217 currency's minor unit, not millisatoshis. In that case `amountMsats` is omitted/null; callers should use the fetched BOLT12 invoice for the actual payable msat amount.

```json
{
  "paths": [
    {
      "introductionNode": { "type": "node_id", "nodeId": "..." },
      "blindingPoint": "...",
      "blindedHops": [
        { "blindedNodeId": "...", "encryptedPayload": "..." }
      ]
    }
  ]
}
```


#### Payments
```rust
// BOLT 11
node.create_invoice(CreateInvoiceParams) -> Result<Transaction, ApiError>
node.pay_invoice(PayInvoiceParams) -> Result<PayInvoiceResponse, ApiError>

// BOLT 12
node.create_offer(params: CreateOfferParams)  -> Result<Offer, ApiError> 

node.get_offer(search: Option<String>) -> Result<Offer, ApiError> // return the first offer or by search id
node.pay_offer(offer: String, amount_msats: i64, payer_note: Option<String>) -> Result<PayInvoiceResponse, ApiError> 
node.list_offers(search: Option<String>) -> Result<Vec<Offer>, ApiError>

// LNURL & Lightning Address (see lnurl module)
lnurl::resolve_to_bolt11(destination, amount_msats) -> Result<String, ApiError>  // Resolve any destination to BOLT11
lnurl::get_payment_info(destination, amount_msats) -> Result<PaymentInfo, ApiError>  // Get min/max amounts
lnurl::detect_payment_type(destination) -> PaymentDestination  // Auto-detect: bolt11|bolt12|lnurl|lightning_address
lnurl::needs_resolution(destination) -> bool  // Check if LNURL resolution needed

// On-chain Bitcoin payments (currently implemented for Strike, Blink, LND, CLN, and Phoenixd)
node.prepare_onchain_transaction(PrepareOnchainTransactionParams) -> Result<OnchainTransaction, ApiError>
node.pay_onchain(OnchainTransaction) -> Result<PayOnchainResponse, ApiError>
node.pay_onchain_with_options(OnchainTransaction, PayOnchainOptions) -> Result<PayOnchainResponse, ApiError>

// Lookup
node.decode(str: String) -> Result<String, ApiError> // Decode BOLT11
node.decode_offer(offer: String) -> Result<String, ApiError> // Decode BOLT12 offer
node.lookup_invoice(payment_hash: String) -> Result<Transaction, ApiError>
node.list_transactions(ListTransactionsParams) -> Result<Transaction, ApiError>
```

On-chain Bitcoin payments
-------------------------

On-chain payments use a prepare-then-pay flow so apps can show fees before executing a payment. This is currently implemented for Strike, Blink, LND, CLN, and Phoenixd. `fee_payer` answers who pays the mining/provider fee:

- `OnchainFeePayer::Sender` means the recipient receives the full requested amount and the sender pays fees on top.
- `OnchainFeePayer::Recipient` means fees are deducted from the requested amount.

On-chain amounts are expressed in sats. Lightning invoice and offer APIs continue to use msats.

**Rust (Strike)**
```rust
use lni::{
    OnchainFeePayer, OnchainFeePreference, OnchainFeePreferenceType, OnchainFeeSpeed,
    PrepareOnchainTransactionParams, StrikeConfig, StrikeNode,
};

let node = StrikeNode::new(StrikeConfig {
    api_key: "...".to_string(),
    ..Default::default()
});

let transaction = node
    .prepare_onchain_transaction(PrepareOnchainTransactionParams {
        address: "bc1q...".to_string(),
        amount_sats: 100_000,
        fee: Some(OnchainFeePreference {
            preference_type: OnchainFeePreferenceType::Speed,
            speed: Some(OnchainFeeSpeed::Normal),
            target_conf: None,
            sats_per_vbyte: None,
            backend: None,
        }),
        fee_payer: Some(OnchainFeePayer::Sender),
        description: Some("cold storage".to_string()),
        idempotency_key: None,
    })
    .await?;

// Show transaction.fee_sats, transaction.total_amount_sats, and transaction.expires_at to the user.

let payment = node.pay_onchain(transaction).await?;
```

**TypeScript (Strike)**
```typescript
import { StrikeNode } from '@sunnyln/lni';

const node = new StrikeNode({ apiKey: '...' });

const transaction = await node.prepareOnchainTransaction({
    address: 'bc1q...',
    amountSats: 100_000,
    fee: { type: 'speed', speed: 'normal' },
    feePayer: 'sender',
    description: 'cold storage',
});

// Show transaction.feeSats, transaction.totalAmountSats, and transaction.expiresAt to the user.

const payment = await node.payOnchain(transaction);
```

`pay_onchain` / `payOnchain` enforces the shared default fee guardrail: `DEFAULT_ONCHAIN_MAX_FEE_SATS` / `DEFAULT_ONCHAIN_MAX_FEE_PERCENT` in Rust and `DEFAULT_ONCHAIN_FEE_GUARDRAIL` in TypeScript. The current defaults are `25_000` sats and `25%` of the send amount. It fails closed when the prepared transaction has no fee quote, such as a recovered duplicate quote that only includes the original quote id.

Use custom limits to make the guardrail stricter or looser:

```typescript
await node.payOnchain(transaction, {
    feeGuardrail: {
        maxFeeSats: 5_000,
        maxFeePercent: 10,
    },
});
```

Disable it only when the caller has shown the fee another way and intentionally accepts the risk:

```typescript
await node.payOnchain(transaction, {
    dangerouslyDisableFeeGuardrail: true,
});
```

```rust
use lni::{OnchainFeeGuardrail, PayOnchainOptions};

let payment = node
    .pay_onchain_with_options(
        transaction,
        PayOnchainOptions {
            fee_guardrail: Some(OnchainFeeGuardrail {
                max_fee_sats: Some(5_000),
                max_fee_percent: Some(10.0),
            }),
            dangerously_disable_fee_guardrail: false,
        },
    )
    .await?;
```

For Strike, LNI maps `fast` to `tier_fast`, `normal` to `tier_standard`, and `slow` / `free` to `tier_free`. Use `fee: { type: "backend", value: "tier_..." }` in TypeScript, or `OnchainFeePreferenceType::Backend` with `backend: Some("tier_...")` in Rust, to pass a Strike tier id directly.

For Blink, LNI maps `fast`, `normal`, and `slow` to Blink's `FAST`, `MEDIUM`, and `SLOW` payout speeds. Blink does not support `free`, target-confirmation, sats/vbyte, backend fee preferences, or recipient-paid fees for on-chain sends.

For LND, LNI maps `fast`, `normal`, and `slow` to confirmation targets of `1`, `6`, and `12` blocks. LND also supports explicit target confirmation and sats/vbyte fee preferences. LND does not support `free`, backend fee preferences, or recipient-paid fees for on-chain sends.

For CLN, LNI maps `fast`, `normal`, and `slow` to CLN's `urgent`, `normal`, and `slow` feerates. CLN also supports explicit sats/vbyte fee preferences and raw backend feerate strings such as `1000perkw` or `normal`. CLN does not support `free`, target-confirmation fee preferences, or recipient-paid fees. CLN prepares on-chain transactions with `txprepare`, which reserves wallet inputs until `txsend`, `txdiscard`, or lightningd restart.

For Phoenixd, LNI requires an explicit sats/vbyte fee preference because Phoenixd's `sendtoaddress` endpoint requires `feerateSatByte`. Use `fee: { type: "satsPerVbyte", satsPerVbyte: 12 }` in TypeScript, or `OnchainFeePreferenceType::SatsPerVbyte` in Rust. Phoenixd does not support `default`, speed, target-confirmation, or recipient-paid fees for on-chain sends. Phoenixd does not expose a separate quote endpoint or final mining fee quote, so `pay_onchain` / `payOnchain` requires `dangerously_disable_fee_guardrail: true` after the caller has chosen and accepted the feerate.

LND payment macaroons
---------------------

For LNI payment flows, avoid using `admin.macaroon` in apps. Bake a narrower macaroon with the permissions LNI needs for Lightning sends and on-chain sends:

```bash
lncli bakemacaroon \
  --save_to ./lni-payments.macaroon \
  info:read \
  offchain:read \
  offchain:write \
  onchain:read \
  onchain:write
```

For on-chain-only testing, use a macaroon with just the LND wallet permissions:

```bash
lncli bakemacaroon \
  --save_to ./lni-onchain.macaroon \
  info:read \
  onchain:read \
  onchain:write
```

Plain `lncli bakemacaroon` macaroons do not enforce a max-spend budget; they only grant or restrict permissions. Enforce per-payment or rolling-window budgets in the app before calling `payInvoice` / `pay_invoice` or `payOnchain` / `pay_onchain`.

If you run `litd`, LND Accounts can create an account-restricted macaroon with an enforced off-chain balance:

```bash
litcli accounts create 50000 --save_to ./lni-account.macaroon
```

That account balance limits Lightning payments, including routing fees. It does not provide an on-chain send budget; account-restricted users cannot spend the node's on-chain wallet. LNI's on-chain fee guardrail limits unusually high fees; it is not a total spend budget.

Testing on-chain payments
-------------------------

On-chain integration tests should use the same safety pattern for every provider LNI supports now or adds later, such as Strike, Blink, LND, or CLN:

- Quote-only tests are safe to run with normal provider credentials and a test address.
- Broadcast tests send real bitcoin, must be marked ignored where the test runner supports it, and must require explicit confirmation env vars.
- Provider-specific env vars should use the provider prefix, for example `STRIKE_...`, `BLINK_...`, `LND_...`, or `CLN_...`.

Add the provider values to `crates/lni/.env`:

```env
# Strike
STRIKE_API_KEY=...
STRIKE_ONCHAIN_TEST_ADDRESS=bc1q...
STRIKE_ONCHAIN_AMOUNT_SATS=76000

# Blink
BLINK_API_KEY=...
BLINK_BASE_URL=https://api.blink.sv/graphql
BLINK_ONCHAIN_TEST_ADDRESS=bc1q...
BLINK_ONCHAIN_AMOUNT_SATS=10000

# LND
LND_URL=https://127.0.0.1:8080
LND_MACAROON=...
LND_ONCHAIN_TEST_ADDRESS=bc1q...
LND_ONCHAIN_AMOUNT_SATS=10000

# CLN
CLN_URL=https://127.0.0.1:3010
CLN_RUNE=...
CLN_ONCHAIN_TEST_ADDRESS=bc1q...
CLN_ONCHAIN_AMOUNT_SATS=10000

# Phoenixd
PHOENIXD_URL=http://127.0.0.1:9740
PHOENIXD_PASSWORD=...
PHOENIXD_ONCHAIN_TEST_ADDRESS=bc1q...
PHOENIXD_ONCHAIN_AMOUNT_SATS=10000
PHOENIXD_ONCHAIN_FEERATE_SAT_BYTE=12
```

Run TypeScript integration tests:

```bash
cd bindings/typescript
npm run test:integration:strike
npm run test:integration:blink
npm run test:integration:lnd
npm run test:integration:cln
npm run test:integration:phoenixd
```

Blink, LND, CLN, and Phoenixd TypeScript and Rust tests prepare a quote first, then skip the broadcast unless the confirmation variables are set. CLN prepare-only tests call `txdiscard` to release reserved inputs. Run Rust tests from `crates/lni` so `dotenv` loads `crates/lni/.env`:

```bash
cd crates/lni
cargo test blink::lib::tests::test_pay_onchain_e2e -- --nocapture
cargo test lnd::lib::tests::test_pay_onchain_e2e -- --nocapture
cargo test cln::lib::tests::test_pay_onchain_e2e -- --nocapture
cargo test phoenixd::lib::tests::test_pay_onchain_e2e -- --nocapture
```

Broadcast tests require a second pair of env vars for the provider being tested:

```env
STRIKE_RUN_ONCHAIN_SEND=true
STRIKE_ONCHAIN_SEND_CONFIRM=I_UNDERSTAND_THIS_BROADCASTS_BITCOIN

BLINK_RUN_ONCHAIN_SEND=true
BLINK_ONCHAIN_SEND_CONFIRM=I_UNDERSTAND_THIS_BROADCASTS_BITCOIN

LND_RUN_ONCHAIN_SEND=true
LND_ONCHAIN_SEND_CONFIRM=I_UNDERSTAND_THIS_BROADCASTS_BITCOIN

CLN_RUN_ONCHAIN_SEND=true
CLN_ONCHAIN_SEND_CONFIRM=I_UNDERSTAND_THIS_BROADCASTS_BITCOIN

PHOENIXD_RUN_ONCHAIN_SEND=true
PHOENIXD_ONCHAIN_SEND_CONFIRM=I_UNDERSTAND_THIS_BROADCASTS_BITCOIN
```

Run broadcast tests only when you intentionally want to send real bitcoin:

```bash
cd bindings/typescript
npm run test:integration:strike
npm run test:integration:blink
npm run test:integration:lnd
npm run test:integration:cln
npm run test:integration:phoenixd

cd crates/lni
cargo test strike::lib::tests::test_pay_onchain_e2e -- --ignored --nocapture
cargo test blink::lib::tests::test_pay_onchain_e2e -- --nocapture
cargo test lnd::lib::tests::test_pay_onchain_e2e -- --nocapture
cargo test cln::lib::tests::test_pay_onchain_e2e -- --nocapture
cargo test phoenixd::lib::tests::test_pay_onchain_e2e -- --nocapture
```

Without `-- --ignored`, Strike's Rust broadcast test is discovered but skipped. Blink, LND, CLN, and Phoenixd Rust tests are not ignored; they prepare a quote and return before broadcasting unless confirmation is present. TypeScript broadcast tests are skipped unless the provider-specific confirmation variables are present. Without the provider-specific `*_RUN_ONCHAIN_SEND=true` and `*_ONCHAIN_SEND_CONFIRM=I_UNDERSTAND_THIS_BROADCASTS_BITCOIN`, broadcast tests refuse to broadcast.

#### Node Management
```rust
node.get_info() -> Result<NodeInfo, ApiError> // returns NodeInfo and balances
```

#### Channel Management
```rust
// TODO - Not implemented
node.channel_info()
```

#### LNURL & Lightning Address

LNI supports paying to Lightning Addresses (`user@domain.com`) and LNURL endpoints. The `lnurl` module handles automatic resolution to BOLT11 invoices.

**Rust**
```rust
use lni::lnurl::{resolve_to_bolt11, get_payment_info, PaymentDestination};

// Auto-detect payment type
let dest = PaymentDestination::parse("nicktee@strike.me")?;
// Returns: LightningAddress { user: "nicktee", domain: "strike.me" }

// Check if destination needs LNURL resolution
let needs_resolve = lni::lnurl::needs_resolution("nicktee@strike.me"); // true
let needs_resolve = lni::lnurl::needs_resolution("lnbc10u1..."); // false

// Get payment info (fetches min/max amounts from LNURL endpoint)
let info = get_payment_info("nicktee@strike.me", Some(100_000)).await?;
println!("Min: {} msats, Max: {} msats", info.min_sendable_msats, info.max_sendable_msats);

// Resolve to BOLT11 invoice and pay
let bolt11 = resolve_to_bolt11("nicktee@strike.me", Some(100_000)).await?;
node.pay_invoice(PayInvoiceParams { invoice: bolt11, ..Default::default() }).await?;
```

**TypeScript (Node.js)**
```typescript
import { detectPaymentType, needsResolution, resolveToBolt11, getPaymentInfo } from '@sunnyln/lni';

// Auto-detect payment type
const type = detectPaymentType('nicktee@strike.me'); // "lightning_address"
const type2 = detectPaymentType('lnbc10u1...'); // "bolt11"
const type3 = detectPaymentType('lno1pg...'); // "bolt12"

// Check if needs resolution
const needsResolve = needsResolution('nicktee@strike.me'); // true

// Get payment info
const info = await getPaymentInfo('nicktee@strike.me', 100_000n);
console.log(`Min: ${info.minSendableMsats}, Max: ${info.maxSendableMsats}`);

// Resolve and pay
const bolt11 = await resolveToBolt11('nicktee@strike.me', 100_000n);
await node.payInvoice({ invoice: bolt11 });
```

**Supported Destinations**
| Type | Example | Resolution |
|------|---------|------------|
| BOLT11 | `lnbc10u1p5...` | None (direct pay) |
| BOLT12 | `lno1pg...` | None (use `payOffer`) |
| Lightning Address | `user@domain.com` | LNURL-pay → BOLT11 |
| LNURL | `lnurl1...` | Decode → LNURL-pay → BOLT11 |

#### Event Polling

LNI does some simple event polling over http to get some basic invoice status events. 
Polling is used instead of a heavier grpc/pubsub (for now) to make sure the lib runs cross platform and stays lightweight.

Typescript for react-native
```typescript
await node.onInvoiceEvents(
    // polling params
    {
        paymentHash: TEST_PAYMENT_HASH,
        pollingDelaySec: BigInt(3), // poll every 3 seconds
        maxPollingSec: BigInt(60), // for up to 60 seconds
    },
    // callbacks for each polling round
    // The polling ends if success or maxPollingSec timeout is hit
    {
        success(transaction: Transaction | undefined): void {
            console.log('Received success invoice event:', transaction);
            setResult('Success');
        },
        pending(transaction: Transaction | undefined): void {
            console.log('Received pending event:', transaction);
        },
        failure(transaction: Transaction | undefined): void {
            console.log('Received failure event:', transaction);
        },
    }
);
```

Typescript for nodejs
```typescript
await node.onInvoiceEvents(
    // polling params
    {
        paymentHash: process.env.LND_TEST_PAYMENT_HASH,
        pollingDelaySec: 4,
        maxPollingSec: 60,
    }, 
    // callback for each polling round
    // The polling ends if success or maxPollingSec timeout is hit
    (status, tx) => {
        console.log("Invoice event:", status, tx);
    }
);
```

Rust
```rust
struct OnInvoiceEventCallback {}
impl crate::types::OnInvoiceEventCallback for OnInvoiceEventCallback {
    fn success(&self, transaction: Option<Transaction>) {
        println!("success");
    }
    fn pending(&self, transaction: Option<Transaction>) {
        println!("pending");
    }
    fn failure(&self, transaction: Option<Transaction>) {
        println!("epic fail");
    }
}
let params = crate::types::OnInvoiceEventParams {
    payment_hash: TEST_PAYMENT_HASH.to_string(),
    polling_delay_sec: 3,
    max_polling_sec: 60,
};
let callback = OnInvoiceEventCallback {};
NODE.on_invoice_events(params, Arc::new(callback));
```


Build
=======
```
cd crates/lni
cargo clean
cargo build
cargo test
```

Folder Structure
================
```
lni
├── bindings
│   ├── lni_nodejs
│   ├── lni_react_native
│   ├── typescript
├── crates
│   ├── lni
│       |─── lnd
│       |─── cln
│       |─── phoenixd
│       |─── nwc
│       |─── strike
│       |─── speed
│       |─── blink
```

### TypeScript (frontend)
```sh
npm install @sunnyln/lni
```

Build from source:
```sh
cd bindings/typescript
npm install
npm run typecheck
npm run build
```

Install TypeScript binding from GitHub repo ([lightning-node-interface/lni](https://github.com/lightning-node-interface/lni)):
```sh
TMP_DIR=$(mktemp -d) && git clone --depth 1 https://github.com/lightning-node-interface/lni.git "$TMP_DIR/lni" && npm install "$TMP_DIR/lni/bindings/typescript"
```

Why this form: `bindings/typescript` is a subfolder package in a monorepo, so install is done from the cloned path.

Example
========
#### react-native
```sh
cd bindings/lni_react_native
cat example/src/App.tsx 
yarn start
```

`*troubleshooting react-natve`: 
- if you get an error like `uniffiEnsureInitialized`, then you might need to kill the app and restart. (ios simulator - double tap home button then swipe away app)
- try updating the pods for ios `cd example/ios && pod install --repo-update && cd ../`
- for ios open the xcode app - lni/bindings/lni_react_native/example/ios/LniExample.xcworkspace
    - Then click the project in the left "LniExample" to select for the context menu
    - In the top click "Product -> Clean build folder" and then build and run
- Lastly uninstalling the app from the mobile device

#### kotlin (android)
```sh
cd bindings/kotlin

# Build Kotlin bindings and Android native libraries
./build.sh --release --android

# Run the example Android app
cd example
./gradlew :app:installDebug

# Or launch emulator first
emulator -avd YOUR_AVD_NAME &
./gradlew :app:installDebug
adb shell am start -n com.lni.example/.MainActivity
```

#### nodejs 
```sh
cd bindings/lni_nodejs
cat main.mjs
yarn
# then open ../../crates/lni/Cargo.toml and comment out #crate-type = ["staticlib"]
yarn build
node main.mjs
```

#### .env
```sh
# These env vars are used for tests

TEST_RECEIVER_OFFER=lnotestoffer***
PHOENIX_MOBILE_OFFER=lnotestoffer***

PHOENIXD_URL=http://localhost:9740
PHOENIXD_PASSWORD=YOUR_HTTP_PASSWORD
PHOENIXD_TEST_PAYMENT_HASH=YOUR_TEST_PAYMENT_HASH

CLN_URL=http://localhost:3010
CLN_RUNE=YOUR_RUNE
CLN_TEST_PAYMENT_HASH=YOUR_HASH
CLN_OFER=lnotestoffer***

LND_URL=""
LND_MACAROON=""
LND_TEST_PAYMENT_HASH=""
LND_TEST_PAYMENT_REQUEST=""

NWC_URI="nostr+walletconnect://*"
NWC_TEST_PAYMENT_HASH=""
NWC_TEST_PAYMENT_REQUEST=""

STRIKE_API_KEY=""
STRIKE_TEST_PAYMENT_HASH=""
STRIKE_TEST_PAYMENT_REQUEST=""

BLINK_API_KEY=""
BLINK_TEST_PAYMENT_HASH=""
BLINK_TEST_PAYMENT_REQUEST=""

SPEED_API_KEY=""
SPEED_TEST_PAYMENT_HASH=""
SPEED_TEST_PAYMENT_REQUEST=""
```

Language Bindings
=================

- ## nodejs 
    - napi_rs
    - https://napi.rs/docs/introduction/simple-package
    - `cd bindings/lni_nodejs && cargo clean && cargo build --release && yarn && yarn build`
    - test `node main.mjs`
    - ### native modules (electron, vercel etc..)
        - if you want to use the native node module (maybe for an electron app) you can reference the file `bindings/lni_nodejs/lni_js.${platform}-${arch}.node`. It would look something like in your project:
            ```typescript
            const path = require("path");
            const os = require("os");
            const platform = os.platform();
            const arch = os.arch();
            const nativeModulePath = path.join(
            __dirname,
            `../../code/lni/bindings/lni_nodejs/lni_js.${platform}-${arch}.node`
            );
            const { PhoenixdNode } = require(nativeModulePath);
            ```
- ## react-native 
    - `uniffi-bindgen-react-native` lib
    - https://jhugman.github.io/uniffi-bindgen-react-native/guides/getting-started.html
    - sample https://github.com/ianthetechie/uniffi-starter  
    - `cd bindings/lni_react_native && ./build.sh`

    **To include it in your react-native project:**
    1. In this project run `cd bindings/lni_react_native && ./build.sh && yarn package --out lni.tgz`
    2. This creates a `lni.tgz`. Copy this to your target React Native project in the root.
    3. Prereq: In the React Native project that you want to include `lni`, make sure the new architecure is enabled. And include `lni` in the podfile
        - android 
        
            `build.gradle`
            ```gradle
            defaultConfig {
                // Explicitly set New Architecture flag
                buildConfigField "boolean", "IS_NEW_ARCHITECTURE_ENABLED", (findProperty("newArchEnabled") ?: "false")
                buildConfigField "boolean", "IS_HERMES_ENABLED", (findProperty("hermesEnabled") ?: "true")
            }
            ```

            `gradle.properties`
            ```
            newArchEnabled=true
            hermesEnabled=true
            ```

            `MainApplication.kt`
            ```kt
            override fun onCreate() {
                super.onCreate()
                SoLoader.init(this, OpenSourceMergedSoMapping)
                if (BuildConfig.IS_NEW_ARCHITECTURE_ENABLED) {
                    // If you opted-in for the New Architecture, we load the native entry point for this app.
                    load(bridgelessEnabled = true)
                }
            }
            ```

        - ios
            
            podfile
            ```
            ENV['RCT_NEW_ARCH_ENABLED'] = '1'


            pod 'lni-react-native', :path => '../node_modules/lni_react_native'

            # Fix for New Architecture - remove compiler overrides that conflict with -index-store-path
            installer.pods_project.targets.each do |target|
                target.build_configurations.each do |config|
                    # Remove custom compiler settings that interfere with New Architecture
                    config.build_settings.delete("CC")
                    config.build_settings.delete("LD")
                    config.build_settings.delete("CXX")
                    config.build_settings.delete("LDPLUSPLUS")
                    
                    # Disable index store path for New Architecture compatibility
                    config.build_settings["COMPILER_INDEX_STORE_ENABLE"] = "NO"
                end
            end
            ```
    4. `yarn add ./lni.tgz`
    5. `cd ios && pod install`
    6. `yarn clean`
        package.json
        ```
            "clean": "yarn clean:rn && yarn clean:ios && yarn clean:android",
            "clean:rn": "watchman watch-del-all && npx del-cli node_modules && yarn",
            "clean:android": "cd android && npx del-cli build app/build app/release",
            "clean:ios": "cd ios && npx del-cli Pods build && pod install", 
        ```
    
        ***Troubleshooting (clean)***
            ```
            rm -rf node_modules/.cache ios/build ios/Pods ios/Podfile.lock android/build android/app/build 

            rm -rf ~/Library/Developer/Xcode/DerivedData/YOUR_APP_NAME-* 2>/dev/null;

            cd ios && pod install && cd ..

            cd android && ./gradlew clean && cd ..

            npx react-native start --reset-cache

            yarn

            # if the app does not recognize change try deleting the lni node module
            rm node_modules/lni_react_native

            ```
            ios open xcworkspace in xcode and `Product > Clean` and build project


- ## uniffi (kotlin, swift) 
    - https://mozilla.github.io/uniffi-rs/latest/
    - Uses decorators like `#[cfg_attr(feature = "uniffi", uniffi::export)]` to foreign codegen 

    ### Kotlin Bindings
    
    **Prerequisites:**
    1. Install Android NDK via Android Studio SDK Manager
    2. Add Rust Android targets:
        ```bash
        rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android i686-linux-android
        ```
    3. Configure cargo linker in `~/.cargo/config.toml`:
        ```toml
        [target.aarch64-linux-android]
        linker = "/Users/YOUR_USER/Library/Android/sdk/ndk/VERSION/toolchains/llvm/prebuilt/darwin-x86_64/bin/aarch64-linux-android21-clang"
        
        [target.x86_64-linux-android]
        linker = "/Users/YOUR_USER/Library/Android/sdk/ndk/VERSION/toolchains/llvm/prebuilt/darwin-x86_64/bin/x86_64-linux-android21-clang"
        
        [target.armv7-linux-androideabi]
        linker = "/Users/YOUR_USER/Library/Android/sdk/ndk/VERSION/toolchains/llvm/prebuilt/darwin-x86_64/bin/armv7a-linux-androideabi21-clang"
        
        [target.i686-linux-android]
        linker = "/Users/YOUR_USER/Library/Android/sdk/ndk/VERSION/toolchains/llvm/prebuilt/darwin-x86_64/bin/i686-linux-android21-clang"
        ```
        Replace `YOUR_USER` and `VERSION` with your actual values. To find them:
        ```bash
        # Find your username
        whoami
        
        # List available NDK versions
        ls ~/Library/Android/sdk/ndk/
        ```
        Then use the latest version (e.g., `28.2.13676358`).

    **Build Kotlin Bindings:**
    ```bash
    cd bindings/kotlin
    
    # Generate Kotlin bindings only
    ./build.sh --release
    
    # Build for all Android architectures + generate bindings
    ./build.sh --release --android
    ```

    This generates:
    - Kotlin source files in `bindings/kotlin/src/main/kotlin/`
    - Native `.so` libraries in `bindings/kotlin/example/app/src/main/jniLibs/`
        - `arm64-v8a/` - ARM64 devices & Apple Silicon emulators
        - `armeabi-v7a/` - Older ARM devices
        - `x86_64/` - Intel emulators
        - `x86/` - Older Intel emulators

    **Run Example Android App:**
    ```bash
    cd bindings/kotlin/example
    
    # Build and install on connected device/emulator
    ./gradlew :app:installDebug
    
    # Or open in Android Studio
    # File > Open > Navigate to bindings/kotlin/example
    ```

    The example app demonstrates:
    - Strike API integration with balance lookup
    - Blink and NWC node creation
    - Proper async/coroutine usage with LNI

    **Kotlin Usage Example:**
    ```kotlin
    import uniffi.lni.*
    import kotlinx.coroutines.Dispatchers
    import kotlinx.coroutines.withContext

    // Get Strike balance
    suspend fun getStrikeBalance(apiKey: String) = withContext(Dispatchers.IO) {
        val config = StrikeConfig(
            apiKey = apiKey,
            baseUrl = null,
            httpTimeout = null,
            socks5Proxy = null,
            acceptInvalidCerts = null
        )
        val node = StrikeNode(config)
        
        try {
            val info = node.getInfo()
            println("Balance: ${info.sendBalanceMsat / 1000} sats")
        } catch (e: ApiException) {
            println("API Error: ${e.message}")
        }
    }

    // Create invoice with CLN
    suspend fun createInvoice() = withContext(Dispatchers.IO) {
        val config = ClnConfig(
            url = "http://localhost:3010",
            rune = "YOUR_RUNE",
            httpTimeout = null,
            socks5Proxy = null,
            acceptInvalidCerts = null
        )
        val node = ClnNode(config)
        
        val params = CreateInvoiceParams(
            invoiceType = InvoiceType.BOLT11,
            amountMsats = 10000L,
            description = "Test invoice",
            expiry = null,
            descriptionHash = null,
            fallbackAddress = null,
            paymentPreimage = null
        )
        
        val invoice = node.createInvoice(params)
        println("Invoice: ${invoice.invoice}")
    }
    ```

    **Include in Your Android Project:**
    
    1. Copy the generated Kotlin sources from `bindings/kotlin/src/main/kotlin/uniffi/` to your project
    2. Copy the native libraries from `bindings/kotlin/example/app/src/main/jniLibs/` to your app's `src/main/jniLibs/`
    3. Add JNA dependency to your `build.gradle.kts`:
        ```kotlin
        dependencies {
            implementation("net.java.dev.jna:jna:5.13.0@aar")
        }
        ```

    See `bindings/kotlin/example/app/src/main/kotlin/com/lni/example/MainActivity.kt` for a complete Android Compose example.

    ### Swift Bindings
    
    See [Swift bindings documentation](bindings/swift/example/README.md) for setup instructions and iOS usage examples.

Shared Foreign Language Objects
===============================
If you do not want to copy objects to the foreign language bindings we can simply use the features `napi_rs` or `uniffi`
to turn on or off language specific decorators and then implement them in their respective bindings project.

Example:
```
#[cfg(feature = "napi_rs")]
use napi_derive::napi;

#[cfg_attr(feature = "napi_rs", napi(object))]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct PhoenixdConfig {
    pub url: String,
    pub password: String,
}

#[cfg_attr(feature = "napi_rs", napi(object))]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct PhoenixdNode {
    pub url: String,
    pub password: String,
}
```

Tor
===
Use the Tor Socks5 proxy settings if you are connecting to a `.onion` hidden service. Make sure to include the `"h"` in `"socks5h://"` to resolve onion addresses properly. You can start up a Tor Socks5 proxy easily using Arti https://tpo.pages.torproject.net/core/arti/

example
```rust
LndNode::new(LndConfig {
    url: "https://YOUR_LND_ONION_ADDRESS.onion",
    macaroon: "YOUR_MACAROON",
    socks5_proxy: Some("socks5h://127.0.0.1:9150".to_string()),
    accept_invalid_certs: Some(true),
    ..Default::default()
})
```

**Strike with SOCKS5 Proxy Support:**
Strike now supports the same SOCKS5 proxy and invalid certificate settings as LND:

```rust
StrikeNode::new(StrikeConfig {
    base_url: Some("https://api.strike.me/v1".to_string()),
    api_key: "YOUR_API_KEY".to_string(),
    http_timeout: Some(120),
    socks5_proxy: Some("socks5h://127.0.0.1:9150".to_string()), // Tor proxy
    accept_invalid_certs: Some(true), // Accept self-signed certificates
})
```

This is useful for:
- Connecting through Tor for privacy
- Testing with self-signed certificates in development
- Connecting through corporate proxies that use invalid certificates


Inspiration
==========
- https://github.com/ZeusLN/zeus/blob/master/backends/LND.ts
- https://github.com/getAlby/hub/tree/master/lnclient
- https://github.com/fedimint/fedimint/blob/master/gateway/ln-gateway/src/lightning/lnd.rs

Project Structure
==================
This project structure was inpired by this https://github.com/ianthetechie/uniffi-starter/ with the intention of 
automating the creation of `lni_react_native` https://jhugman.github.io/uniffi-bindgen-react-native/guides/getting-started.html 

LNI Sequence Diagram
==================
```mermaid
sequenceDiagram
    participant App as Application (JS/Swift/Kotlin)
    participant Binding as Language Binding (Node.js/React Native/UniFfi)
    participant LNI as LNI Core (Rust)
    participant Node as Lightning Node Implementation (CLN/LND/Phoenixd)
    participant LN as Lightning Node (REST/gRPC API)

    App->>Binding: Create config (URL, authentication)
    Binding->>LNI: Initialize node with config
    LNI->>LNI: Create node object (PhoenixdNode, ClnNode, etc.)
    
    Note over App,LN: Example: Get Node Info
    
    App->>Binding: node.getInfo()
    Binding->>LNI: get_info()
    LNI->>Node: api::get_info(url, auth)
    Node->>LN: HTTP/REST request to /v1/getinfo
    LN-->>Node: Response (JSON)
    Node->>Node: Parse response
    Node-->>LNI: NodeInfo object
    LNI-->>Binding: NodeInfo struct
    Binding-->>App: NodeInfo object

    Note over App,LN: Example: Create Invoice
    
    App->>Binding: node.createInvoice(params)
    Binding->>LNI: create_invoice(params)
    LNI->>Node: api::create_invoice(url, auth, params)
    Node->>LN: HTTP/REST request to create invoice
    LN-->>Node: Response with invoice data
    Node->>Node: Parse response
    Node-->>LNI: Transaction object
    LNI-->>Binding: Transaction struct
    Binding-->>App: Transaction object

    Note over App,LN: Example: Pay Invoice/Offer
    
    App->>Binding: node.payOffer(offer, amount, note)
    Binding->>LNI: pay_offer(offer, amount, note)
    LNI->>Node: api::pay_offer(url, auth, offer, amount, note)
    Node->>LN: HTTP/REST request to pay
    LN-->>Node: Payment response
    Node->>Node: Parse response
    Node-->>LNI: PayInvoiceResponse object
    LNI-->>Binding: PayInvoiceResponse struct
    Binding-->>App: PayInvoiceResponse object
```

Todo
====
- [ ] bug in fees (bad conversion? msats)
- [X] make interface
- [X] napi-rs for nodejs
- [X] uniffi bindings for Android and IOS
- [X] react-native - uniffi-bindgen-react-native
- [X] async promise architecture for bindings
- [X] Tor Socks5 fetch https://tpo.pages.torproject.net/core/arti/guides/starting-arti
- [X] Simple event polling
- [ ] implement lightning nodes
    - [X] phoenixd
    - [X] cln
    - [X] lnd
    - [ ] lndk
    - [ ] ldk_node? (Alby hub?)
    - [ ] eclair
    - [X] strike (BOLT 12 support?, BOLT 11 blinded path support?)
    - [X] nwc
    - [X] blink
    - [X] speed
- [ ] test zero amount invoices
- [ ] add status to global Transaction type to help with custodial wallets (status = "PENDING", "FAILED", "PAID")
- [X] add create_offer and remove the logic from get_offer. this should take in params like description and amount (or zero)
    - [X] Phoenix https://phoenix.acinq.co/server/api#create-bolt12-offer 
    - [X] CLN https://docs.corelightning.org/reference/offer 

To Research
============
- [X] napi-rs https://napi.rs/docs/introduction/simple-package
- [ ] can we support more complex grpc in 
- [ ] wasm?
