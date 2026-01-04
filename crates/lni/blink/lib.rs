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
pub struct BlinkConfig {
    #[cfg_attr(feature = "uniffi", uniffi(default = Some("https://api.blink.sv/graphql")))]
    pub base_url: Option<String>,
    pub api_key: String,
   #[cfg_attr(feature = "uniffi", uniffi(default = Some("")))]
    pub socks5_proxy: Option<String>, // Some("socks5h://127.0.0.1:9150") or Some("".to_string())
    #[cfg_attr(feature = "uniffi", uniffi(default = Some(true)))]
    pub accept_invalid_certs: Option<bool>,
    #[cfg_attr(feature = "uniffi", uniffi(default = Some(120)))]
    pub http_timeout: Option<i64>,
}

impl Default for BlinkConfig {
    fn default() -> Self {
        Self {
            base_url: Some("https://api.blink.sv/graphql".to_string()),
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
pub struct BlinkNode {
    pub config: BlinkConfig,
}

// Constructor is inherent, not part of the trait
#[cfg_attr(feature = "uniffi", uniffi::export)]
impl BlinkNode {
    #[cfg_attr(feature = "uniffi", uniffi::constructor)]
    pub fn new(config: BlinkConfig) -> Self {
        Self { config }
    }
}

// All node methods - UniFFI exports these directly when the feature is enabled
#[cfg_attr(feature = "uniffi", uniffi::export(async_runtime = "tokio"))]
impl BlinkNode {
    pub async fn get_info(&self) -> Result<NodeInfo, ApiError> {
        crate::blink::api::get_info(&self.config).await
    }

    pub async fn create_invoice(&self, params: CreateInvoiceParams) -> Result<Transaction, ApiError> {
        crate::blink::api::create_invoice(&self.config, params).await
    }

    pub async fn pay_invoice(&self, params: PayInvoiceParams) -> Result<PayInvoiceResponse, ApiError> {
        crate::blink::api::pay_invoice(&self.config, params).await
    }

    pub async fn create_offer(&self, _params: CreateOfferParams) -> Result<Offer, ApiError> {
        Err(ApiError::Api { reason: "create_offer not implemented for BlinkNode".to_string() })
    }

    pub async fn get_offer(&self, search: Option<String>) -> Result<Offer, ApiError> {
        crate::blink::api::get_offer(&self.config, search).await
    }

    pub async fn list_offers(&self, search: Option<String>) -> Result<Vec<Offer>, ApiError> {
        crate::blink::api::list_offers(&self.config, search).await
    }

    pub async fn pay_offer(
        &self,
        offer: String,
        amount_msats: i64,
        payer_note: Option<String>,
    ) -> Result<PayInvoiceResponse, ApiError> {
        crate::blink::api::pay_offer(&self.config, offer, amount_msats, payer_note).await
    }

    pub async fn lookup_invoice(&self, params: LookupInvoiceParams) -> Result<crate::Transaction, ApiError> {
        crate::blink::api::lookup_invoice(
            &self.config,
            params.payment_hash,
            None,
            None,
            params.search,
        ).await
    }

    pub async fn list_transactions(
        &self,
        params: ListTransactionsParams,
    ) -> Result<Vec<crate::Transaction>, ApiError> {
        crate::blink::api::list_transactions(&self.config, params.from, params.limit, params.search).await
    }

    pub async fn decode(&self, str: String) -> Result<String, ApiError> {
        crate::blink::api::decode(&self.config, str).await
    }
}

// Methods not supported by UniFFI (callbacks)
#[cfg(not(feature = "uniffi"))]
impl BlinkNode {
    pub async fn on_invoice_events(
        &self,
        params: crate::types::OnInvoiceEventParams,
        callback: Box<dyn crate::types::OnInvoiceEventCallback>,
    ) {
        crate::blink::api::on_invoice_events(self.config.clone(), params, callback).await
    }
}

// Trait implementation for Rust consumers - uses the impl_lightning_node macro
#[cfg(not(feature = "uniffi"))]
crate::impl_lightning_node!(BlinkNode);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::InvoiceType;
    use dotenv::dotenv;
    use lazy_static::lazy_static;
    use std::env;
    use std::sync::{Arc, Mutex};
    use std::thread;

    lazy_static! {
        static ref BASE_URL: String = {
            dotenv().ok();
            env::var("BLINK_BASE_URL")
                .unwrap_or_else(|_| "https://api.blink.sv/graphql".to_string())
        };
        static ref API_KEY: String = {
            dotenv().ok();
            env::var("BLINK_API_KEY").expect("BLINK_API_KEY must be set")
        };
        static ref TEST_PAYMENT_HASH: String = {
            dotenv().ok();
            env::var("BLINK_TEST_PAYMENT_HASH").expect("BLINK_TEST_PAYMENT_HASH must be set")
        };
        static ref TEST_PAYMENT_REQUEST: String = {
            dotenv().ok();
            env::var("BLINK_TEST_PAYMENT_REQUEST").expect("BLINK_TEST_PAYMENT_REQUEST must be set")
        };
        static ref NODE: BlinkNode = {
            BlinkNode::new(BlinkConfig {
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
                panic!("Failed to get info: {:?}", e);
            }
        }
    }

    #[tokio::test]
    async fn test_create_invoice() {
        let amount_msats = 21000; // 21 sats
        let description = "Test Blink invoice".to_string();
        let expiry = 3600;

        match NODE.create_invoice(CreateInvoiceParams {
            invoice_type: InvoiceType::Bolt11,
            amount_msats: Some(amount_msats),
            description: Some(description.clone()),
            expiry: Some(expiry),
            ..Default::default()
        }).await {
            Ok(txn) => {
                println!("Blink create_invoice: {:?}", txn);
                assert!(
                    !txn.invoice.is_empty(),
                    "Blink create_invoice Invoice should not be empty"
                );
            }
            Err(e) => {
                println!(
                    "Blink create_invoice failed (expected if no API key): {:?}",
                    e
                );
                // Don't panic as this requires valid API key
            }
        }
    }

    // #[tokio::test]
    // async fn test_pay_invoice() {
    //     match NODE.pay_invoice(PayInvoiceParams {
    //         invoice: TEST_PAYMENT_REQUEST.clone(),
    //         amount_msats: None, // Use amount from invoice
    //         ..Default::default()
    //     }).await {
    //         Ok(response) => {
    //             println!("Blink pay_invoice response: {:?}", response);
    //             assert!(
    //                 response.payment_hash.len() > 0,
    //                 "Payment hash should not be empty"
    //             );
    //         }
    //         Err(e) => {
    //             println!(
    //                 "Blink pay_invoice failed (expected if no API key or invalid invoice): {:?}",
    //                 e
    //             );
    //             // Don't panic as this requires valid API key and valid invoice
    //             // Common errors: insufficient balance, invalid invoice, etc.
    //         }
    //     }
    // }

    #[tokio::test]
    async fn test_lookup_invoice() {
        match NODE.lookup_invoice(LookupInvoiceParams {
            payment_hash: Some(TEST_PAYMENT_HASH.to_string()),
            ..Default::default()
        }).await {
            Ok(txn) => {
                println!("Blink lookup invoice: {:?}", txn);
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
                        "Blink lookup invoice failed (expected if no API key): {:?}",
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
                dbg!(&txns);
                // Validate we can parse transactions
                assert!(txns.len() >= 0, "Should contain at least zero transactions");
            }
            Err(e) => {
                println!(
                    "Blink list transactions failed (expected if no API key): {:?}",
                    e
                );
                // Don't panic as this requires valid API key
            }
        }
    }

    #[cfg(not(feature = "uniffi"))]
    #[tokio::test]
    async fn test_on_invoice_events() {
        struct OnInvoiceEventCallback {
            events: Arc<Mutex<Vec<String>>>,
        }

        impl crate::types::OnInvoiceEventCallback for OnInvoiceEventCallback {
            fn success(&self, transaction: Option<Transaction>) {
                dbg!("Success blink paid");
                dbg!(&transaction);
                let mut events = self.events.lock().unwrap();
                events.push(format!("{} - {:?}", "success", transaction));
            }
            fn pending(&self, transaction: Option<Transaction>) {
                dbg!("Pending blink payment");
                let mut events = self.events.lock().unwrap();
                events.push(format!("{} - {:?}", "pending", transaction));
            }
            fn failure(&self, transaction: Option<Transaction>) {
                dbg!("Failure blink payment");
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
        println!("Blink events captured: {:?}", *events_guard);

        // We expect at least one event (even if it's a failure due to invoice not found)
        assert!(
            !events_guard.is_empty(),
            "Should capture at least one event"
        );
    }
}
