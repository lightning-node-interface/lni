#[cfg(feature = "napi_rs")]
use napi_derive::napi;

use crate::{
    phoenixd::api::*, ApiError, ListTransactionsParams, PayInvoiceParams, PayInvoiceResponse,
    Transaction,
};

use crate::{CreateInvoiceParams, LightningNode, PayCode};

#[cfg_attr(feature = "napi_rs", napi(object))]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
#[derive(Debug, Clone)]
pub struct PhoenixdConfig {
    pub url: String,
    pub password: String,
    pub socks5_proxy: Option<String>, // socks5h://127.0.0.1:9150
    pub accept_invalid_certs: Option<bool>,
    pub http_timeout: Option<i64>,
}
impl Default for PhoenixdConfig {
    fn default() -> Self {
        Self {
            url: "https://127.0.0.1:8080".to_string(),
            password: "".to_string(),
            socks5_proxy: None,
            accept_invalid_certs: Some(true),
            http_timeout: Some(60),
        }
    }
}

#[cfg_attr(feature = "napi_rs", napi(object))]
#[cfg_attr(feature = "uniffi", derive(uniffi::Object))]
pub struct PhoenixdNode {
    pub config: PhoenixdConfig,
}

// Constructor is inherent, not part of the trait
#[cfg_attr(feature = "uniffi", uniffi::export)]
impl PhoenixdNode {
    #[cfg_attr(feature = "uniffi", uniffi::constructor)]
    pub fn new(config: PhoenixdConfig) -> Self {
        Self { config }
    }
}

#[cfg_attr(feature = "uniffi", uniffi::export)]
impl LightningNode for PhoenixdNode {
    fn get_info(&self) -> Result<crate::NodeInfo, ApiError> {
        crate::phoenixd::api::get_info(&self.config)
    }

    fn create_invoice(&self, params: CreateInvoiceParams) -> Result<Transaction, ApiError> {
        create_invoice(
            &self.config,
            params.invoice_type,
            Some(params.amount_msats.unwrap_or_default()),
            params.description,
            params.description_hash,
            params.expiry,
        )
    }

    fn pay_invoice(&self, params: PayInvoiceParams) -> Result<PayInvoiceResponse, ApiError> {
        pay_invoice(&self.config, params)
    }

    fn get_offer(&self, search: Option<String>) -> Result<PayCode, ApiError> {
        crate::phoenixd::api::get_offer(&self.config)
    }

    fn list_offers(&self, search: Option<String>) -> Result<Vec<PayCode>, ApiError> {
        crate::phoenixd::api::list_offers()
    }

    fn pay_offer(
        &self,
        offer: String,
        amount_msats: i64,
        payer_note: Option<String>,
    ) -> Result<PayInvoiceResponse, ApiError> {
        crate::phoenixd::api::pay_offer(&self.config, offer, amount_msats, payer_note)
    }

    fn lookup_invoice(&self, payment_hash: String) -> Result<crate::Transaction, ApiError> {
        crate::phoenixd::api::lookup_invoice(&self.config, payment_hash)
    }

    fn list_transactions(
        &self,
        params: ListTransactionsParams,
    ) -> Result<Vec<crate::Transaction>, ApiError> {
        crate::phoenixd::api::list_transactions(&self.config, params.from, params.limit, None)
    }

    fn decode(&self, str: String) -> Result<String, ApiError> {
        Ok("".to_string())
    }

