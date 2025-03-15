#[cfg(feature = "napi_rs")]
use napi_derive::napi;

use crate::{ApiError, ListTransactionsParams, PayInvoiceResponse};
use crate::types::NodeInfo;

#[cfg_attr(feature = "napi_rs", napi(object))]
pub struct ClnConfig {
    pub url: String,
    pub rune: String,
}

#[cfg_attr(feature = "napi_rs", napi(object))]
pub struct ClnNode {
    pub url: String,
    pub rune: String,
}

impl ClnNode {
    pub fn new(config: ClnConfig) -> Self {
        Self {
            url: config.url,
            rune: config.rune,
        }
    }

    pub async fn get_info(&self) -> Result<NodeInfo, ApiError> {
        crate::cln::api::get_info(self.url.clone(), self.rune.clone())
    }

    pub async fn pay_offer(
        &self,
        offer: String,
        amount_msats: i64,
        payer_note: Option<String>,
    ) -> Result<PayInvoiceResponse, ApiError> {
        crate::cln::api::pay_offer(
            self.url.clone(),
            self.rune.clone(),
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
        let transactions = crate::cln::api::lookup_invoice(
            self.url.clone(),
            self.rune.clone(),
            Some(payment_hash),
            None,
            None,
        )
        .unwrap();
        transactions.into_iter().next().ok_or(ApiError::Json {
            reason: "No transactions found".to_string(),
        })
    }

    pub async fn list_transactions(
        &self,
        params: ListTransactionsParams,
    ) -> Result<Vec<crate::Transaction>, ApiError> {
        crate::cln::api::lookup_invoice(
            self.url.clone(),
            self.rune.clone(),
            None,
            Some(params.from),
            Some(params.limit),
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
        static ref TEST_PAYMENT_HASH: String = {
            dotenv().ok();
            env::var("CLN_TEST_PAYMENT_HASH").expect("CLN_TEST_PAYMENT_HASH must be set")
        };
        static ref NODE: ClnNode = {
            ClnNode::new(ClnConfig {
                url: URL.clone(),
                rune: RUNE.clone(),
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
    async fn test_pay_offer() {
        match NODE
            .pay_offer(
                PHOENIX_MOBILE_OFFER.to_string(),
                3000,
                Some("from LNI test".to_string()),
            )
            .await
        {
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
    async fn test_lookup_invoice() {
        match NODE.lookup_invoice(TEST_PAYMENT_HASH.to_string()).await {
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
    async fn test_list_transactions() {
        let params = ListTransactionsParams {
            from: 0,
            limit: 10,
            payment_hash: None,
        };
        match NODE.list_transactions(params).await {
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
}
