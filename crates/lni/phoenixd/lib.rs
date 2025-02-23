#[cfg(feature = "napi_rs")]
use napi_derive::napi;

use crate::{phoenixd::api::*, ApiError, InvoiceType, PayInvoiceResponse, Transaction};
use serde::{Deserialize, Serialize};

#[cfg_attr(feature = "napi_rs", napi(object))]
pub struct PhoenixdConfig {
    pub url: String,
    pub password: String,
}

#[cfg_attr(feature = "napi_rs", napi(object))]
pub struct PhoenixdNode {
    pub url: String,
    pub password: String,
}

#[cfg_attr(feature = "napi_rs", napi(object))]
#[derive(Debug, Serialize, Deserialize)]
pub struct Bolt11Resp {
    #[serde(rename = "amountSat")]
    pub amount_sat: i64,
    #[serde(rename = "paymentHash")]
    pub payment_hash: String,
    #[serde(rename = "serialized")]
    pub serialized: String,
}

#[cfg_attr(feature = "napi_rs", napi(object))]
#[derive(Debug, Serialize, Deserialize)]
pub struct PhoenixdMakeInvoiceParams {
    pub invoice_type: InvoiceType,
    pub amount_msats: i64,
    pub description: Option<String>,
    pub description_hash: Option<String>,
    pub expiry: Option<i64>,
}

#[cfg_attr(feature = "napi_rs", napi(object))]
#[derive(Debug, Serialize, Deserialize)]
pub struct ListTransactionsParams {
    pub from: i64,
    pub until: i64,
    pub limit: i64,
    pub offset: i64,
    pub unpaid: bool,
    pub invoice_type: String, // all
}

impl PhoenixdNode {
    pub fn new(config: PhoenixdConfig) -> Self {
        Self {
            url: config.url,
            password: config.password,
        }
    }

    pub async fn get_info(&self) -> Result<crate::NodeInfo, ApiError> {
        crate::phoenixd::api::get_info(self.url.clone(), self.password.clone())
    }

    pub async fn make_invoice(
        &self,
        params: PhoenixdMakeInvoiceParams,
    ) -> Result<Transaction, ApiError> {
        make_invoice(
            self.url.clone(),
            self.password.clone(),
            params.invoice_type,
            params.amount_msats,
            params.description,
            params.description_hash,
            params.expiry,
        )
        .await
    }

    pub async fn pay_offer(
        &self,
        offer: String,
        amount_msats: i64,
        payer_note: Option<String>,
    ) -> Result<PayInvoiceResponse, ApiError> {
        crate::phoenixd::api::pay_offer(
            self.url.clone(),
            self.password.clone(),
            offer,
            amount_msats,
            payer_note,
        )
        .await
    }

    pub async fn lookup_invoice(
        &self,
        payment_hash: String,
    ) -> Result<crate::Transaction, ApiError> {
        crate::phoenixd::api::lookup_invoice(self.url.clone(), self.password.clone(), payment_hash)
    }

    pub async fn list_transactions(
        &self,
        params: ListTransactionsParams,
    ) -> Result<Vec<crate::Transaction>, ApiError> {
        crate::phoenixd::api::list_transactions(
            self.url.clone(),
            self.password.clone(),
            params.from,
            params.until,
            params.limit,
            params.offset,
            params.unpaid,
            params.invoice_type,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dotenv::dotenv;
    use lazy_static::lazy_static;
    use std::env;
    use tokio::test;

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
            })
        };
        static ref TEST_PAYMENT_HASH: String = {
            dotenv().ok();
            env::var("PHOENIXD_TEST_PAYMENT_HASH").expect("PHOENIXD_TEST_PAYMENT_HASH must be set")
        };
        static ref TEST_OFFER: String = {
            dotenv().ok();
            env::var("TEST_OFFER").expect("TEST_OFFER must be set")
        };
    }

    #[test]
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

    #[test]
    async fn test_make_invoice() {
        let amount_msats = 1000;
        let description = "Test invoice".to_string();
        let description_hash = "".to_string();
        let expiry = 3600;
        let params = PhoenixdMakeInvoiceParams {
            invoice_type: InvoiceType::Bolt11,
            amount_msats,
            description: Some(description),
            description_hash: Some(description_hash),
            expiry: Some(expiry),
        };

        match NODE.make_invoice(params).await {
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
    async fn test_pay_offer() {
        match NODE
            .pay_offer(TEST_OFFER.to_string(), 2000, Some("payment from lni".to_string()))
            .await
        {
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
    async fn test_list_transactions() {
        let params = ListTransactionsParams {
            from: 0,
            until: 0,
            limit: 10,
            offset: 0,
            unpaid: false,
            invoice_type: "all".to_string(),
        };
        match NODE.list_transactions(params).await {
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
    async fn test_lookup_invoice() {
        match NODE.lookup_invoice(TEST_PAYMENT_HASH.to_string()).await {
            Ok(txn) => {
                println!("invoice: {:?}", txn);
                assert!(txn.amount_msats.gt(&1), "Invoice contain an amount");
            }
            Err(e) => {
                panic!("Failed to lookup invoice: {:?}", e);
            }
        }
    }
}
