#[cfg(feature = "napi_rs")]
use napi_derive::napi;

use crate::types::NodeInfo;
use crate::{
    ApiError, CreateInvoiceParams, LightningNode, ListTransactionsParams, PayCode,
    PayInvoiceParams, PayInvoiceResponse, Transaction,
};

#[cfg_attr(feature = "napi_rs", napi(object))]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
#[derive(Debug, Clone)]
pub struct ClnConfig {
    pub url: String,
    pub rune: String,
    pub socks5_proxy: Option<String>, // socks5h://127.0.0.1:9150
    pub accept_invalid_certs: Option<bool>,
    pub http_timeout: Option<i64>,
}
impl Default for ClnConfig {
    fn default() -> Self {
        Self {
            url: "https://127.0.0.1:8080".to_string(),
            rune: "".to_string(),
            socks5_proxy: None,
            accept_invalid_certs: Some(true),
            http_timeout: Some(60),
        }
    }
}

#[cfg_attr(feature = "napi_rs", napi(object))]
#[cfg_attr(feature = "uniffi", derive(uniffi::Object))]
pub struct ClnNode {
    pub config: ClnConfig,
}

// Constructor is inherent, not part of the trait
#[cfg_attr(feature = "uniffi", uniffi::export)]
impl ClnNode {
    #[cfg_attr(feature = "uniffi", uniffi::constructor)]
    pub fn new(config: ClnConfig) -> Self {
        Self { config }
    }
}

#[cfg_attr(feature = "uniffi", uniffi::export)]
impl LightningNode for ClnNode {
    fn get_info(&self) -> Result<NodeInfo, ApiError> {
        crate::cln::api::get_info(&self.config)
    }

    fn create_invoice(&self, params: CreateInvoiceParams) -> Result<Transaction, ApiError> {
        crate::cln::api::create_invoice(
            &self.config,
            params.invoice_type,
            params.amount_msats,
            params.offer.clone(),
            params.description,
            params.description_hash,
            params.expiry,
        )
    }

    fn pay_invoice(&self, params: PayInvoiceParams) -> Result<PayInvoiceResponse, ApiError> {
        crate::cln::api::pay_invoice(&self.config, params)
    }

    fn get_offer(&self, search: Option<String>) -> Result<PayCode, ApiError> {
        crate::cln::api::get_offer(&self.config, search)
    }

    fn list_offers(&self, search: Option<String>) -> Result<Vec<PayCode>, ApiError> {
        crate::cln::api::list_offers(&self.config, search)
    }

    fn pay_offer(
        &self,
        offer: String,
        amount_msats: i64,
        payer_note: Option<String>,
    ) -> Result<PayInvoiceResponse, ApiError> {
        crate::cln::api::pay_offer(&self.config, offer, amount_msats, payer_note)
    }

    fn lookup_invoice(&self, payment_hash: String) -> Result<crate::Transaction, ApiError> {
        crate::cln::api::lookup_invoice(&self.config, Some(payment_hash), None, None)
    }

    fn list_transactions(
        &self,
        params: ListTransactionsParams,
    ) -> Result<Vec<crate::Transaction>, ApiError> {
        crate::cln::api::list_transactions(&self.config, params.from, params.limit)
    }

    fn decode(&self, str: String) -> Result<String, ApiError> {
        crate::cln::api::decode(&self.config, str)
    }

    fn on_invoice_events(
        &self,
        params: crate::types::OnInvoiceEventParams,
        callback: Box<dyn crate::types::OnInvoiceEventCallback>,
    ) {
        crate::cln::api::on_invoice_events(self.config.clone(), params, callback)
    }
}

#[cfg(test)]
mod tests {
    use crate::InvoiceType;

    use super::*;
    use dotenv::dotenv;
    use lazy_static::lazy_static;
    use std::env;

    lazy_static! {
        static ref URL: String = {
            dotenv().ok();
            env::var("CLN_URL").expect("CLN_URL must be set")
        };
        static ref RUNE: String = {
            dotenv().ok();
            env::var("CLN_RUNE").expect("CLN_RUNE must be set")
        };
        static ref PHOENIX_MOBILE_OFFER: String = {
            dotenv().ok();
            env::var("PHOENIX_MOBILE_OFFER").expect("PHOENIX_MOBILE_OFFER must be set")
        };
        static ref CLN_OFFER: String = {
            dotenv().ok();
            env::var("CLN_OFFER").expect("CLN_OFFER must be set")
        };
        static ref TEST_PAYMENT_HASH: String = {
            dotenv().ok();
            env::var("CLN_TEST_PAYMENT_HASH").expect("CLN_TEST_PAYMENT_HASH must be set")
        };
        static ref NODE: ClnNode = {
            ClnNode::new(ClnConfig {
                url: URL.clone(),
                rune: RUNE.clone(),
                // socks5_proxy: Some("socks5h://127.0.0.1:9150".to_string()),
                // accept_invalid_certs: Some(true)
                ..Default::default()
            })
        };
    }

    #[test]
    fn test_get_info() {
        match NODE.get_info() {
            Ok(info) => {
                println!("info: {:?}", info);
                assert!(!info.pubkey.is_empty(), "Node pubkey should not be empty");
            }
            Err(e) => {
                panic!("Failed to get offer: {:?}", e);
            }
        }
    }

