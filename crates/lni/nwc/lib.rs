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
pub struct NwcConfig {
    pub nwc_uri: String, // The full NWC URI string like "nostr+walletconnect://pubkey?relay=...&secret=..."
    #[cfg_attr(feature = "uniffi", uniffi(default = Some("")))]
    pub socks5_proxy: Option<String>, // Some("socks5h://127.0.0.1:9150") or Some("".to_string())
    #[cfg_attr(feature = "uniffi", uniffi(default = Some(true)))]
    pub accept_invalid_certs: Option<bool>,
    #[cfg_attr(feature = "uniffi", uniffi(default = Some(120)))]
    pub http_timeout: Option<i64>,
}

impl Default for NwcConfig {
    fn default() -> Self {
        Self {
            nwc_uri: "".to_string(),
            socks5_proxy: Some("".to_string()),
            accept_invalid_certs: Some(true),
            http_timeout: Some(60),
        }
    }
}

#[cfg_attr(feature = "napi_rs", napi(object))]
#[cfg_attr(feature = "uniffi", derive(uniffi::Object))]
pub struct NwcNode {
    pub config: NwcConfig,
}

// Constructor is inherent, not part of the trait
#[cfg_attr(feature = "uniffi", uniffi::export)]
impl NwcNode {
    #[cfg_attr(feature = "uniffi", uniffi::constructor)]
    pub fn new(config: NwcConfig) -> Self {
        Self { config }
    }
}

#[cfg_attr(feature = "uniffi", uniffi::export)]
impl LightningNode for NwcNode {
    async fn get_info(&self) -> Result<NodeInfo, ApiError> {
        crate::nwc::api::get_info(self.config.clone()).await
    }

    async fn create_invoice(&self, params: CreateInvoiceParams) -> Result<Transaction, ApiError> {
        crate::nwc::api::create_invoice(self.config.clone(), params).await
    }

    async fn pay_invoice(&self, params: PayInvoiceParams) -> Result<PayInvoiceResponse, ApiError> {
        crate::nwc::api::pay_invoice(self.config.clone(), params).await
    }

    async fn get_offer(&self, search: Option<String>) -> Result<PayCode, ApiError> {
        crate::nwc::api::get_offer(&self.config, search).await
    }

    async fn list_offers(&self, search: Option<String>) -> Result<Vec<PayCode>, ApiError> {
        crate::nwc::api::list_offers(&self.config, search).await
    }

    async fn pay_offer(
        &self,
        offer: String,
        amount_msats: i64,
        payer_note: Option<String>,
    ) -> Result<PayInvoiceResponse, ApiError> {
        crate::nwc::api::pay_offer(&self.config, offer, amount_msats, payer_note).await
    }

    async fn lookup_invoice(&self, params: LookupInvoiceParams) -> Result<crate::Transaction, ApiError> {
        crate::nwc::api::lookup_invoice(self.config.clone(), params.payment_hash, params.search).await
    }

    async fn list_transactions(
        &self,
        params: ListTransactionsParams,
    ) -> Result<Vec<crate::Transaction>, ApiError> {
        crate::nwc::api::list_transactions(self.config.clone(), params).await
    }

    async fn decode(&self, str: String) -> Result<String, ApiError> {
        crate::nwc::api::decode(self.config.clone(), str).await
    }

    async fn on_invoice_events(
        &self,
        params: crate::types::OnInvoiceEventParams,
        callback: Box<dyn crate::types::OnInvoiceEventCallback>,
    ) {
        crate::nwc::api::on_invoice_events(self.config.clone(), params, callback).await
    }
}

#[cfg(test)]
mod tests {
    use crate::InvoiceType;

    use super::*;
    use dotenv::dotenv;
    use lazy_static::lazy_static;
    use std::env;
    use std::sync::{Arc, Mutex};

    lazy_static! {
        static ref NWC_URI: String = {
            dotenv().ok();
            env::var("NWC_URI").expect("NWC_URI must be set")
        };
        static ref NODE: NwcNode = {
            NwcNode::new(NwcConfig {
                nwc_uri: NWC_URI.clone(),
                //socks5_proxy: Some("socks5h://127.0.0.1:9150".to_string()), // Tor socks5 proxy using arti
                ..Default::default()
            })
        };
        static ref TEST_PAYMENT_HASH: String = {
            dotenv().ok();
            env::var("NWC_TEST_PAYMENT_HASH").expect("NWC_TEST_PAYMENT_HASH must be set")
        };
        static ref TEST_PAYMENT_REQUEST: String = {
            dotenv().ok();
            env::var("NWC_TEST_PAYMENT_REQUEST").expect("NWC_TEST_PAYMENT_REQUEST must be set")
        };
    }

