use crate::types::Transaction;

#[async_trait::async_trait]
pub trait LightningNodeInterface {
    fn new(
        key: String,
        url: String,
        polling_interval: Option<u64>,
        polling_timeout: Option<u64>,
    ) -> Self;

    async fn create_invoice(&self, amount: u64, memo: String) -> String;

    fn key(&self) -> String;
    fn url(&self) -> String;
    fn polling_interval(&self) -> u64;
    fn polling_timeout(&self) -> u64;
    fn get_wallet_transactions(&self) -> Vec<Transaction>;
    fn check_payment_status(&self, payment_id: String) -> String;

    async fn get_invoice(&self) -> String;
}
