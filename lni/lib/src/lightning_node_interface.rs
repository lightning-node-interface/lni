use crate::types::Transaction;
use async_trait::async_trait;


#[async_trait]
pub trait LightningNodeInterface {

    // Implement the new inherent downstream in your implementation
    // pub fn new(
    //     key: String,
    //     url: String,
    //     polling_interval: Option<u64>,
    //     polling_timeout: Option<u64>,
    // ) -> Self {
    //     Self {
    //         key,
    //         url,
    //         polling_interval: polling_interval.unwrap_or(2),
    //         polling_timeout: polling_timeout.unwrap_or(60),
    //     }
    // }

    fn key(&self) -> String;
    fn url(&self) -> String;
    fn polling_interval(&self) -> u64;
    fn polling_timeout(&self) -> u64;
    async fn create_invoice(
        &self,
        amount: u64,
        memo: String,
    ) -> Result<String, Box<dyn std::error::Error>>;
    async fn get_transactions(&self) -> Result<Vec<Transaction>, Box<dyn std::error::Error>>;
    async fn get_invoice(&self, payment_id: String) -> Result<String, Box<dyn std::error::Error>>;
}
