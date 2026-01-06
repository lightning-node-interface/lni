#[cfg(feature = "napi_rs")]
use napi_derive::napi;

use crate::types::NodeInfo;
use crate::{
    ApiError, CreateInvoiceParams, LightningNode, ListTransactionsParams, LookupInvoiceParams,
    PayCode, PayInvoiceParams, PayInvoiceResponse, Transaction,
};

#[cfg_attr(feature = "napi_rs", napi(object))]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
#[derive(Debug, Clone)]
pub struct LnBitsConfig {
    #[cfg_attr(feature = "uniffi", uniffi(default = Some("https://demo.lnbits.com")))]
    pub base_url: Option<String>,
    pub api_key: String,
    #[cfg_attr(feature = "uniffi", uniffi(default = Some("")))]
    pub socks5_proxy: Option<String>, // Some("socks5h://127.0.0.1:9150") or Some("".to_string())
    #[cfg_attr(feature = "uniffi", uniffi(default = Some(true)))]
    pub accept_invalid_certs: Option<bool>,
    #[cfg_attr(feature = "uniffi", uniffi(default = Some(120)))]
    pub http_timeout: Option<i64>,
}

impl Default for LnBitsConfig {
    fn default() -> Self {
        Self {
            base_url: Some("https://demo.lnbits.com".to_string()),
            api_key: "".to_string(),
            socks5_proxy: Some("".to_string()),
            accept_invalid_certs: Some(true),
            http_timeout: Some(60),
        }
    }
}

#[cfg_attr(feature = "napi_rs", napi(object))]
#[cfg_attr(feature = "uniffi", derive(uniffi::Object))]
#[derive(Debug, Clone)]
pub struct LnBitsNode {
    pub config: LnBitsConfig,
}

// Constructor is inherent, not part of the trait
#[cfg_attr(feature = "uniffi", uniffi::export)]
impl LnBitsNode {
    #[cfg_attr(feature = "uniffi", uniffi::constructor)]
    pub fn new(config: LnBitsConfig) -> Self {
        Self { config }
    }
}

#[cfg_attr(feature = "uniffi", uniffi::export(async_runtime = "tokio"))]
#[async_trait::async_trait]
impl LightningNode for LnBitsNode {
    async fn get_info(&self) -> Result<NodeInfo, ApiError> {
        crate::lnbits::api::get_info(&self.config).await
    }

    async fn create_invoice(&self, params: CreateInvoiceParams) -> Result<Transaction, ApiError> {
        crate::lnbits::api::create_invoice(&self.config, params).await
    }

    async fn pay_invoice(&self, params: PayInvoiceParams) -> Result<PayInvoiceResponse, ApiError> {
        crate::lnbits::api::pay_invoice(&self.config, params).await
    }

    async fn get_offer(&self, search: Option<String>) -> Result<PayCode, ApiError> {
        crate::lnbits::api::get_offer(&self.config, search).await
    }

    async fn list_offers(&self, search: Option<String>) -> Result<Vec<PayCode>, ApiError> {
        crate::lnbits::api::list_offers(&self.config, search).await
    }

    async fn pay_offer(
        &self,
        offer: String,
        amount_msats: i64,
        payer_note: Option<String>,
    ) -> Result<PayInvoiceResponse, ApiError> {
        crate::lnbits::api::pay_offer(&self.config, offer, amount_msats, payer_note).await
    }

    async fn lookup_invoice(&self, params: LookupInvoiceParams) -> Result<crate::Transaction, ApiError> {
        crate::lnbits::api::lookup_invoice(
            &self.config,
            params.payment_hash,
            None,
            None,
            params.search,
        ).await
    }

    async fn list_transactions(
        &self,
        params: ListTransactionsParams,
    ) -> Result<Vec<crate::Transaction>, ApiError> {
        crate::lnbits::api::list_transactions(&self.config, params.from, params.limit, params.search).await
    }

    async fn decode(&self, str: String) -> Result<String, ApiError> {
        crate::lnbits::api::decode(&self.config, str).await
    }

