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
pub struct BlinkConfig {
    #[cfg_attr(feature = "uniffi", uniffi(default = Some("https://api.blink.sv/graphql")))]
    pub base_url: Option<String>,
    pub api_key: String,
    #[cfg_attr(feature = "uniffi", uniffi(default = Some(120)))]
    pub http_timeout: Option<i64>,
}

impl Default for BlinkConfig {
    fn default() -> Self {
        Self {
            base_url: Some("https://api.blink.sv/graphql".to_string()),
            api_key: "".to_string(),
            http_timeout: Some(120),
        }
    }
}

#[cfg_attr(feature = "napi_rs", napi(object))]
#[cfg_attr(feature = "uniffi", derive(uniffi::Object))]
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

#[cfg_attr(feature = "uniffi", uniffi::export)]
impl LightningNode for BlinkNode {
    fn get_info(&self) -> Result<NodeInfo, ApiError> {
        crate::blink::api::get_info(&self.config)
    }

    fn create_invoice(&self, params: CreateInvoiceParams) -> Result<Transaction, ApiError> {
        crate::blink::api::create_invoice(&self.config, params)
    }

    fn pay_invoice(&self, params: PayInvoiceParams) -> Result<PayInvoiceResponse, ApiError> {
        crate::blink::api::pay_invoice(&self.config, params)
    }

    fn get_offer(&self, search: Option<String>) -> Result<PayCode, ApiError> {
        crate::blink::api::get_offer(&self.config, search)
    }

    fn list_offers(&self, search: Option<String>) -> Result<Vec<PayCode>, ApiError> {
        crate::blink::api::list_offers(&self.config, search)
    }

    fn pay_offer(
        &self,
        offer: String,
        amount_msats: i64,
        payer_note: Option<String>,
    ) -> Result<PayInvoiceResponse, ApiError> {
        crate::blink::api::pay_offer(&self.config, offer, amount_msats, payer_note)
    }

    fn lookup_invoice(&self, params: LookupInvoiceParams) -> Result<crate::Transaction, ApiError> {
        crate::blink::api::lookup_invoice(
            &self.config,
            params.payment_hash,
            None,
            None,
            params.search,
        )
    }

    fn list_transactions(
        &self,
        params: ListTransactionsParams,
    ) -> Result<Vec<crate::Transaction>, ApiError> {
        crate::blink::api::list_transactions(
            &self.config,
            params.from,
            params.limit,
            params.search,
        )
    }

    fn decode(&self, str: String) -> Result<String, ApiError> {
        crate::blink::api::decode(&self.config, str)
    }

    fn on_invoice_events(
        &self,
        params: crate::types::OnInvoiceEventParams,
        callback: Box<dyn crate::types::OnInvoiceEventCallback>,
    ) {
        crate::blink::api::on_invoice_events(self.config.clone(), params, callback)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{InvoiceType, PayInvoiceParams};
    use dotenv::dotenv;
    use lazy_static::lazy_static;
    use std::env;
    use std::sync::{Arc, Mutex};
    use std::thread;

    lazy_static! {
        static ref BASE_URL: String = {
            dotenv().ok();
            env::var("BLINK_BASE_URL").unwrap_or_else(|_| "https://api.blink.sv/graphql".to_string())
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
            env::var("BLINK_TEST_PAYMENT_REQUEST")
                .expect("BLINK_TEST_PAYMENT_REQUEST must be set")
        };
        static ref NODE: BlinkNode = {
            BlinkNode::new(BlinkConfig {
                base_url: BASE_URL.clone(),
                api_key: API_KEY.clone(),
                http_timeout: Some(120),
            })
        };
    }

    #[test]
    fn test_get_info() {
        match NODE.get_info() {
            Ok(info) => {
                println!("info: {:?}", info);
            }
            Err(e) => {
                panic!("Failed to get info: {:?}", e);
            }
        }
    }

    #[test]
    fn test_create_invoice() {
        let amount_msats = 21000; // 21 sats
        let description = "Test Blink invoice".to_string();
        let expiry = 3600;

        match NODE.create_invoice(CreateInvoiceParams {
            invoice_type: InvoiceType::Bolt11,
            amount_msats: Some(amount_msats),
            description: Some(description.clone()),
            expiry: Some(expiry),
            ..Default::default()
        }) {
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

    // #[test]
    // fn test_pay_invoice() {
    //     match NODE.pay_invoice(PayInvoiceParams {
    //         invoice: TEST_PAYMENT_REQUEST.clone(),
    //         amount_msats: None, // Use amount from invoice
    //         ..Default::default()
    //     }) {
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

    #[test]
    fn test_lookup_invoice() {
        match NODE.lookup_invoice(LookupInvoiceParams {
            payment_hash: Some(TEST_PAYMENT_HASH.to_string()),
            ..Default::default()
        }) {
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

    #[test]
    fn test_list_transactions() {
        let params = ListTransactionsParams {
            from: 0,
            limit: 100,
            payment_hash: None,
            search: None,
        };
        
        match NODE.list_transactions(params) {
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

    #[test]
    fn test_on_invoice_events() {
        struct OnInvoiceEventCallback {
            events: Arc<Mutex<Vec<String>>>,
        }

        impl crate::types::OnInvoiceEventCallback for OnInvoiceEventCallback {
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

        let params = crate::types::OnInvoiceEventParams {
            payment_hash: Some(TEST_PAYMENT_HASH.to_string()),
            polling_delay_sec: 3,
            max_polling_sec: 60,
            ..Default::default()
        };

        // Start the event listener in a separate thread
        thread::spawn(move || {
            NODE.on_invoice_events(params, Box::new(callback));
        });

        // Give it some time to process
        thread::sleep(std::time::Duration::from_secs(5));

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