    #[tokio::test]
    async fn test_get_info() {
        match NODE.get_info().await {
            Ok(info) => {
                println!("info: {:?}", info);
                assert!(!info.pubkey.is_empty(), "Node pubkey should not be empty");
            }
            Err(e) => {
                panic!("Failed to get info: {:?}", e);
            }
        }
    }

    #[tokio::test]
    async fn test_create_invoice() {
        let amount_msats = 3000;
        let description = "Test NWC invoice".to_string();
        let expiry = 3600;

        match NODE.create_invoice(CreateInvoiceParams {
            invoice_type: InvoiceType::Bolt11,
            amount_msats: Some(amount_msats),
            description: Some(description.clone()),
            expiry: Some(expiry),
            ..Default::default()
        }).await {
            Ok(txn) => {
                println!("BOLT11 create_invoice: {:?}", txn);
                assert!(
                    !txn.invoice.is_empty(),
                    "BOLT11 create_invoice Invoice should not be empty"
                );
            }
            Err(e) => {
                panic!("BOLT11 create_invoice Failed to make invoice: {:?}", e);
            }
        }
    }

    // #[tokio::test]
    // async fn test_pay_invoice() {
    //     match NODE.pay_invoice(PayInvoiceParams {
    //         invoice: TEST_PAYMENT_REQUEST.clone(),
    //         fee_limit_percentage: Some(1.0), // 1% fee limit
    //         ..Default::default()
    //     }).await {
    //         Ok(invoice_resp) => {
    //             println!("Pay invoice resp: {:?}", invoice_resp);
    //             assert!(
    //                 !invoice_resp.payment_hash.is_empty(),
    //                 "Payment Hash should not be empty"
    //             );
    //         }
    //         Err(e) => {
    //             panic!("Failed to pay invoice: {:?}", e);
    //         }
    //     }
    // }

    // #[tokio::test]
    // async fn test_lookup_invoice() {
    //     match NODE.lookup_invoice(LookupInvoiceParams {
    //         payment_hash: Some(TEST_PAYMENT_HASH.to_string()),
    //         ..Default::default()
    //     }).await {
    //         Ok(txn) => {
    //             dbg!(&txn);
    //             assert!(
    //                 txn.amount_msats >= 0,
    //                 "Invoice should contain a valid amount"
    //             );
    //         }
    //         Err(e) => {
    //             if e.to_string().contains("not found") {
    //                 assert!(true, "Invoice not found as expected");
    //             } else {
    //                 panic!("Failed to lookup invoice: {:?}", e);
    //             }
    //         }
    //     }
    // }

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
                dbg!(&txns);
                assert!(
                    true, // Just verify the call succeeds
                    "List transactions call should succeed"
                );
            }
            Err(e) => {
                panic!("Failed to lookup transactions: {:?}", e);
            }
        }
    }

    // #[tokio::test]
    // async fn test_decode() {
    //     match NODE.decode(TEST_PAYMENT_REQUEST.to_string()).await {
    //         Ok(decoded) => {
    //             println!("decode: {:?}", decoded);
    //         }
    //         Err(e) => {
    //             panic!("Failed to decode: {:?}", e);
    //         }
    //     }
    // }

    #[tokio::test]
    async fn test_on_invoice_events() {
        struct TestCallback {
            events: Arc<Mutex<Vec<String>>>,
        }

        impl crate::types::OnInvoiceEventCallback for TestCallback {
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
        let callback = TestCallback {
            events: events.clone(),
        };

        let params = crate::types::OnInvoiceEventParams {
            payment_hash: Some(TEST_PAYMENT_HASH.to_string()),
            polling_delay_sec: 2,
            max_polling_sec: 10,
            ..Default::default()
        };

        // Start the event listener
        NODE.on_invoice_events(params, Box::new(callback)).await;

        // Check if events were received
        let received_events = events.lock().unwrap();
        println!("Received events: {:?}", *received_events);
        assert!(
            !received_events.is_empty(),
            "Expected to receive at least one invoice event"
        );
    }
}
