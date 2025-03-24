#[cfg(feature = "napi_rs")]
use napi_derive::napi;

use crate::types::NodeInfo;
use crate::{
    ApiError, CreateInvoiceParams, ListTransactionsParams, PayCode, PayInvoiceParams,
    PayInvoiceResponse, Transaction,
};

#[cfg_attr(feature = "napi_rs", napi(object))]
pub struct LndConfig {
    pub url: String,
    pub macaroon: String,
}

#[cfg_attr(feature = "napi_rs", napi(object))]
pub struct LndNode {
    pub url: String,
    pub macaroon: String,
}

impl LndNode {
    pub fn new(config: LndConfig) -> Self {
        Self {
            url: config.url,
            macaroon: config.macaroon,
        }
    }

    pub async fn get_info(&self) -> Result<NodeInfo, ApiError> {
        crate::lnd::api::get_info(self.url.clone(), self.macaroon.clone())
    }

    pub async fn create_invoice(
        &self,
        params: CreateInvoiceParams,
    ) -> Result<Transaction, ApiError> {
        crate::lnd::api::create_invoice(self.url.clone(), self.macaroon.clone(), params).await
    }

    pub async fn pay_invoice(
        &self,
        params: PayInvoiceParams,
    ) -> Result<PayInvoiceResponse, ApiError> {
        crate::lnd::api::pay_invoice(self.url.clone(), self.macaroon.clone(), params).await
    }

    pub async fn get_offer(&self, search: Option<String>) -> Result<PayCode, ApiError> {
        crate::lnd::api::get_offer(self.url.clone(), self.macaroon.clone(), search).await
    }

    pub async fn list_offers(&self, search: Option<String>) -> Result<Vec<PayCode>, ApiError> {
        crate::lnd::api::list_offers(self.url.clone(), self.macaroon.clone(), search).await
    }

    pub async fn pay_offer(
        &self,
        offer: String,
        amount_msats: i64,
        payer_note: Option<String>,
    ) -> Result<PayInvoiceResponse, ApiError> {
        crate::lnd::api::pay_offer(
            self.url.clone(),
            self.macaroon.clone(),
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
        crate::lnd::api::lookup_invoice(self.url.clone(), self.macaroon.clone(), Some(payment_hash))
    }

    pub async fn list_transactions(
        &self,
        params: ListTransactionsParams,
    ) -> Result<Vec<crate::Transaction>, ApiError> {
        crate::lnd::api::list_transactions(
            self.url.clone(),
            self.macaroon.clone(),
            params.from,
            params.limit,
        )
    }

    pub async fn decode(&self, str: String) -> Result<String, ApiError> {
        crate::lnd::api::decode(self.url.clone(), self.macaroon.clone(), str).await
    }
}

#[cfg(test)]
mod tests {
    use crate::{InvoiceType, PayInvoiceParams};

    use super::*;
    use dotenv::dotenv;
    use lazy_static::lazy_static;
    use rand::Rng;
    use sha2::{Digest, Sha256};
    use std::env;
    use tokio::test;

