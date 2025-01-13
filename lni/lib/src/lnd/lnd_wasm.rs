#![cfg(feature = "wasm")] // Only compile if feature=wasm

use wasm_bindgen::prelude::*;
use crate::types::Transaction;
use super::lnd::LndNode;

#[wasm_bindgen]
pub struct WasmLndNode {
    inner: LndNode,
}

#[wasm_bindgen]
impl WasmLndNode {
    #[wasm_bindgen(constructor)]
    pub fn new(
        macaroon: String,
        url: String,
        polling_interval: Option<u64>,
        polling_timeout: Option<u64>,
    ) -> WasmLndNode {
        let node = LndNode::new_inherent(macaroon, url, polling_interval, polling_timeout);
        WasmLndNode { inner: node }
    }

    #[wasm_bindgen]
    pub fn key(&self) -> String {
        self.inner.key_inherent()
    }

    #[wasm_bindgen]
    pub fn url(&self) -> String {
        self.inner.url_inherent()
    }

    #[wasm_bindgen]
    pub fn polling_interval(&self) -> u64 {
        self.inner.polling_interval_inherent()
    }

    #[wasm_bindgen]
    pub fn polling_timeout(&self) -> u64 {
        self.inner.polling_timeout_inherent()
    }

    #[wasm_bindgen]
    pub fn get_wallet_transactions(&self) -> JsValue  {
        // Convert the Vec<Transaction> to JsValue (e.g. JSON)
        let txs = self.inner.get_wallet_transactions_inherent();
        serde_wasm_bindgen::to_value(&txs).unwrap()
    }

    #[wasm_bindgen]
    pub fn check_payment_status(&self, payment_id: String) -> String {
        self.inner.check_payment_status_inherent(&payment_id)
    }

    // For async in wasm-bindgen, you'd typically do:
    // #[wasm_bindgen]
    // pub async fn get_invoice(&self) -> Result<String, JsValue> {
    //     let invoice = self.inner.get_invoice_inherent().await;
    //     Ok(invoice)
    // }
    //
    // But note that your inherent method is behind cfg(not(target_arch="wasm32"))].
    // If you want to do real async in the browser, you'll need an approach that
    // compiles for WASM (maybe a different async strategy).
}
