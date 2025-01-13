#[cfg(feature = "uniffi")] // Only compile if feature=uniffi
use uniffi_macros::export;

use async_trait::async_trait;
use crate::types::Transaction;
use crate::lightning_node_interface::LightningNodeInterface;
use super::lnd::LndNode; // The core struct

#[cfg(feature = "uniffi")]
#[async_trait::async_trait]
impl LightningNodeInterface for LndNode {
    #[uniffi::constructor]
    fn new(
        macaroon: String,
        url: String,
        polling_interval: Option<u64>,
        polling_timeout: Option<u64>,
    ) -> Self {
        LndNode::new_inherent(macaroon, url, polling_interval, polling_timeout)
    }

    async fn create_invoice(&self, amount: u64, memo: String) -> String {
        // Note: if your target language supports async,
        // UniFFI will let you call this asynchronously.
        // We forward to the inherent method:
        self.create_invoice_inherent(amount, &memo).await
    }

    fn key(&self) -> String {
        self.key_inherent()
    }

    fn url(&self) -> String {
        self.url_inherent()
    }

    fn polling_interval(&self) -> u64 {
        self.polling_interval_inherent()
    }

    fn polling_timeout(&self) -> u64 {
        self.polling_timeout_inherent()
    }

    fn get_wallet_transactions(&self) -> Vec<Transaction> {
        self.get_wallet_transactions_inherent()
    }

    fn check_payment_status(&self, payment_id: String) -> String {
        self.check_payment_status_inherent(&payment_id)
    }

    async fn get_invoice(&self) -> String {
        self.get_invoice_inherent().await
    }
}
