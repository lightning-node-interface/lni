// lnd.rs
//
// A single file demonstrating how to export the same Rust code
// to both WASM + JavaScript (using wasm-bindgen) and native code (using Uniffi).

use wasm_bindgen::prelude::*;

// ----------------- WASM-only Imports ------------------
#[cfg(target_arch = "wasm32")]
use gloo_timers::future::TimeoutFuture;
#[cfg(target_arch = "wasm32")]
use js_sys::Function;
#[cfg(target_arch = "wasm32")]
use serde_wasm_bindgen;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_futures::spawn_local;

// ----------------- Native-only Imports ----------------
#[cfg(not(target_arch = "wasm32"))]
use async_std::future::{pending, timeout};
#[cfg(not(target_arch = "wasm32"))]
use std::time::Duration;

// ===========================================
// Transaction (a simple record-like struct)
// ===========================================
#[wasm_bindgen]
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(not(target_arch = "wasm32"), derive(uniffi::Record))]
pub struct Transaction {
    // Make fields private so wasm-bindgen doesn't require them to be `Copy`.
    amount: i64,
    date: String,
    memo: String,
}

#[wasm_bindgen]
impl Transaction {
    // A constructor for Wasm (and optionally Uniffi).
    #[wasm_bindgen(constructor)]
    #[cfg_attr(not(target_arch = "wasm32"), uniffi::constructor)]
    pub fn new(amount: i64, date: String, memo: String) -> Transaction {
        Transaction { amount, date, memo }
    }

    // Getters for each field
    #[wasm_bindgen(getter)]
    pub fn amount(&self) -> i64 {
        self.amount
    }

    #[wasm_bindgen(getter)]
    pub fn date(&self) -> String {
        self.date.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn memo(&self) -> String {
        self.memo.clone()
    }
}

// ===========================================
// InvoiceEvent (a simple record-like struct)
// ===========================================
#[wasm_bindgen]
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(not(target_arch = "wasm32"), derive(uniffi::Record))]
pub struct InvoiceEvent {
    invoice_id: String,
    status: String,
    amount: u64,
    datetime: String,
}

#[wasm_bindgen]
impl InvoiceEvent {
    #[wasm_bindgen(constructor)]
    #[cfg_attr(not(target_arch = "wasm32"), uniffi::constructor)]
    pub fn new(invoice_id: String, status: String, amount: u64, datetime: String) -> InvoiceEvent {
        InvoiceEvent {
            invoice_id,
            status,
            amount,
            datetime,
        }
    }