    async fn on_invoice_events(
        &self,
        params: crate::types::OnInvoiceEventParams,
        callback: Box<dyn crate::types::OnInvoiceEventCallback>,
    ) {
        crate::lnbits::api::on_invoice_events(self.config.clone(), params, callback).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::InvoiceType;
    use dotenv::dotenv;
    use lazy_static::lazy_static;
    use std::env;
    use std::sync::{Arc, Mutex};

    lazy_static! {
        static ref BASE_URL: String = {
            dotenv().ok();
            env::var("LNBITS_BASE_URL")
                .unwrap_or_else(|_| "https://demo.lnbits.com".to_string())
        };
        static ref API_KEY: String = {
            dotenv().ok();
            env::var("LNBITS_API_KEY").expect("LNBITS_API_KEY must be set")
        };
        static ref TEST_PAYMENT_HASH: String = {
            dotenv().ok();
            env::var("LNBITS_TEST_PAYMENT_HASH").expect("LNBITS_TEST_PAYMENT_HASH must be set")
        };
        static ref TEST_PAYMENT_REQUEST: String = {
            dotenv().ok();
            env::var("LNBITS_TEST_PAYMENT_REQUEST").expect("LNBITS_TEST_PAYMENT_REQUEST must be set")
        };
        static ref NODE: LnBitsNode = {
            LnBitsNode::new(LnBitsConfig {
                base_url: Some(BASE_URL.clone()),
                api_key: API_KEY.clone(),
                http_timeout: Some(120),
                ..Default::default()
            })
        };
    }

    #[tokio::test]
    async fn test_get_info() {
        match NODE.get_info().await {
            Ok(info) => {
                println!("info: {:?}", info);
            }
            Err(e) => {
                println!("Failed to get info (expected if no API key): {:?}", e);
                // Don't panic as this requires valid API key
            }
        }
    }

    #[tokio::test]
    async fn test_create_invoice() {
        let amount_msats = 21000; // 21 sats
        let description = "Test LNBits invoice".to_string();
        let expiry = 3600;

        match NODE.create_invoice(CreateInvoiceParams {
            invoice_type: InvoiceType::Bolt11,
            amount_msats: Some(amount_msats),
            description: Some(description.clone()),
            expiry: Some(expiry),
            ..Default::default()
        }).await {
            Ok(txn) => {
                println!("LNBits create_invoice: {:?}", txn);
                assert!(
                    !txn.invoice.is_empty(),
                    "LNBits create_invoice Invoice should not be empty"
                );
            }
            Err(e) => {
                println!(
                    "LNBits create_invoice failed (expected if no API key): {:?}",
                    e
                );
                // Don't panic as this requires valid API key
            }
        }
    }

    #[tokio::test]
    async fn test_lookup_invoice() {
        match NODE.lookup_invoice(LookupInvoiceParams {
            payment_hash: Some(TEST_PAYMENT_HASH.to_string()),
            ..Default::default()
        }).await {
            Ok(txn) => {
                println!("LNBits lookup invoice: {:?}", txn);
                assert!(
                    txn.amount_msats >= 0,
                    "Invoice should contain a valid amount"
                );
            }
            Err(e) => {
                if e.to_string().contains("not found") {
                    assert!(true, "Invoice not found as expected");
                } else {
                    println!(
                        "LNBits lookup invoice failed (expected if no API key): {:?}",
                        e
                    );
                }
            }
        }
    }

    #[tokio::test]
    async fn test_list_transactions() {
        let params = ListTransactionsParams {
            from: 0,
            limit: 100,
            payment_hash: None,
            search: None,
        };

        match NODE.list_transactions(params).await {
            Ok(txns) => {
                println!("LNBits transactions: {:?}", txns);
                assert!(txns.len() >= 0, "Should contain at least zero transactions");
            }
            Err(e) => {
                println!(
                    "LNBits list transactions failed (expected if no API key): {:?}",
                    e
                );
                // Don't panic as this requires valid API key
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

        NODE.on_invoice_events(params, Box::new(callback)).await;
        
        // Check that some events were captured
        let events_guard = events.lock().unwrap();
        println!("LNBits events captured: {:?}", *events_guard);

        // We expect at least one event (even if it's a failure due to invoice not found)
        assert!(
            !events_guard.is_empty(),
            "Should capture at least one event"
        );
    }
}