    fn on_invoice_events(
        &self,
        params: crate::types::OnInvoiceEventParams,
        callback: Box<dyn crate::types::OnInvoiceEventCallback>,
    ) {
        crate::phoenixd::api::on_invoice_events(self.config.clone(), params, callback)
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
            env::var("PHOENIXD_URL").expect("PHOENIXD_URL must be set")
        };
        static ref PASSWORD: String = {
            dotenv().ok();
            env::var("PHOENIXD_PASSWORD").expect("PHOENIXD_PASSWORD must be set")
        };
        static ref NODE: PhoenixdNode = {
            PhoenixdNode::new(PhoenixdConfig {
                url: URL.clone(),
                password: PASSWORD.clone(),
                // socks5_proxy: "socks5h://127.0.0.1:9150".to_string().into(),
                // accept_invalid_certs: true.into(),
                ..Default::default()
            })
        };
        static ref TEST_PAYMENT_HASH: String = {
            dotenv().ok();
            env::var("PHOENIXD_TEST_PAYMENT_HASH").expect("PHOENIXD_TEST_PAYMENT_HASH must be set")
        };
        static ref TEST_RECEIVER_OFFER: String = {
            dotenv().ok();
            env::var("TEST_RECEIVER_OFFER").expect("TEST_RECEIVER_OFFER must be set")
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
        let amount_msats = 1000;
        let description = "Test invoice".to_string();
        let description_hash = "".to_string();
        let expiry = 3600;
        let params = CreateInvoiceParams {
            invoice_type: InvoiceType::Bolt11,
            amount_msats: Some(amount_msats),
            offer: None,
            description: Some(description),
            description_hash: Some(description_hash),
            expiry: Some(expiry),
            ..Default::default()
        };

        match NODE.create_invoice(params) {
            Ok(txn) => {
                println!("txn: {:?}", txn);
                assert!(!txn.invoice.is_empty(), "Invoice should not be empty");
            }
            Err(e) => {
                panic!("Failed to make invoice: {:?}", e);
            }
        }
    }

    #[test]
    fn test_pay_invoice() {
        match NODE.pay_invoice(PayInvoiceParams {
            invoice: "".to_string(), // TODO pull from somewhere
            ..Default::default()
        }) {
            Ok(txn) => {
                println!("txn: {:?}", txn);
                assert!(
                    !txn.payment_hash.is_empty(),
                    "Payment hash should not be empty"
                );
            }
            Err(e) => {
                panic!("Failed to pay invoice: {:?}", e);
            }
        }
    }

    #[test]
    fn test_get_offer() {
        match NODE.get_offer(None) {
            Ok(resp) => {
                println!("Get Offer resp: {:?}", resp);
                assert!(!resp.bolt12.is_empty(), "Offer should not be empty");
            }
            Err(e) => {
                panic!("Failed to get offer: {:?}", e);
            }
        }
    }

    #[test]
    fn test_pay_offer() {
        match NODE.pay_offer(
            TEST_RECEIVER_OFFER.to_string(),
            2000,
            Some("payment from lni".to_string()),
        ) {
            Ok(resp) => {
                println!("Pay invoice resp: {:?}", resp);
                assert!(!resp.preimage.is_empty(), "Preimage should not be empty");
            }
            Err(e) => {
                panic!("Failed to pay offer: {:?}", e);
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
                println!("Transactions: {:?}", txns);
                // You can add more assertions here if desired
                assert!(true, "Successfully fetched transactions");
            }
            Err(e) => {
                panic!("Failed to list transactions: {:?}", e);
            }
        }
    }

    #[test]
    fn test_lookup_invoice() {
        match NODE.lookup_invoice(TEST_PAYMENT_HASH.to_string()) {
            Ok(txn) => {
                println!("invoice: {:?}", txn);
                assert!(txn.amount_msats.gt(&1), "Invoice contain an amount");
            }
            Err(e) => {
                panic!("Failed to lookup invoice: {:?}", e);
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
                println!("pending {:?}", transaction);
            }
            fn failure(&self, transaction: Option<Transaction>) {
                println!("epic fail");
            }
        }
        let params = crate::types::OnInvoiceEventParams {
            payment_hash: TEST_PAYMENT_HASH.to_string(),
            polling_delay_sec: 3,
            max_polling_sec: 5,
        };
        let callback = OnInvoiceEventCallback {};
        NODE.on_invoice_events(params, Box::new(callback));
    }
}