    #[wasm_bindgen(getter)]
    pub fn invoice_id(&self) -> String {
        self.invoice_id.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn status(&self) -> String {
        self.status.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn amount(&self) -> u64 {
        self.amount
    }

    #[wasm_bindgen(getter)]
    pub fn datetime(&self) -> String {
        self.datetime.clone()
    }
}

// ===========================================
// LndNode (the main "object")
// ===========================================
#[wasm_bindgen]
#[cfg_attr(not(target_arch = "wasm32"), derive(uniffi::Object))]
pub struct LndNode {
    macaroon: String,
    url: String,
    polling_interval: u64,
    polling_timeout: u64,
}

// --------------- WASM Implementation ---------------
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
impl LndNode {
    #[wasm_bindgen(constructor)]
    pub fn new(
        macaroon: String,
        url: String,
        polling_interval: Option<u64>,
        polling_timeout: Option<u64>,
    ) -> Self {
        Self {
            macaroon,
            url,
            polling_interval: polling_interval.unwrap_or(2),
            polling_timeout: polling_timeout.unwrap_or(60),
        }
    }

    // Example getters
    #[wasm_bindgen(getter)]
    pub fn macaroon(&self) -> String {
        self.macaroon.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn url(&self) -> String {
        self.url.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn polling_interval(&self) -> u64 {
        self.polling_interval
    }

    #[wasm_bindgen(getter)]
    pub fn polling_timeout(&self) -> u64 {
        self.polling_timeout
    }

    /// Example function returning multiple Transactions
    #[wasm_bindgen]
    pub fn get_wallet_transactions(&self) -> Vec<Transaction> {
        vec![
            Transaction::new(
                100,
                "2023-01-01".to_string(),
                "Payment from Bob".to_string(),
            ),
            Transaction::new(
                -50,
                "2023-01-02".to_string(),
                "Payment to Alice".to_string(),
            ),
        ]
    }

    /// A simple function returning a string as “payment status”
    #[wasm_bindgen]
    pub fn check_payment_status(&self, _payment_id: String) -> String {
        "PAID".to_string()
    }

    /// on_payment_received - WASM version, accepting a JS function for callbacks
    #[wasm_bindgen]
    pub fn on_payment_received(&self, invoice_id: String, callback: Function) {
        let interval = self.polling_interval;
        let times = self.polling_timeout / self.polling_interval;

        spawn_local(async move {
            for _ in 0..times {
                let event = InvoiceEvent::new(
                    invoice_id.clone(),
                    "paid".to_string(),
                    1000,
                    "wasm-datetime".to_string(),
                );

                // Convert to JsValue with serde
                let event_js = serde_wasm_bindgen::to_value(&event).unwrap();
                let _ = callback.call1(&JsValue::NULL, &event_js);

                // Sleep
                TimeoutFuture::new((interval * 1000) as u32).await;
            }
        });
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[uniffi::export(callback_interface)]
pub trait PaymentListener: Sync + Send {
    fn on_event(&self, event: InvoiceEvent);
}

// #[cfg(not(target_arch = "wasm32"))]
// #[uniffi::export(with_foreign)]
// #[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
// pub trait MyFetcher {
//     async get(url: String) -> String;
// }

// --------------- Native (Uniffi) Implementation ---------------
#[cfg(not(target_arch = "wasm32"))]
#[uniffi::export]
impl LndNode {
    #[uniffi::constructor]
    pub fn new(
        macaroon: String,
        url: String,
        polling_interval: Option<u64>,
        polling_timeout: Option<u64>,
    ) -> Self {
        Self {
            macaroon,
            url,
            polling_interval: polling_interval.unwrap_or(2),
            polling_timeout: polling_timeout.unwrap_or(60),
        }
    }

    // Example getters
    pub fn macaroon(&self) -> String {
        self.macaroon.clone()
    }

    pub fn url(&self) -> String {
        self.url.clone()
    }

    pub fn polling_interval(&self) -> u64 {
        self.polling_interval
    }

    pub fn polling_timeout(&self) -> u64 {
        self.polling_timeout
    }

    /// Return multiple Transactions as a vector
    pub fn get_wallet_transactions(&self) -> Vec<Transaction> {
        vec![
            Transaction::new(100, "2023-01-01".into(), "Payment from Bob".into()),
            Transaction::new(-50, "2023-01-02".into(), "Payment to Alice".into()),
        ]
    }

    /// Return a string as “payment status”
    pub fn check_payment_status(&self, _payment_id: String) -> String {
        "PAID".to_string()
    }

    pub async fn get_invoice(&self) -> String {
        let never = pending::<()>();
        timeout(Duration::from_secs(3), never).await.unwrap_err();
        return "lnop12324rrefdsc".to_string();
    }

    pub async fn on_payment_received(
        &self,
        invoice_id: String,
        callback: Box<dyn PaymentListener>,
    ) {
        let times = self.polling_timeout; // e.g. 5
        let interval = self.polling_interval; // e.g. 1

        for count in 0..times {
            let event = InvoiceEvent::new(
                invoice_id.clone(),
                "paid".to_string(),
                1000,
                format!("native-datetime #{}", count),
            );
            callback.on_event(event);

            tokio::time::sleep(Duration::from_secs(interval)).await;
        }
    }
}

// ------------------------------------
// Optional: native test
// ------------------------------------
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_payment_status() {
        let node = LndNode::new("mac".into(), "http://127.0.0.1".into(), None, None);
        assert_eq!(node.check_payment_status("test123".into()), "PAID");
    }
}
