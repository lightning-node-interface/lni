LNI - Lightning Node Interface
==============================

LNI - Lightning Node Interface. Connect to the major lightning node implementations with a standard interface. 

- Supports *CLN, *LND, *LNDK, *Phoenixd, *LNURL, *BOLT 11 and *BOLT 12 (WIP). 
- Language Binding support for kotlin, swift, react-native, nodejs (typescript, javaScript). No support for WASM (yet)
- Runs on Android, iOS, Linux, Windows and Mac

<img src="./assets/logo.jpg" alt="logo" style="max-height: 300px;">

### Interface API

#### LND
```rust
let lnd_node = LndNode::new("test_macaroon".to_string(), "https://127.0.0.1:8080".to_string());
let lnd_result =  lnd_node.pay_invoice("invoice".to_string());
println!("Pay LND invoice result {}", lnd_result);
let lnd_txns = lnd_node.get_wallet_transactions("wallet_id".to_string());
lnd_txns.iter().for_each(|txn| {
    println!("LND Transaction amount: {}, date: {}, memo: {}", txn.amount(), txn.date(), txn.memo()); 
});
let lnd_macaroon = lnd_node.key();
```

#### CLN
```rust
let cln_node = ClnNode::new("test_rune".to_string(), "https://127.0.0.1:8081".to_string());
let cln_result =  cln_node.pay_invoice("invoice".to_string());
println!("Pay CLN invoice result {}", cln_result);
let cln_txns = cln_node.get_wallet_transactions("wallet_id".to_string());
cln_txns.iter().for_each(|txn| {
    println!("CLN Transaction amount: {}, date: {}, memo: {}", txn.amount(), txn.date(), txn.memo()); 
});
let cln_rune = cln_node.key();
```


#### Payments
```rust
lni.create_invoice(amount, expiration, memo, BOLT11 | BOLT12)
lni.pay_invoice()
lni.fetch_invoice_from_offer('lno***')
lni.decode_invoice(invoice)
lni.check_invoice_status(invoice)
```

#### Node Management
```
lni.get_info()
lni.get_transactions(limit, skip)
lni.wallet_balance()
```

#### Channel Management
```
lni.fetch_channel_info()
```

#### Event Polling
```
await lni.on_invoice_events(invoice_id, (event) =>{
    console.log("Callback result:", result);
})
```


Event Polling
============
LNI does some simple event polling over https to get some basic invoice status events. 
Polling is used instead of a heavier grpc/pubsub (for now) event system to make sure the lib runs cross platform and stays lightweight. TODO websockets

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
│   ├── lni_uniffi
├── crates
│   ├── lni
│       |─── lnd
│       |─── cln
│       |─── phoenixd
```

Example
========
react-native
```
cd bindings/lni_react_native
cat example/src/App.tsx 
yarn start
```

nodejs 
```
cd bindings/lni_nodejs
cat main.mjs
yarn
yarn build
node main.mjs
```

Bindings
========

- nodejs 
    - napi_rs
    - https://napi.rs/docs/introduction/simple-package
    - `cd bindings/lni_nodejs && cargo build`

- react-native 
    - uniffi-bindgen-react-native 
    - https://jhugman.github.io/uniffi-bindgen-react-native/guides/getting-started.html
    - sample https://github.com/ianthetechie/uniffi-starter  
    - `cd`
- uniffi (kotlin, swift) 
    - https://mozilla.github.io/uniffi-rs/latest/
    - `cd bindings/lni_uniffi && cargo build`


Tor
===
Use Tor socks if connecting to a .onion hidden service by passing in socks5 proxy.


Inspiration
==========
- https://github.com/ZeusLN/zeus/blob/master/backends/LND.ts
- https://github.com/getAlby/hub/tree/master/lnclient
- https://github.com/fedimint/fedimint/blob/master/gateway/ln-gateway/src/lightning/lnd.rs

Project Structure
==================
This project structure was inpired by this https://github.com/ianthetechie/uniffi-starter/ with the intention of 
automating the creation of `react-native-lni` https://jhugman.github.io/uniffi-bindgen-react-native/guides/getting-started.html 

Todo
====
- [X] make interface
- [X] napi-rs for nodejs
- [X] uniffi bindings for Android and IOS
- [X] react-native - uniffi-bindgen-react-native
- [X] async promise architecture for bindings
- [ ] implement lightning nodes
    - [ ] phoenixd
    - [ ] cln
    - [ ] lndk
    - [ ] ldknode
    - [ ] lnd
    - [ ] eclair
    - [ ] Strike? (BOLT 12 support, BOLT 11 blinded path support?)
    - [ ] NWC? (AlbyHub - blinded path support?)


To Research
============
- [X] napi-rs https://napi.rs/docs/introduction/simple-package
- [ ] can we support more complex grpc in 
- [ ] wasm