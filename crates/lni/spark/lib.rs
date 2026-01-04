#[cfg(feature = "napi_rs")]
use napi_derive::napi;

use std::sync::Arc;

use breez_sdk_spark::{
    connect, default_config, BreezSdk, ConnectRequest, Network, Seed,
};

use crate::types::NodeInfo;
use crate::{
    ApiError, CreateInvoiceParams, CreateOfferParams, LightningNode, ListTransactionsParams,
    LookupInvoiceParams, Offer, PayInvoiceParams, PayInvoiceResponse, Transaction,
};

#[cfg_attr(feature = "napi_rs", napi(object))]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
#[derive(Debug, Clone)]
pub struct SparkConfig {
    /// 12 or 24 word mnemonic phrase
    pub mnemonic: String,
    /// Optional passphrase for the mnemonic
    #[cfg_attr(feature = "uniffi", uniffi(default = None))]
    pub passphrase: Option<String>,
    /// Breez API key (required for mainnet)
    #[cfg_attr(feature = "uniffi", uniffi(default = None))]
    pub api_key: Option<String>,
    /// Storage directory path for wallet data
    pub storage_dir: String,
    /// Network: "mainnet" or "regtest"
    #[cfg_attr(feature = "uniffi", uniffi(default = Some("mainnet")))]
    pub network: Option<String>,
}

impl Default for SparkConfig {
    fn default() -> Self {
        Self {
            mnemonic: "".to_string(),
            passphrase: None,
            api_key: None,
            storage_dir: "./spark_data".to_string(),
            network: Some("mainnet".to_string()),
        }
    }
}

impl SparkConfig {
    fn get_network(&self) -> Network {
        match self.network.as_deref() {
            Some("regtest") => Network::Regtest,
            _ => Network::Mainnet,
        }
    }
}

#[cfg_attr(feature = "napi_rs", napi(object))]
#[cfg_attr(feature = "uniffi", derive(uniffi::Object))]
pub struct SparkNode {
    pub config: SparkConfig,
    sdk: Arc<BreezSdk>,
}

// Constructor is inherent, not part of the trait
#[cfg_attr(feature = "uniffi", uniffi::export(async_runtime = "tokio"))]
impl SparkNode {
    /// Create a new SparkNode and connect to the Spark network
    #[cfg_attr(feature = "uniffi", uniffi::constructor)]
    pub async fn new(config: SparkConfig) -> Result<Self, ApiError> {
        let network = config.get_network();
        let mut sdk_config = default_config(network);
        sdk_config.api_key = config.api_key.clone();

        let seed = Seed::Mnemonic {
            mnemonic: config.mnemonic.clone(),
            passphrase: config.passphrase.clone(),
        };

        let sdk = connect(ConnectRequest {
            config: sdk_config,
            seed,
            storage_dir: config.storage_dir.clone(),
        })
        .await
        .map_err(|e| ApiError::Api {
            reason: format!("Failed to connect to Spark: {}", e),
        })?;

        Ok(Self {
            config,
            sdk: Arc::new(sdk),
        })
    }

    /// Disconnect from the Spark network
    pub async fn disconnect(&self) -> Result<(), ApiError> {
        self.sdk.disconnect().await.map_err(|e| ApiError::Api {
            reason: e.to_string(),
        })
    }

    /// Get the Spark address for receiving payments
    pub async fn get_spark_address(&self) -> Result<String, ApiError> {
        use breez_sdk_spark::{ReceivePaymentMethod, ReceivePaymentRequest};

        let response = self
            .sdk
            .receive_payment(ReceivePaymentRequest {
                payment_method: ReceivePaymentMethod::SparkAddress,
            })
            .await
            .map_err(|e| ApiError::Api {
                reason: e.to_string(),
            })?;

        Ok(response.payment_request)
    }

    /// Get a Bitcoin address for on-chain deposits
    pub async fn get_deposit_address(&self) -> Result<String, ApiError> {
        use breez_sdk_spark::{ReceivePaymentMethod, ReceivePaymentRequest};

        let response = self
            .sdk
            .receive_payment(ReceivePaymentRequest {
                payment_method: ReceivePaymentMethod::BitcoinAddress,
            })
            .await
            .map_err(|e| ApiError::Api {
                reason: e.to_string(),
            })?;

        Ok(response.payment_request)
    }
}

#[cfg_attr(feature = "uniffi", uniffi::export(async_runtime = "tokio"))]
#[async_trait::async_trait]
impl LightningNode for SparkNode {
    async fn get_info(&self) -> Result<NodeInfo, ApiError> {
        crate::spark::api::get_info(self.sdk.clone()).await
    }

    async fn create_invoice(&self, params: CreateInvoiceParams) -> Result<Transaction, ApiError> {
        crate::spark::api::create_invoice(self.sdk.clone(), params).await
    }

