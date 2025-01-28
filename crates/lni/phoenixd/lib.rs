#[cfg(feature = "napi_rs")]
use napi_derive::napi;

use crate::{phoenixd::api::*, InvoiceType, Transaction};
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
    pub amountSat: i64,
    pub paymentHash: String,
    pub serialized: String,
}

#[cfg_attr(feature = "napi_rs", napi(object))]
#[derive(Debug, Serialize, Deserialize)]
pub struct PhoenixdMakeInvoiceParams {
    pub invoice_type: InvoiceType,
    pub amount: i64,
    pub description: Option<String>,
    pub description_hash: Option<String>,
    pub expiry: Option<i64>,
}

impl PhoenixdNode {
    pub fn new(config: PhoenixdConfig) -> Self {
        Self {
            url: config.url,
            password: config.password,
        }
    }

    pub async fn get_info(&self) -> crate::Result<crate::NodeInfo> {
        crate::phoenixd::api::get_info(self.url.clone(), self.password.clone())
    }

    pub async fn make_invoice(&self, params: PhoenixdMakeInvoiceParams) -> crate::Result<Transaction> {
        make_invoice(
            self.url.clone(),
            self.password.clone(),
            params.invoice_type,
            params.amount,
            params.description,
            params.description_hash,
            params.expiry,
        )
        .await
    }

    // pub async fn create_bolt_11_invoice(&self) -> crate::Result<Bolt11Resp> {
    //     create_bolt_11_invoice(self.url.clone(), self.password.clone()).await
    // }
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
        let amount = 1000;
        let description = "Test invoice".to_string();
        let description_hash = "".to_string();
        let expiry = 3600;
        let params = PhoenixdMakeInvoiceParams {
            invoice_type: InvoiceType::Bolt11,
            amount,
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
}
