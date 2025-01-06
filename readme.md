LNI - Lightning Node Interface
==============================

<img src="./assets/logo.jpg" alt="logo" style="max-height: 300px;">

LNI - Lightning Node Interface. Connect to the major lightning node implementations with a standard interface. CLN, LND, LNDK, Phoenixd, LNURL (BOLT 11 and BOLT 12). Binding support for Android, IOS, React-Native, Typescript, JavaScript, Linux, Windows and Mac

```
### Examples

# LND
let lnd_node = LndNode::new("test_macaroon".to_string(), "https://127.0.0.1:8080".to_string());
let lnd_result =  lnd_node.pay_invoice("invoice".to_string());
println!("Pay LND invoice result {}", lnd_result);
let lnd_txns = lnd_node.get_wallet_transactions("wallet_id".to_string());
lnd_txns.iter().for_each(|txn| {
    println!("LND Transaction amount: {}, date: {}, memo: {}", txn.amount(), txn.date(), txn.memo()); 
});
let lnd_macaroon = lnd_node.key();

# CLN
let cln_node = ClnNode::new("test_rune".to_string(), "https://127.0.0.1:8081".to_string());
let cln_result =  cln_node.pay_invoice("invoice".to_string());
println!("Pay CLN invoice result {}", cln_result);
let cln_txns = cln_node.get_wallet_transactions("wallet_id".to_string());
cln_txns.iter().for_each(|txn| {
    println!("CLN Transaction amount: {}, date: {}, memo: {}", txn.amount(), txn.date(), txn.memo()); 
});
let cln_rune = cln_node.key();


### Payments
lni.create_invoice(amount, expiration, memo, BOLT11 | BOLT12)
lni.pay_invoice()
lni.fetch_invoice_from_offer('lno***')
lni.decode_invoice(invoice)
lni.check_invoice_status(invoice)

### Node Management
lni.get_info()
lni.get_transactions(limit, skip)
lni.wallet_balance()

### Channel Management
lni.fetch_channel_info()

### Events
lni.on_payment_recievced(invoice_info)

```

Dev
====
```
cargo clean
scripts/deps.sh
cargo build
cargo test
cargo run
```

Bindings
========
- Wasm for Javascript and Typescript
```
scripts/wasm.sh
```
- uniffi for Android and IOS

Tor
===
Use Tor socks if connecting to a .onion hidden service by passing in socks5 proxy.


Inpiration
==========
- https://github.com/ZeusLN/zeus/blob/master/backends/LND.ts
- https://github.com/fedimint/fedimint/blob/master/gateway/ln-gateway/src/lightning/lnd.rs