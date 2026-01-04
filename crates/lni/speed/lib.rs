#[cfg(feature = "napi_rs")]
use napi_derive::napi;

use crate::types::{ListTransactionsParams, LookupInvoiceParams, NodeInfo};
use crate::{
    ApiError, CreateInvoiceParams, CreateOfferParams, Offer, PayInvoiceParams, PayInvoiceResponse, Transaction,
};
#[cfg(not(feature = "uniffi"))]
use crate::{LightningNode, OnInvoiceEventCallback, OnInvoiceEventParams};

#[cfg_attr(feature = "napi_rs", napi(object))]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
#[derive(Debug, Clone)]
pub struct SpeedConfig {
    #[cfg_attr(feature = "uniffi", uniffi(default = Some("https://api.tryspeed.com")))]
    pub base_url: Option<String>,
    pub api_key: String,
    #[cfg_attr(feature = "uniffi", uniffi(default = Some("")))]
    pub socks5_proxy: Option<String>, // Some("socks5h://127.0.0.1:9150") or Some("".to_string())
    #[cfg_attr(feature = "uniffi", uniffi(default = Some(true)))]
    pub accept_invalid_certs: Option<bool>,
    #[cfg_attr(feature = "uniffi", uniffi(default = Some(120)))]
    pub http_timeout: Option<i64>,
}

