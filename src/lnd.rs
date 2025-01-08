use crate::interface::{
    FetchChannelInfoResponseType, FetchWalletBalanceResponseType, InvoiceEvent, PaymentStatus,
    Transaction, WalletInterface,
};
use wasm_bindgen::prelude::*;

// NOT WASM DEPS
#[cfg(not(target_arch = "wasm32"))]
use tokio::time::sleep;
#[cfg(not(target_arch = "wasm32"))]
use std::time::Duration;

// WASM ONLY DEPS
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_futures::spawn_local;
#[cfg(target_arch = "wasm32")]
use gloo_timers::future::sleep;
#[cfg(target_arch = "wasm32")]
use gloo_timers::future::TimeoutFuture;
#[cfg(target_arch = "wasm32")]
use js_sys::Function;
#[cfg(target_arch = "wasm32")]
use js_sys::Date;

#[wasm_bindgen]
pub struct LndNode {
    macaroon: String,
    url: String,
    wallet_interface: WalletInterface,
    polling_interval: u64,
    polling_timeout: u64,
}

#[wasm_bindgen]
impl LndNode {
    #[wasm_bindgen(constructor)]
    pub fn new(macaroon: String, url: String, polling_interval: Option<u64>, polling_timeout: Option<u64>) -> LndNode {
        LndNode {
            macaroon,
            url,
            wallet_interface: WalletInterface::LND_REST,
            polling_interval: polling_interval.unwrap_or(2),
            polling_timeout: polling_timeout.unwrap_or(60),
        }
    }

    #[wasm_bindgen(getter)]
    pub fn key(&self) -> String {
        self.macaroon.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn url(&self) -> String {
        self.url.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn wallet_interface(&self) -> WalletInterface {
        self.wallet_interface.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn polling_interval(&self) -> u64 {
        self.polling_interval
    }

    #[wasm_bindgen(getter)]
    pub fn polling_timeout(&self) -> u64 {
        self.polling_timeout
    }

    pub fn fetch_wallet_balance(&self) -> FetchWalletBalanceResponseType {
        FetchWalletBalanceResponseType::new(1000)
    }

    pub fn fetch_channel_info(&self, channel_id: String) -> FetchChannelInfoResponseType {
        FetchChannelInfoResponseType::new(100, 50)
    }

    pub fn check_payment_status(&self, payment_id: String) -> PaymentStatus {
        PaymentStatus::new("PAID".to_string())
    }

    pub fn get_wallet_transactions(&self, wallet_id: String) -> Vec<Transaction> {
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

    pub fn pay_invoice(&self, invoice: String) -> String {
        format!("Paid invoice: {}", invoice)
    }

    pub fn get_bolt12_offer(&self) -> String {
        "lno".to_string()
    }

    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen]
    pub fn on_payment_received(&self, invoice_id: String, callback: js_sys::Function) {
        let invoice_id_clone = invoice_id.clone();
        let polling_interval = self.polling_interval;
        let polling_timeout = self.polling_timeout;
    
        spawn_local(async move {
            for _ in 0..(polling_timeout / polling_interval) {
                let datetime = "a".to_string(); // Placeholder for datetime logic
                let event = InvoiceEvent::new(
                    invoice_id_clone.clone(),
                    "paid".to_string(),
                    1000,
                    datetime,
                );
    
                // Convert event to a JsValue using serde_wasm_bindgen
                let event_js = serde_wasm_bindgen::to_value(&event).unwrap();
    
                // Call the JavaScript callback with the event
                callback.call1(&JsValue::NULL, &event_js).unwrap();
    
                // Wait for the polling interval
                TimeoutFuture::new((polling_interval * 1000) as u32).await;
            }
        });
    }
}

impl LndNode {
    #[cfg(not(target_arch = "wasm32"))]
    pub async fn on_payment_received<F>(&self, invoice_id: String, callback: F)
    where
        F: Fn(InvoiceEvent) + Send + 'static,
    {
        let url = self.url.clone();
        let macaroon = self.macaroon.clone();
        let invoice_id_clone = invoice_id.clone();
        let max = self.polling_timeout.clone();
        let i = self.polling_interval.clone();

        tokio::spawn(async move {
            for _ in 0..max{
                let datetime = "a".to_string(); // Date::new_0().to_iso_string();
                let event = InvoiceEvent::new(
                    invoice_id.to_string(),
                    "paid".to_string(),
                    1000,
                    datetime,
                );
                callback(event);
                sleep(Duration::from_secs(i)).await;
            }
        })
        .await
        .unwrap();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lnd_payment() {
        let lnd_node = LndNode::new(
            "test_macaroon".to_string(),
            "https://127.0.0.1:8081".to_string(),
            None,
            None,   
        );
        let result = lnd_node.pay_invoice("invoice".to_string());
        assert!(!result.is_empty());
    }
}