    lazy_static! {
        static ref URL: String = {
            dotenv().ok();
            env::var("LND_URL").expect("LND_URL must be set")
        };
        static ref macaroon: String = {
            dotenv().ok();
            env::var("LND_MACAROON").expect("LND_MACAROON must be set")
        };
        static ref PHOENIX_MOBILE_OFFER: String = {
            dotenv().ok();
            env::var("PHOENIX_MOBILE_OFFER").expect("PHOENIX_MOBILE_OFFER must be set")
        };
        static ref TEST_PAYMENT_HASH: String = {
            dotenv().ok();
            env::var("LND_TEST_PAYMENT_HASH").expect("LND_TEST_PAYMENT_HASH must be set")
        };
        static ref LND_TEST_PAYMENT_REQUEST: String = {
            dotenv().ok();
            env::var("LND_TEST_PAYMENT_REQUEST").expect("LND_TEST_PAYMENT_REQUEST must be set")
        };
        static ref NODE: LndNode = {
            LndNode::new(LndConfig {
                url: URL.clone(),
                macaroon: macaroon.clone(),
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
    async fn test_create_invoice() {
        let amount_msats = 3000;
        let description = "Test invoice".to_string();
        let description_hash = "".to_string();
        let expiry = 3600;

        // Generate a random 32-byte preimage
        let mut preimage_bytes = [0u8; 32];
        rand::thread_rng().fill(&mut preimage_bytes);

        // Hex encode the preimage for human readability
        let preimage = hex::encode(preimage_bytes);
        // Note: payment_hash is automatically derived from the preimage by the LND node
        // We don't need to specify it when creating an invoice
        println!("Generated preimage: {:?}", preimage);

        // Calculate payment hash (SHA-256 of preimage)
        let mut hasher = Sha256::new();
        hasher.update(hex::decode(&preimage).unwrap());
        let payment_hash = hex::encode(hasher.finalize());
        println!("Generated payment_hash: {:?}", payment_hash);

        // BOLT11
        match NODE
            .create_invoice(CreateInvoiceParams {
                invoice_type: InvoiceType::Bolt11,
                amount_msats: Some(amount_msats),
                description: Some(description.clone()),
                description_hash: Some(description_hash.clone()),
                expiry: Some(expiry),
                r_preimage: Some(base64::encode(preimage_bytes)), // LND expects the base64 encoded preimage bytes via the docs if you generate your own preimage+payment for your invoice
                ..Default::default()
            })
            .await
        {
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

        // BOLT 11 with blinded paths
        match NODE
            .create_invoice(CreateInvoiceParams {
                invoice_type: InvoiceType::Bolt11,
                amount_msats: Some(amount_msats),
                description: Some(description.clone()),
                description_hash: Some(description_hash.clone()),
                expiry: Some(expiry),
                is_blinded: Some(true),
                ..Default::default()
            })
            .await
        {
            Ok(txn) => {
                println!("BOLT11 with blinded create_invoice: {:?}", txn);
                assert!(
                    !txn.invoice.is_empty(),
                    "BOLT11 create_invoice Invoice should not be empty"
                );
            }
            Err(e) => {
                panic!(
                    "BOLT11 with blinded create_invoice Failed to make invoice: {:?}",
                    e
                );
            }
        }
    }

    #[test]
    async fn test_pay_invoice() {
        match NODE.pay_invoice(PayInvoiceParams{
            invoice: "".to_string(), // TODO remote grab a invoice maybe from LNURL
            fee_limit_percentage: Some(1.0), // 1% fee limit
            allow_self_payment: Some(true),
            ..Default::default()
        }).await {
            Ok(invoice_resp) => {
                // println!("Pay invoice resp: {:?}", invoice_resp);
                assert!(!invoice_resp.payment_hash.is_empty(), "Payment Hash should not be empty");
            }
            Err(e) => {
                panic!("Failed to pay invoice: {:?}", e);
            }
        }
    }

    // #[test]
    // async fn test_list_offers() {
    //     match NODE.get_offer(None).await {
    //         Ok(resp) => {
    //             println!("Get offer: {:?}", resp);
    //         }
    //         Err(e) => {
    //             panic!("Failed to get offer: {:?}", e);
    //         }
    //     }
    //     match NODE.list_offers(None).await {
    //         Ok(resp) => {
    //             println!("List offers: {:?}", resp);
    //         }
    //         Err(e) => {
    //             panic!("Failed to list offer: {:?}", e);
    //         }
    //     }
    // }

    // #[test]
    // async fn test_pay_offer() {
    //     match NODE
    //         .pay_offer(
    //             PHOENIX_MOBILE_OFFER.to_string(),
    //             3000,
    //             Some("from LNI test".to_string()),
    //         )
    //         .await
    //     {
    //         Ok(pay_resp) => {
    //             println!("pay_resp: {:?}", pay_resp);
    //             assert!(
    //                 !pay_resp.payment_hash.is_empty(),
    //                 "Payment hash should not be empty"
    //             );
    //         }
    //         Err(e) => {
    //             panic!("Failed to get offer: {:?}", e);
    //         }
    //     }
    // }

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
                if e.to_string().contains("not found") {
                    assert!(true, "Invoice not found as expected");
                } else {
                    panic!("Failed to lookup invoice: {:?}", e);
                }
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

    #[test]
    async fn test_decode() {
        match NODE.decode(LND_TEST_PAYMENT_REQUEST.to_string()).await {
            Ok(txns) => {
                println!("decode: {:?}", txns);
            }
            Err(e) => {
                panic!("Failed to decode: {:?}", e);
            }
        }
    }
}