    async fn pay_invoice(&self, params: PayInvoiceParams) -> Result<PayInvoiceResponse, ApiError> {
        crate::spark::api::pay_invoice(self.sdk.clone(), params).await
    }

    async fn create_offer(&self, _params: CreateOfferParams) -> Result<Offer, ApiError> {
        Err(ApiError::Api {
            reason: "create_offer not yet implemented for SparkNode".to_string(),
        })
    }

    async fn lookup_invoice(&self, params: LookupInvoiceParams) -> Result<Transaction, ApiError> {
        crate::spark::api::lookup_invoice(
            self.sdk.clone(),
            params.payment_hash,
            None,
            None,
            params.search,
        )
        .await
    }

    async fn list_transactions(
        &self,
        params: ListTransactionsParams,
    ) -> Result<Vec<Transaction>, ApiError> {
        crate::spark::api::list_transactions(
            self.sdk.clone(),
            params.from,
            params.limit,
            params.search,
        )
        .await
    }

    async fn decode(&self, str: String) -> Result<String, ApiError> {
        crate::spark::api::decode(&self.config, str)
    }

    async fn on_invoice_events(
        &self,
        params: crate::types::OnInvoiceEventParams,
        callback: Box<dyn crate::types::OnInvoiceEventCallback>,
    ) {
        crate::spark::api::on_invoice_events(self.sdk.clone(), params, callback).await
    }

    async fn get_offer(&self, search: Option<String>) -> Result<Offer, ApiError> {
        crate::spark::api::get_offer(&self.config, search)
    }

    async fn list_offers(&self, search: Option<String>) -> Result<Vec<Offer>, ApiError> {
        crate::spark::api::list_offers(&self.config, search)
    }

    async fn pay_offer(
        &self,
        offer: String,
        amount_msats: i64,
        payer_note: Option<String>,
    ) -> Result<PayInvoiceResponse, ApiError> {
        crate::spark::api::pay_offer(&self.config, offer, amount_msats, payer_note)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dotenv::dotenv;
    use lazy_static::lazy_static;
    use std::env;

    lazy_static! {
        static ref MNEMONIC: String = {
            dotenv().ok();
            env::var("SPARK_MNEMONIC").unwrap_or_else(|_| {
                "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about".to_string()
            })
        };
        static ref API_KEY: String = {
            dotenv().ok();
            env::var("SPARK_API_KEY").unwrap_or_default()
        };
        static ref STORAGE_DIR: String = {
            dotenv().ok();
            env::var("SPARK_STORAGE_DIR").unwrap_or_else(|_| "/tmp/spark_test".to_string())
        };
        static ref TEST_PAYMENT_HASH: String = {
            dotenv().ok();
            env::var("SPARK_TEST_PAYMENT_HASH").unwrap_or_default()
        };
    }

    // Note: These tests require a valid Spark configuration to run
    // They are skipped if the environment variables are not set

    #[tokio::test]
    async fn test_spark_config_default() {
        let config = SparkConfig::default();
        assert_eq!(config.network, Some("mainnet".to_string()));
        assert!(config.mnemonic.is_empty());
    }

    #[tokio::test]
    async fn test_spark_network_parsing() {
        let config = SparkConfig {
            network: Some("regtest".to_string()),
            ..Default::default()
        };
        assert!(matches!(config.get_network(), Network::Regtest));

        let config = SparkConfig {
            network: Some("mainnet".to_string()),
            ..Default::default()
        };
        assert!(matches!(config.get_network(), Network::Mainnet));
    }

    // Integration tests - require valid credentials
    // Uncomment and set environment variables to run

    // #[tokio::test]
    // async fn test_get_info() {
    //     if MNEMONIC.is_empty() || API_KEY.is_empty() {
    //         println!("Skipping test: SPARK_MNEMONIC or SPARK_API_KEY not set");
    //         return;
    //     }
    //
    //     let config = SparkConfig {
    //         mnemonic: MNEMONIC.clone(),
    //         api_key: Some(API_KEY.clone()),
    //         storage_dir: STORAGE_DIR.clone(),
    //         network: Some("mainnet".to_string()),
    //         passphrase: None,
    //     };
    //
    //     match SparkNode::new(config).await {
    //         Ok(node) => {
    //             match node.get_info().await {
    //                 Ok(info) => {
    //                     println!("Spark node info: {:?}", info);
    //                     assert_eq!(info.alias, "Spark Node");
    //                 }
    //                 Err(e) => {
    //                     println!("Failed to get info: {:?}", e);
    //                 }
    //             }
    //             let _ = node.disconnect().await;
    //         }
    //         Err(e) => {
    //             println!("Failed to connect: {:?}", e);
    //         }
    //     }
    // }
}
