use crate::{lightning_node_interface::LightningNodeInterface, types::Transaction};
use crate::lnd_api;
use async_trait::async_trait;
// use napi::bindgen_prelude::*;
// use napi_derive::napi;


/// The main LND node object.
pub struct LndNode {
    pub(crate) macaroon: String,
    pub(crate) url: String,
    pub(crate) polling_interval: u64,
    pub(crate) polling_timeout: u64,
}

impl LndNode {
    /// Public inherent constructor for outside crates
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
}

#[async_trait]
impl LightningNodeInterface for LndNode {
    
    /// Note: fn new Core constructor logic above so it can be public

    /// Return the "macaroon" (like a key).
    fn key(&self) -> String {
        self.macaroon.clone()
    }

    fn url(&self) -> String {
        self.url.clone()
    }

    fn polling_interval(&self) -> u64 {
        self.polling_interval
    }

    fn polling_timeout(&self) -> u64 {
        self.polling_timeout
    }

    async fn get_transactions(&self) -> Result<Vec<Transaction>, Box<dyn std::error::Error>> {
        Ok(lnd_api::get_wallet_transactions().await)
    }

    /// Example async function that "gets an invoice" after some delay
    async fn get_invoice(&self, payment_id: String) -> Result<String, Box<dyn std::error::Error>> {
        Ok(lnd_api::get_invoice(payment_id).await)
    }

    /// Another async function for creating an invoice
    async fn create_invoice(
        &self,
        amount: u64,
        memo: String,
    ) -> Result<String, Box<dyn std::error::Error>> {
        Ok(lnd_api::create_invoice(amount, memo).await)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::test;

    #[test]
    async fn test_get_invoice() {
        let node = LndNode::new("mac".into(), "http://127.0.0.1".into(), None, None);
        let result = node.get_invoice("lnp".to_string()).await;
        assert!(result.is_ok());
        assert!(result.unwrap().contains("ln"));
    }
}
