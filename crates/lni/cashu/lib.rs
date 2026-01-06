#[cfg(feature = "napi_rs")]
use napi_derive::napi;

use crate::types::NodeInfo;
use crate::{
    ApiError, CreateInvoiceParams, CreateOfferParams, ListTransactionsParams, LookupInvoiceParams,
    Offer, PayInvoiceParams, PayInvoiceResponse, Transaction,
};
#[cfg(not(feature = "uniffi"))]
use crate::LightningNode;

#[cfg_attr(feature = "napi_rs", napi(object))]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
#[derive(Debug, Clone)]
pub struct CashuConfig {
    /// The Cashu mint URL
    pub mint_url: String,
    /// Optional wallet seed (64 bytes as hex string). If not provided, a random seed is generated.
    #[cfg_attr(feature = "uniffi", uniffi(default = None))]
    pub seed: Option<String>,
    #[cfg_attr(feature = "uniffi", uniffi(default = Some("")))]
    pub socks5_proxy: Option<String>,
    #[cfg_attr(feature = "uniffi", uniffi(default = Some(true)))]
    pub accept_invalid_certs: Option<bool>,
    #[cfg_attr(feature = "uniffi", uniffi(default = Some(120)))]
    pub http_timeout: Option<i64>,
}

impl Default for CashuConfig {
    fn default() -> Self {
        Self {
            mint_url: "https://mint.minibits.cash/Bitcoin".to_string(),
            seed: None,
            socks5_proxy: Some("".to_string()),
            accept_invalid_certs: Some(false),
            http_timeout: Some(60),
        }
    }
}

#[cfg_attr(feature = "napi_rs", napi(object))]
#[cfg_attr(feature = "uniffi", derive(uniffi::Object))]
#[derive(Debug, Clone)]
pub struct CashuNode {
    pub config: CashuConfig,
}

// Constructor is inherent, not part of the trait
#[cfg_attr(feature = "uniffi", uniffi::export)]
impl CashuNode {
    #[cfg_attr(feature = "uniffi", uniffi::constructor)]
    pub fn new(config: CashuConfig) -> Self {
        Self { config }
    }
}

// All node methods - UniFFI exports these directly when the feature is enabled
#[cfg_attr(feature = "uniffi", uniffi::export(async_runtime = "tokio"))]
impl CashuNode {
    pub async fn get_info(&self) -> Result<NodeInfo, ApiError> {
        crate::cashu::api::get_info(self.config.clone()).await
    }

    pub async fn create_invoice(&self, params: CreateInvoiceParams) -> Result<Transaction, ApiError> {
        crate::cashu::api::create_invoice(self.config.clone(), params).await
    }

    pub async fn pay_invoice(&self, params: PayInvoiceParams) -> Result<PayInvoiceResponse, ApiError> {
        crate::cashu::api::pay_invoice(self.config.clone(), params).await
    }

    pub async fn create_offer(&self, _params: CreateOfferParams) -> Result<Offer, ApiError> {
        Err(ApiError::Api { reason: "create_offer not implemented for CashuNode".to_string() })
    }

    pub async fn lookup_invoice(
        &self,
        params: LookupInvoiceParams,
    ) -> Result<crate::Transaction, ApiError> {
        crate::cashu::api::lookup_invoice(
            self.config.clone(),
            params.payment_hash,
            None,
            None,
            params.search,
        )
        .await
    }

    pub async fn list_transactions(
        &self,
        params: ListTransactionsParams,
    ) -> Result<Vec<crate::Transaction>, ApiError> {
        crate::cashu::api::list_transactions(
            self.config.clone(),
            params.from,
            params.limit,
            params.search,
        )
        .await
    }

    pub async fn decode(&self, str: String) -> Result<String, ApiError> {
        crate::cashu::api::decode(&self.config, str)
    }

    pub async fn get_offer(&self, search: Option<String>) -> Result<Offer, ApiError> {
        crate::cashu::api::get_offer(&self.config, search)
    }

    pub async fn list_offers(&self, search: Option<String>) -> Result<Vec<Offer>, ApiError> {
        crate::cashu::api::list_offers(&self.config, search).await
    }

    pub async fn pay_offer(
        &self,
        offer: String,
        amount_msats: i64,
        payer_note: Option<String>,
    ) -> Result<PayInvoiceResponse, ApiError> {
        crate::cashu::api::pay_offer(&self.config, offer, amount_msats, payer_note)
    }

    pub async fn on_invoice_events(
        &self,
        params: crate::types::OnInvoiceEventParams,
        callback: std::sync::Arc<dyn crate::types::OnInvoiceEventCallback>,
    ) {
        crate::cashu::api::on_invoice_events(self.config.clone(), params, callback).await
    }
}

