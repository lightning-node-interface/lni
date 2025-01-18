use crate::{lightning_node_interface::LightningNodeInterface, types::Transaction};
use crate::lnd::lnd_api;
use crate::types::NodeConfig;
use async_trait::async_trait;

/// The main LND node object exposed via UniFFI.
#[derive(uniffi::Object)]
pub struct LndNode {
    pub(crate) macaroon: String,
    pub(crate) url: String,
    pub(crate) polling_interval: u64,
    pub(crate) polling_timeout: u64,
}

/// Inherent methods for constructing and configuring LndNode.
// #[uniffi::export]
impl LndNode {
    /// Public inherent constructor for outside crates.
    // #[uniffi::constructor]
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

    // Public synchronous method (you can export any non-async function like this as well).
    // #[uniffi::export]
    // pub fn key(&self) -> String {
    //     self.macaroon.clone()
    // }
}

/// Trait implementation with async methods, using async_trait for convenience.
#[async_trait]
impl LightningNodeInterface for LndNode {
    // We already have the constructor in inherent form above.

    fn key(&self) -> String {
        let n =  NodeConfig {
            key: self.macaroon.clone(),
            endpoint: self.url.clone(),
            polling_interval: self.polling_interval,
            polling_timeout: self.polling_timeout,
        };  
        eprint!("Key: {}", n.key);
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

    async fn get_invoice(&self, payment_id: String) -> Result<String, Box<dyn std::error::Error>> {
        Ok(lnd_api::get_invoice(payment_id).await)
    }

    async fn create_invoice(
        &self,
        amount: u64,
        memo: String,
    ) -> Result<String, Box<dyn std::error::Error>> {
        Ok(lnd_api::create_invoice(amount, memo).await)
    }
}

/// UniFFI-exported async wrapper methods that internally call your trait methods.
/// The default runtime is "blocking". You may switch with `async_runtime = "spawn"` or `"none"`.
impl LndNode {
    pub async fn get_invoice_uniffi(&self, payment_id: String) -> String {
        match self.get_invoice(payment_id).await {
            Ok(invoice) => invoice,
            Err(e) => format!("Error: {}", e),
        }
    }

    pub async fn create_invoice_uniffi(&self, amount: u64, memo: String) -> String {
        match self.create_invoice(amount, memo).await {
            Ok(invoice) => invoice,
            Err(e) => format!("Error: {}", e),
        }
    }

    pub async fn get_transactions_uniffi(&self) -> Vec<Transaction> {
        match self.get_transactions().await {
            Ok(txs) => txs,
            Err(_) => vec![],
        }
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