    #[test]
    fn test_create_invoice() {
        let amount_msats = 3000;
        let description = "Test invoice".to_string();
        let description_hash = "".to_string();
        let expiry = 3600;

        // BOLT11
        match NODE.create_invoice(CreateInvoiceParams {
            invoice_type: InvoiceType::Bolt11,
            amount_msats: Some(amount_msats),
            description: Some(description.clone()),
            description_hash: Some(description_hash.clone()),
            expiry: Some(expiry),
            ..Default::default()
        }) {
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

        // BOLT11 - Zero amount
        match NODE.create_invoice(CreateInvoiceParams {
            invoice_type: InvoiceType::Bolt11,
            expiry: Some(expiry),
            ..Default::default()
        }) {
            Ok(txn) => {
                println!("BOLT11 - Zero amount: {:?}", txn);
                assert!(
                    !txn.invoice.is_empty(),
                    "BOLT11 - Zero amount Invoice should not be empty"
                );
            }
            Err(e) => {
                panic!("BOLT11 - Zero amount Failed to make invoice: {:?}", e);
            }
        }

        // BOLT12
        match NODE.create_invoice(CreateInvoiceParams {
            invoice_type: InvoiceType::Bolt12,
            amount_msats: Some(amount_msats),
            offer: Some(PHOENIX_MOBILE_OFFER.to_string()),
            description: Some(description.clone()),
            description_hash: None,
            expiry: Some(expiry),
            ..Default::default()
        }) {
            Ok(txn) => {
                println!("BOLT12 create_invoice from offer: {:?}", txn);
                assert!(
                    !txn.invoice.is_empty(),
                    "BOLT12 Invoice should not be empty"
                );
            }
            Err(e) => {
                panic!(
                    "BOLT12 create_invoice from offer Failed to make invoice: {:?}",
                    e
                );
            }
        }

        // TODO test zero amount offers (i.e the amount is embedded in the offer)
    }

    #[test]
    fn test_pay_invoice() {
        match NODE.pay_invoice(PayInvoiceParams {
            invoice: "".to_string(),    // TODO remote grab a invoice maybe from LNURL
            fee_limit_msat: Some(5000), // 5 sats fee limit
            ..Default::default()
        }) {
            Ok(invoice_resp) => {
                println!("Pay invoice resp: {:?}", invoice_resp);
                assert!(
                    !invoice_resp.payment_hash.is_empty(),
                    "Payment Hash should not be empty"
                );
            }
            Err(e) => {
                panic!("Failed to pay invoice: {:?}", e);
            }
        }
    }
    #[test]
    fn test_list_offers() {
        match NODE.get_offer(None) {
            Ok(resp) => {
                println!("Get offer: {:?}", resp);
            }
            Err(e) => {
                panic!("Failed to get offer: {:?}", e);
            }
        }
        match NODE.list_offers(None) {
            Ok(resp) => {
                println!("List offers: {:?}", resp);
            }
            Err(e) => {
                panic!("Failed to list offer: {:?}", e);
            }
        }
    }

    #[test]
    fn test_pay_offer() {
        match NODE.pay_offer(
            PHOENIX_MOBILE_OFFER.to_string(),
            3000,
            Some("from LNI test".to_string()),
        ) {
            Ok(pay_resp) => {
                println!("pay_resp: {:?}", pay_resp);
                assert!(
                    !pay_resp.payment_hash.is_empty(),
                    "Payment hash should not be empty"
                );
            }
            Err(e) => {
                panic!("Failed to get offer: {:?}", e);
            }
        }
    }

    #[test]
    fn test_lookup_invoice() {
        match NODE.lookup_invoice(TEST_PAYMENT_HASH.to_string()) {
            Ok(txn) => {
                println!("invoice: {:?}", txn);
                assert!(
                    txn.amount_msats >= 0,
                    "Invoice should contain a valid amount"
                );
            }
            Err(e) => {
                panic!("Failed to lookup invoice: {:?}", e);
            }
        }
    }

    #[test]
    fn test_list_transactions() {
        let params = ListTransactionsParams {
            from: 0,
            limit: 10,
            payment_hash: None,
        };
        match NODE.list_transactions(params) {
            Ok(txns) => {
                println!("transactions: {:?}", txns);
                assert!(
                    txns.len() >= 0,
                    "Should contain at least zero or one transaction"
                );
            }
            Err(e) => {
                panic!("Failed to lookup transactions: {:?}", e);
            }
        }
    }

    #[test]
    fn test_decode() {
        match NODE.decode(PHOENIX_MOBILE_OFFER.to_string()) {
            Ok(txns) => {
                println!("decode: {:?}", txns);
            }
            Err(e) => {
                panic!("Failed to decode: {:?}", e);
            }
        }
    }

    #[test]
    fn test_on_invoice_events() {
        struct OnInvoiceEventCallback {}
        impl crate::types::OnInvoiceEventCallback for OnInvoiceEventCallback {
            fn success(&self, transaction: Option<Transaction>) {
                println!("success");
            }
            fn pending(&self, transaction: Option<Transaction>) {
                println!("pending");
            }
            fn failure(&self, transaction: Option<Transaction>) {
                println!("epic fail");
            }
        }
        let params = crate::types::OnInvoiceEventParams {
            payment_hash: TEST_PAYMENT_HASH.to_string(),
            polling_delay_sec: 3,
            max_polling_sec: 60,
        };
        let callback = OnInvoiceEventCallback {};
        NODE.on_invoice_events(params, Box::new(callback));
    }
}