impl Default for SpeedConfig {
    fn default() -> Self {
        Self {
            base_url: Some("https://api.tryspeed.com".to_string()),
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
pub struct SpeedNode {
    pub config: SpeedConfig,
}

// Constructor is inherent, not part of the trait
#[cfg_attr(feature = "uniffi", uniffi::export)]
impl SpeedNode {
    #[cfg_attr(feature = "uniffi", uniffi::constructor)]
    pub fn new(config: SpeedConfig) -> Self {
        Self { config }
    }
}

// All node methods - UniFFI exports these directly when the feature is enabled
#[cfg_attr(feature = "uniffi", uniffi::export(async_runtime = "tokio"))]
impl SpeedNode {
    pub async fn get_info(&self) -> Result<NodeInfo, ApiError> {
        crate::speed::api::get_info(&self.config).await
    }

    pub async fn create_invoice(
        &self,
        params: CreateInvoiceParams,
    ) -> Result<Transaction, ApiError> {
        crate::speed::api::create_invoice(&self.config, params).await
    }

    pub async fn pay_invoice(
        &self,
        params: PayInvoiceParams,
    ) -> Result<PayInvoiceResponse, ApiError> {
        crate::speed::api::pay_invoice(&self.config, params).await
    }

    pub async fn create_offer(&self, _params: CreateOfferParams) -> Result<Offer, ApiError> {
        Err(ApiError::Api { reason: "create_offer not implemented for SpeedNode".to_string() })
    }

    pub async fn lookup_invoice(&self, params: LookupInvoiceParams) -> Result<Transaction, ApiError> {
        crate::speed::api::lookup_invoice(
            &self.config,
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
    ) -> Result<Vec<Transaction>, ApiError> {
        crate::speed::api::list_transactions(&self.config, params.from, params.limit, params.search)
            .await
    }

    pub async fn decode(&self, str: String) -> Result<String, ApiError> {
        crate::speed::api::decode(&self.config, str).await
    }

    pub async fn get_offer(&self, search: Option<String>) -> Result<Offer, ApiError> {
        crate::speed::api::get_offer(&self.config, search).await
    }

    pub async fn list_offers(&self, search: Option<String>) -> Result<Vec<Offer>, ApiError> {
        crate::speed::api::list_offers(&self.config, search).await
    }

    pub async fn pay_offer(
        &self,
        offer: String,
        amount_msats: i64,
        payer_note: Option<String>,
    ) -> Result<PayInvoiceResponse, ApiError> {
        crate::speed::api::pay_offer(&self.config, offer, amount_msats, payer_note).await
    }
}

// Methods not supported by UniFFI (callbacks)
#[cfg(not(feature = "uniffi"))]
impl SpeedNode {
    pub async fn on_invoice_events(
        &self,
        params: OnInvoiceEventParams,
        callback: Box<dyn OnInvoiceEventCallback>,
    ) {
        crate::speed::api::on_invoice_events(self.config.clone(), params, callback).await;
    }
}

// Trait implementation for Rust consumers - uses the impl_lightning_node macro
#[cfg(not(feature = "uniffi"))]
crate::impl_lightning_node!(SpeedNode);

#[cfg(test)]
mod tests {
    use super::*;
    use dotenv::dotenv;
    use lazy_static::lazy_static;
    use lightning_invoice::Bolt11Invoice;
    use std::env;
    use std::str::FromStr;
    use std::sync::{Arc, Mutex};

    lazy_static! {
        static ref BASE_URL: String = {
            dotenv().ok();
            env::var("SPEED_BASE_URL").unwrap_or_else(|_| "https://api.tryspeed.com".to_string())
        };
        static ref API_KEY: String = {
            dotenv().ok();
            env::var("SPEED_API_KEY").expect("SPEED_API_KEY must be set")
        };
        static ref TEST_PAYMENT_REQUEST: String = {
            dotenv().ok();
            env::var("SPEED_TEST_PAYMENT_REQUEST").expect("SPEED_TEST_PAYMENT_REQUEST must be set")
        };
        static ref TEST_PAYMENT_HASH: String = {
            dotenv().ok();
            env::var("SPEED_TEST_PAYMENT_HASH").expect("SPEED_TEST_PAYMENT_HASH must be set")
        };
        static ref NODE: SpeedNode = {
            SpeedNode::new(SpeedConfig {
                base_url: Some(BASE_URL.clone()),
                api_key: API_KEY.clone(),
                ..Default::default()
            })
        };
    }

    #[tokio::test]
    async fn test_get_info() {
        match NODE.get_info().await {
            Ok(node_info) => {
                dbg!(&node_info);
                assert_eq!(node_info.alias, "Speed Node");
            }
            Err(e) => {
                // Expected to fail without valid API key
                dbg!(e);
            }
        }
    }

    #[tokio::test]
    async fn test_create_invoice() {
        let params = CreateInvoiceParams {
            invoice_type: crate::InvoiceType::Bolt11,
            amount_msats: Some(1000), // 1 sat
            description: Some("Test invoice".to_string()),
            description_hash: None,
            expiry: None,
            offer: None,
            r_preimage: None,
            is_blinded: Some(false),
            is_keysend: Some(false),
            is_amp: Some(false),
            is_private: Some(false),
        };

        match NODE.create_invoice(params).await {
            Ok(transaction) => {
                dbg!(&transaction);
                assert_eq!(transaction.type_, "incoming");
                assert_eq!(transaction.amount_msats, 1000);
            }
            Err(e) => {
                // Expected to fail without valid API key
                dbg!(e);
            }
        }
    }

    // #[test]
    // fn test_pay_invoice() {
    //     let params = PayInvoiceParams {
    //         invoice: TEST_PAYMENT_REQUEST.clone(),
    //         amount_msats: None, // Use amount from invoice
    //         fee_limit_msat: None,
    //         fee_limit_percentage: Some(0.5), // 0.5% fee limit
    //         timeout_seconds: Some(30),
    //         max_parts: None,
    //         first_hop_pubkey: None,
    //         last_hop_pubkey: None,
    //         allow_self_payment: None,
    //         is_amp: None,
    //     };

    //     match NODE.pay_invoice(params) {
    //         Ok(response) => {
    //             dbg!(&response);
    //             assert!(!response.payment_hash.is_empty(), "Payment hash should not be empty");
    //         }
    //         Err(e) => {
    //             // Expected to fail - invalid/test invoice or insufficient balance
    //             dbg!(&e);
    //             // Common errors: "Invalid invoice", "Insufficient balance", etc.
    //             assert!(true, "Expected to fail with test invoice");
    //         }
    //     }
    // }

    #[tokio::test]
    async fn test_list_transactions() {
        let params = ListTransactionsParams {
            from: 0,
            limit: 100,
            payment_hash: None,
            search: None,
        };

        match NODE.list_transactions(params).await {
            Ok(transactions) => {
                dbg!(&transactions);
            }
            Err(e) => {
                // Expected to fail without valid API key
                dbg!(e);
            }
        }
    }

    #[tokio::test]
    async fn test_payment_hash_computation() {
        // First, let's verify that our stored SPEED_TEST_PAYMENT_HASH matches the one computed from the withdraw_request
        let withdraw_request = &*TEST_PAYMENT_REQUEST;
        let stored_payment_hash = &*TEST_PAYMENT_HASH;

        // Compute payment hash from the BOLT11 invoice
        match Bolt11Invoice::from_str(withdraw_request) {
            Ok(bolt11) => {
                let computed_payment_hash = format!("{:x}", bolt11.payment_hash());
                println!("Stored payment hash: {}", stored_payment_hash);
                println!("Computed payment hash: {}", computed_payment_hash);
                assert_eq!(
                    stored_payment_hash,
                    &computed_payment_hash,
                    "Payment hash from environment should match computed hash from withdraw_request"
                );
            }
            Err(e) => {
                panic!("Failed to parse BOLT11 invoice: {}", e);
            }
        }
    }

    #[cfg(not(feature = "uniffi"))]
    #[tokio::test]
    async fn test_on_invoice_events() {
        struct OnInvoiceEventCallback {
            events: Arc<Mutex<Vec<String>>>,
        }

        impl crate::OnInvoiceEventCallback for OnInvoiceEventCallback {
            fn success(&self, transaction: Option<Transaction>) {
                dbg!("Success speed paid");
                dbg!(&transaction);
                let mut events = self.events.lock().unwrap();
                events.push(format!("{} - {:?}", "success", transaction));
            }
            fn pending(&self, transaction: Option<Transaction>) {
                dbg!("pending speed payment");
                let mut events = self.events.lock().unwrap();
                events.push(format!("{} - {:?}", "pending", transaction));
            }
            fn failure(&self, transaction: Option<Transaction>) {
                dbg!("failure speed payment");
                let mut events = self.events.lock().unwrap();
                events.push(format!("{} - {:?}", "failure", transaction));
            }
        }

        let events = Arc::new(Mutex::new(Vec::new()));
        let callback = OnInvoiceEventCallback {
            events: events.clone(),
        };

        // Use the payment hash from the environment variable
        let params = OnInvoiceEventParams {
            payment_hash: Some(TEST_PAYMENT_HASH.to_string()),
            polling_delay_sec: 2,
            max_polling_sec: 6,
            search: Some(TEST_PAYMENT_REQUEST.to_string()), // Also provide the withdraw_request as search term
        };

        NODE.on_invoice_events(params, Box::new(callback)).await;

        // Check that some events were captured
        let events_guard = events.lock().unwrap();
        println!("Speed events captured: {:?}", *events_guard);

        // We expect at least one event (even if it's a failure due to invoice not found)
        assert!(
            !events_guard.is_empty(),
            "Should capture at least one event"
        );
    }
}