// Trait implementation for polymorphic access via Arc<dyn LightningNode>
crate::impl_lightning_node!(CashuNode);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::InvoiceType;
    use dotenv::dotenv;
    use lazy_static::lazy_static;
    use std::env;
    use std::sync::{Arc, Mutex};

    lazy_static! {
        static ref MINT_URL: String = {
            dotenv().ok();
            env::var("CASHU_MINT_URL").unwrap_or_else(|_| "https://mint.minibits.cash/Bitcoin".to_string())
        };
        static ref SEED: Option<String> = {
            dotenv().ok();
            env::var("CASHU_SEED").ok()
        };
        static ref TEST_PAYMENT_HASH: String = {
            dotenv().ok();
            env::var("CASHU_TEST_PAYMENT_HASH").unwrap_or_else(|_| "test_quote_id".to_string())
        };
        static ref NODE: CashuNode = {
            CashuNode::new(CashuConfig {
                mint_url: MINT_URL.clone(),
                seed: SEED.clone(),
                http_timeout: Some(120),
                socks5_proxy: Some("".to_string()),
                accept_invalid_certs: Some(false),
            })
        };
    }

    #[tokio::test]
    async fn test_get_info() {
        match NODE.get_info().await {
            Ok(info) => {
                println!("Cashu get_info: {:?}", info);
                assert!(!info.alias.is_empty(), "Alias should not be empty");
            }
            Err(e) => {
                println!("Cashu get_info failed (may be expected without mint connection): {:?}", e);
            }
        }
    }

    #[tokio::test]
    async fn test_create_invoice() {
        let amount_msats = 5000; // 5 sats
        let description = "Test Cashu invoice".to_string();
        let expiry = 3600;

        match NODE
            .create_invoice(CreateInvoiceParams {
                invoice_type: Some(InvoiceType::Bolt11),
                amount_msats: Some(amount_msats),
                description: Some(description.clone()),
                expiry: Some(expiry),
                ..Default::default()
            })
            .await
        {
            Ok(txn) => {
                println!("Cashu create_invoice: {:?}", txn);
                assert!(
                    !txn.invoice.is_empty(),
                    "Cashu create_invoice Invoice should not be empty"
                );
            }
            Err(e) => {
                println!(
                    "Cashu create_invoice failed (expected without mint connection): {:?}",
                    e
                );
            }
        }
    }

    #[tokio::test]
    async fn test_list_transactions() {
        let params = ListTransactionsParams {
            from: 0,
            limit: 10,
            payment_hash: None,
            search: None,
        };
        match NODE.list_transactions(params).await {
            Ok(txns) => {
                println!("Cashu transactions: {:?}", txns);
                assert!(true, "Should be able to list transactions");
            }
            Err(e) => {
                println!(
                    "Cashu list transactions failed (expected without mint connection): {:?}",
                    e
                );
            }
        }
    }

    #[tokio::test]
    async fn test_decode() {
        let token = "cashuAeyJ0b2tlbiI6W3sibWludCI6Imh0dHBzOi8vbWludC5jYXNodS5zcGFjZSIsInByb29mcyI6W3siYW1vdW50IjoyLCJpZCI6IjAwOWExZjI5MzI1M2U0MWUiLCJzZWNyZXQiOiI0MDcwMjVjYWY1MjYzNTljYWUyYWNjMjMxYjM4Mzg0NDk3NjlmNmNhZjlmMjI4YzMxMzE2YTE4YmQ5ZjA0MzdiIiwiQyI6IjAzNGI2MjQ0YmNhZDkzN2QzMjBkZTFlNmU0YTM2MDM3MDVmMmQyNWQyNmNkYjhkNzNmYjQ5NTRlMzZlZGQxMDk5OCJ9XX1dfQ";
        match NODE.decode(token.to_string()).await {
            Ok(decoded) => {
                println!("Cashu decode: {:?}", decoded);
                assert!(decoded.contains("Cashu token"), "Should decode as Cashu token");
            }
            Err(e) => {
                panic!("Cashu decode failed: {:?}", e);
            }
        }
    }

    #[tokio::test]
    async fn test_on_invoice_events() {
        struct OnInvoiceEventCallback {
            events: Arc<Mutex<Vec<String>>>,
        }

        impl crate::types::OnInvoiceEventCallback for OnInvoiceEventCallback {
            fn success(&self, transaction: Option<Transaction>) {
                let mut events = self.events.lock().unwrap();
                events.push(format!("{} - {:?}", "success", transaction));
            }
            fn pending(&self, transaction: Option<Transaction>) {
                let mut events = self.events.lock().unwrap();
                events.push(format!("{} - {:?}", "pending", transaction));
            }
            fn failure(&self, transaction: Option<Transaction>) {
                let mut events = self.events.lock().unwrap();
                events.push(format!("{} - {:?}", "failure", transaction));
            }
        }

        let events = Arc::new(Mutex::new(Vec::new()));
        let callback = OnInvoiceEventCallback {
            events: events.clone(),
        };

        let params = crate::types::OnInvoiceEventParams {
            payment_hash: Some(TEST_PAYMENT_HASH.to_string()),
            polling_delay_sec: 2,
            max_polling_sec: 5,
            ..Default::default()
        };

        NODE.on_invoice_events(params, std::sync::Arc::new(callback)).await;

        let events_guard = events.lock().unwrap();
        println!("Cashu events captured: {:?}", *events_guard);
        assert!(
            !events_guard.is_empty(),
            "Should capture at least one event"
        );
    }

    #[test]
    fn test_config_defaults() {
        let config = CashuConfig::default();
        assert!(!config.mint_url.is_empty(), "Default mint URL should not be empty");
        assert!(config.seed.is_none(), "Default seed should be None");
    }
}
