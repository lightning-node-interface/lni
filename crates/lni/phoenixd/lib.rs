#[cfg(feature = "napi_rs")]
use napi_derive::napi;

use crate::{
    phoenixd::api::*, ApiError, ListTransactionsParams, PayInvoiceParams, PayInvoiceResponse,
    Transaction, LightningNode
};

use crate::{CreateInvoiceParams, LookupInvoiceParams, PayCode};

#[cfg_attr(feature = "napi_rs", napi(object))]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
#[derive(Debug, Clone)]
pub struct PhoenixdConfig {
    pub url: String,
    pub password: String,
   #[cfg_attr(feature = "uniffi", uniffi(default = Some("")))]
    pub socks5_proxy: Option<String>, // Some("socks5h://127.0.0.1:9150") or Some("".to_string())
    #[cfg_attr(feature = "uniffi", uniffi(default = Some(true)))]
    pub accept_invalid_certs: Option<bool>,
    #[cfg_attr(feature = "uniffi", uniffi(default = Some(120)))]
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
#[derive(Debug, Clone)]
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

#[cfg_attr(feature = "uniffi", uniffi::export(async_runtime = "tokio"))]
#[async_trait::async_trait]
impl LightningNode for PhoenixdNode {
    async fn get_info(&self) -> Result<crate::NodeInfo, ApiError> {
        crate::phoenixd::api::get_info(self.config.clone()).await
    }

    async fn create_invoice(&self, params: CreateInvoiceParams) -> Result<Transaction, ApiError> {
        create_invoice(
            self.config.clone(),
            params.invoice_type,
            Some(params.amount_msats.unwrap_or_default()),
            params.description,
            params.description_hash,
            params.expiry,
        ).await
    }

    async fn pay_invoice(&self, params: PayInvoiceParams) -> Result<PayInvoiceResponse, ApiError> {
        pay_invoice(self.config.clone(), params).await
    }

    async fn get_offer(&self, _search: Option<String>) -> Result<PayCode, ApiError> {
        crate::phoenixd::api::get_offer(self.config.clone()).await
    }

    async fn list_offers(&self, _search: Option<String>) -> Result<Vec<PayCode>, ApiError> {
        crate::phoenixd::api::list_offers()
    }

    async fn pay_offer(
        &self,
        offer: String,
        amount_msats: i64,
        payer_note: Option<String>,
    ) -> Result<PayInvoiceResponse, ApiError> {
        crate::phoenixd::api::pay_offer(self.config.clone(), offer, amount_msats, payer_note).await
    }

    async fn lookup_invoice(&self, params: LookupInvoiceParams) -> Result<crate::Transaction, ApiError> {
        crate::phoenixd::api::lookup_invoice(
            self.config.clone(),
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
        crate::phoenixd::api::list_transactions(self.config.clone(), params).await
    }

    async fn decode(&self, _str: String) -> Result<String, ApiError> {
        Ok("".to_string())
    }

    async fn on_invoice_events(
        &self,
        params: crate::types::OnInvoiceEventParams,
        callback: Box<dyn crate::types::OnInvoiceEventCallback>,
    ) {
        crate::phoenixd::api::on_invoice_events(self.config.clone(), params, callback).await
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

    #[tokio::test]
    async fn test_get_info() {
        match NODE.get_info().await {
            Ok(info) => {
                println!("info: {:?}", info);
                assert!(!info.pubkey.is_empty(), "Node pubkey should not be empty");
            }
            Err(e) => {
                panic!("Failed to get offer: {:?}", e);
            }
        }
    }

    #[tokio::test]
    async fn test_create_invoice() {
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

        match NODE.create_invoice(params).await {
            Ok(txn) => {
                println!("txn: {:?}", txn);
                assert!(!txn.invoice.is_empty(), "Invoice should not be empty");
            }
            Err(e) => {
                panic!("Failed to make invoice: {:?}", e);
            }
        }
    }

    #[tokio::test]
    async fn test_pay_invoice() {
        match NODE.pay_invoice(PayInvoiceParams {
            invoice: "".to_string(), // TODO pull from somewhere
            ..Default::default()
        }).await {
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

    #[tokio::test]
    async fn test_get_offer() {
        match NODE.get_offer(None).await {
            Ok(resp) => {
                println!("Get Offer resp: {:?}", resp);
                assert!(!resp.bolt12.is_empty(), "Offer should not be empty");
            }
            Err(e) => {
                panic!("Failed to get offer: {:?}", e);
            }
        }
    }

    #[tokio::test]
    async fn test_pay_offer() {
        match NODE.pay_offer(
            TEST_RECEIVER_OFFER.to_string(),
            2000,
            Some("payment from lni".to_string()),
        ).await {
            Ok(resp) => {
                println!("Pay invoice resp: {:?}", resp);
                assert!(!resp.preimage.is_empty(), "Preimage should not be empty");
            }
            Err(e) => {
                panic!("Failed to pay offer: {:?}", e);
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
                dbg!(&txns);
                // You can add more assertions here if desired
                assert!(true, "Successfully fetched transactions");
            }
            Err(e) => {
                panic!("Failed to list transactions: {:?}", e);
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
                dbg!(&txn);
                assert!(txn.amount_msats.gt(&1), "Invoice should contain an amount");
            }
            Err(e) => {
                panic!("Failed to lookup invoice: {:?}", e);
            }
        }
    }

    #[tokio::test]
    async fn test_on_invoice_events() {
        struct OnInvoiceEventCallback {}
        impl crate::types::OnInvoiceEventCallback for OnInvoiceEventCallback {
            fn success(&self, transaction: Option<Transaction>) {
                dbg!(transaction);
                println!("success");
            }
            fn pending(&self, transaction: Option<Transaction>) {
                println!("pending {:?}", transaction);
            }
            fn failure(&self, _transaction: Option<Transaction>) {
                println!("epic fail");
            }
        }
        let params = crate::types::OnInvoiceEventParams {
            search: Some(TEST_PAYMENT_HASH.to_string()),
            polling_delay_sec: 2,
            max_polling_sec: 6,
            ..Default::default()
        };
        let callback = OnInvoiceEventCallback {};
        NODE.on_invoice_events(params, Box::new(callback)).await;
    }
}
