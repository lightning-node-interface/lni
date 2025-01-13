// src/lnd/lnd.rs
use async_std::future::{pending, timeout};

#[cfg(not(target_arch = "wasm32"))]
use std::time::Duration;

use crate::types::Transaction;

/// The main LND node object.
/// No wasm_bindgen or uniffi attributes here—just the real code.
pub struct LndNode {
    pub(crate) macaroon: String,
    pub(crate) url: String,
    pub(crate) polling_interval: u64,
    pub(crate) polling_timeout: u64,
}

impl LndNode {
    /// Core constructor logic.
    pub fn new_inherent(
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

    /// Return the "macaroon" (like a key).
    pub fn key_inherent(&self) -> String {
        self.macaroon.clone()
    }

    pub fn url_inherent(&self) -> String {
        self.url.clone()
    }

    pub fn polling_interval_inherent(&self) -> u64 {
        self.polling_interval
    }

    pub fn polling_timeout_inherent(&self) -> u64 {
        self.polling_timeout
    }

    /// Return multiple Transactions as a vector
    pub fn get_wallet_transactions_inherent(&self) -> Vec<Transaction> {
        vec![
            Transaction::new(100, "2023-01-01".into(), "Payment from Bob".into()),
            Transaction::new(-50, "2023-01-02".into(), "Payment to Alice".into()),
        ]
    }

    /// Return a string as "payment status"
    pub fn check_payment_status_inherent(&self, _payment_id: &str) -> String {
        "PAID".to_string()
    }

    /// Example async function that "gets an invoice" after some delay
    #[cfg(not(target_arch = "wasm32"))]
    pub async fn get_invoice_inherent(&self) -> String {
        let never = pending::<()>();
        // 3-second "fake" timeout for demonstration
        timeout(Duration::from_secs(3), never).await.unwrap_err();
        "lnop12324rrefdsc".to_string()
    }

    /// Another async function for creating an invoice
    #[cfg(not(target_arch = "wasm32"))]
    pub async fn create_invoice_inherent(&self, _amount: u64, _memo: &str) -> String {
        let never = pending::<()>();
        timeout(Duration::from_secs(3), never).await.unwrap_err();
        "lnop12324rrefdsc".to_string()
    }

    // ... any additional core logic ...
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_lnd() {
        let node = LndNode::new_inherent(
            "mac".into(),
            "http://127.0.0.1".into(),
            None,
            None,
        );
        assert_eq!(node.check_payment_status_inherent("test123"), "PAID");
    }
}
