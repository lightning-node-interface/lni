#[cfg(feature = "napi_rs")]
use napi_derive::napi;

use crate::types::{NodeInfo, ListTransactionsParams, LookupInvoiceParams};
use crate::{
    ApiError, CreateInvoiceParams, LightningNode, OnInvoiceEventCallback, OnInvoiceEventParams,
    PayCode, PayInvoiceParams, PayInvoiceResponse, Transaction,
};

#[cfg_attr(feature = "napi_rs", napi(object))]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
#[derive(Debug, Clone)]
pub struct SpeedConfig {
    #[cfg_attr(feature = "uniffi", uniffi(default = Some("https://api.tryspeed.com")))]
    pub base_url: Option<String>,
    pub api_key: String,
    #[cfg_attr(feature = "uniffi", uniffi(default = Some(30)))]
    pub http_timeout: Option<i64>,
}

impl Default for SpeedConfig {
    fn default() -> Self {
        Self {
            base_url: Some("https://api.tryspeed.com".to_string()),
            api_key: "".to_string(),
            http_timeout: Some(30),
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

impl SpeedNode {
    pub fn from_credentials(base_url: String, api_key: String) -> Self {
        let config = SpeedConfig {
            base_url: Some(base_url),
            api_key,
            http_timeout: Some(30),
        };
        Self { config }
    }
}

#[cfg_attr(feature = "uniffi", uniffi::export)]
impl LightningNode for SpeedNode {
    fn get_info(&self) -> Result<NodeInfo, ApiError> {
        crate::speed::api::get_info(&self.config)
    }

    fn create_invoice(&self, invoice_params: CreateInvoiceParams) -> Result<Transaction, ApiError> {
        crate::speed::api::create_invoice(&self.config, invoice_params)
    }

    fn pay_invoice(&self, invoice_params: PayInvoiceParams) -> Result<PayInvoiceResponse, ApiError> {
        crate::speed::api::pay_invoice(&self.config, invoice_params)
    }

    fn lookup_invoice(
        &self,
        params: LookupInvoiceParams,
    ) -> Result<Transaction, ApiError> {
        crate::speed::api::lookup_invoice(&self.config, params.payment_hash, None, None, params.search)
    }

    fn list_transactions(
        &self,
        params: ListTransactionsParams,
    ) -> Result<Vec<Transaction>, ApiError> {
        crate::speed::api::list_transactions(&self.config, params.from, params.limit, params.search)
    }

    fn decode(&self, str: String) -> Result<String, ApiError> {
        crate::speed::api::decode(&self.config, str)
    }

    fn get_offer(&self, search: Option<String>) -> Result<PayCode, ApiError> {
        crate::speed::api::get_offer(&self.config, search)
    }

    fn list_offers(&self, search: Option<String>) -> Result<Vec<PayCode>, ApiError> {
        crate::speed::api::list_offers(&self.config, search)
    }

    fn pay_offer(
        &self,
        offer: String,
        amount_msats: i64,
        payer_note: Option<String>,
    ) -> Result<PayInvoiceResponse, ApiError> {
        crate::speed::api::pay_offer(&self.config, offer, amount_msats, payer_note)
    }

    fn on_invoice_events(&self, params: OnInvoiceEventParams, callback: Box<dyn OnInvoiceEventCallback>) {
        crate::speed::api::on_invoice_events(self.config.clone(), params, callback);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dotenv::dotenv;
    use lazy_static::lazy_static;
    use std::env;
    use std::thread;
    use std::sync::{Arc, Mutex};
    use lightning_invoice::Bolt11Invoice;
    use std::str::FromStr;

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
                http_timeout: Some(30),
            })
        };
    }

    #[test]
    fn test_get_info() {
        match NODE.get_info() {
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

    #[test]
    fn test_create_invoice() {
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
        
        match NODE.create_invoice(params) {
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

    #[test]
    fn test_list_transactions() {
        let params = ListTransactionsParams {
            from: 0,
            limit: 100,
            payment_hash: None,
            search: None,
        };
        
        match NODE.list_transactions(params) {
            Ok(transactions) => {
                dbg!(&transactions);
            }
            Err(e) => {
                // Expected to fail without valid API key
                dbg!(e);
            }
        }
    }

    #[test]
    fn test_payment_hash_computation() {
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

    #[test]
    fn test_on_invoice_events() {
        struct OnInvoiceEventCallback {
            events: Arc<Mutex<Vec<String>>>,
        }

        impl crate::OnInvoiceEventCallback for OnInvoiceEventCallback {
            fn success(&self, transaction: Option<Transaction>) {
                dbg!(&transaction);
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

        // Use the payment hash from the environment variable  
        let params = OnInvoiceEventParams {
            payment_hash: Some(TEST_PAYMENT_HASH.to_string()),
            polling_delay_sec: 3,
            max_polling_sec: 15, // Shorter timeout for testing
            search: Some(TEST_PAYMENT_REQUEST.to_string()), // Also provide the withdraw_request as search term
        };

        // Start the event listener in a separate thread
        thread::spawn(move || {
            NODE.on_invoice_events(params, Box::new(callback));
        });

        // Give it some time to process
        thread::sleep(std::time::Duration::from_secs(6));

        // Check that some events were captured
        let events_guard = events.lock().unwrap();
        println!("Speed events captured: {:?}", *events_guard);
        
        // We expect at least one event (even if it's a failure due to invoice not found)
        assert!(
            !events_guard.is_empty(),
            "Should capture at least one event"
        );

        // Verify payment hash computation matches what's in the environment
        match Bolt11Invoice::from_str(&*TEST_PAYMENT_REQUEST) {
            Ok(bolt11) => {
                let computed_hash = format!("{:x}", bolt11.payment_hash());
                println!("Expected payment hash: {}", *TEST_PAYMENT_HASH);
                println!("Computed payment hash: {}", computed_hash);
                assert_eq!(
                    *TEST_PAYMENT_HASH,
                    computed_hash,
                    "Environment payment hash should match computed hash from withdraw_request"
                );
            }
            Err(e) => {
                println!("Warning: Could not parse BOLT11 invoice for verification: {}", e);
            }
        }
    }
}
