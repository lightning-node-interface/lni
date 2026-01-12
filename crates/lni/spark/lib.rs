#[cfg(feature = "napi_rs")]
use napi_derive::napi;

use std::sync::Arc;

use breez_sdk_spark::{
    connect, default_config, BreezSdk, ConnectRequest, Network, Seed,
};

use crate::types::NodeInfo;
use crate::{
    ApiError, CreateInvoiceParams, CreateOfferParams, ListTransactionsParams,
    LookupInvoiceParams, Offer, PayInvoiceParams, PayInvoiceResponse, Transaction,
};
#[cfg(not(feature = "uniffi"))]
use crate::LightningNode;

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

// Note: SparkNode cannot use napi(object) because BreezSdk has private fields
// #[cfg_attr(feature = "napi_rs", napi(object))]
#[cfg_attr(feature = "uniffi", derive(uniffi::Object))]
#[derive(Clone)]
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

// Internal Rust API only - not exported via uniffi
impl SparkNode {
    /// Get the internal SDK Arc for low-level operations
    pub fn get_sdk(&self) -> Arc<BreezSdk> {
        self.sdk.clone()
    }
}

// All node methods - UniFFI exports these directly when the feature is enabled
#[cfg_attr(feature = "uniffi", uniffi::export(async_runtime = "tokio"))]
impl SparkNode {
    pub async fn get_info(&self) -> Result<NodeInfo, ApiError> {
        let network = self.config.network.as_deref().unwrap_or("mainnet");
        crate::spark::api::get_info(self.sdk.clone(), network).await
    }

    pub async fn create_invoice(&self, params: CreateInvoiceParams) -> Result<Transaction, ApiError> {
        crate::spark::api::create_invoice(self.sdk.clone(), params).await
    }

    pub async fn pay_invoice(&self, params: PayInvoiceParams) -> Result<PayInvoiceResponse, ApiError> {
        crate::spark::api::pay_invoice(self.sdk.clone(), params).await
    }

    pub async fn create_offer(&self, _params: CreateOfferParams) -> Result<Offer, ApiError> {
        Err(ApiError::Api {
            reason: "create_offer not yet implemented for SparkNode".to_string(),
        })
    }

    pub async fn lookup_invoice(&self, params: LookupInvoiceParams) -> Result<Transaction, ApiError> {
        crate::spark::api::lookup_invoice(
            self.sdk.clone(),
            params.payment_hash,
            None,
            None,
            params.search,
        )
        .await
    }

    pub async fn list_transactions(
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

    pub async fn decode(&self, str: String) -> Result<String, ApiError> {
        crate::spark::api::decode(self.sdk.clone(), str).await
    }

    pub async fn on_invoice_events(
        &self,
        params: crate::types::OnInvoiceEventParams,
        callback: std::sync::Arc<dyn crate::types::OnInvoiceEventCallback>,
    ) {
        crate::spark::api::on_invoice_events(self.sdk.clone(), params, callback).await
    }

    pub async fn get_offer(&self, search: Option<String>) -> Result<Offer, ApiError> {
        crate::spark::api::get_offer(search)
    }

    pub async fn list_offers(&self, search: Option<String>) -> Result<Vec<Offer>, ApiError> {
        crate::spark::api::list_offers(search)
    }

    pub async fn pay_offer(
        &self,
        offer: String,
        amount_msats: i64,
        payer_note: Option<String>,
    ) -> Result<PayInvoiceResponse, ApiError> {
        crate::spark::api::pay_offer(offer, amount_msats, payer_note)
    }
}

// Trait implementation for polymorphic access via Arc<dyn LightningNode>
crate::impl_lightning_node!(SparkNode);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CreateInvoiceParams, InvoiceType, ListTransactionsParams, LookupInvoiceParams, PayInvoiceParams};
    use dotenv::dotenv;
    use lazy_static::lazy_static;
    use std::env;
    use std::sync::Once;

    static INIT: Once = Once::new();

    lazy_static! {
        static ref MNEMONIC: String = {
            dotenv().ok();
            env::var("SPARK_MNEMONIC").unwrap_or_default()
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
        static ref TEST_RECEIVER_OFFER: String = {
            dotenv().ok();
            env::var("TEST_RECEIVER_OFFER").unwrap_or_default()
        };
    }

    fn should_skip() -> bool {
        MNEMONIC.is_empty() || API_KEY.is_empty()
    }

    async fn get_node() -> Result<SparkNode, ApiError> {
        let config = SparkConfig {
            mnemonic: MNEMONIC.clone(),
            api_key: Some(API_KEY.clone()),
            storage_dir: STORAGE_DIR.clone(),
            network: Some("mainnet".to_string()),
            passphrase: None,
        };
        SparkNode::new(config).await
    }

    // Unit tests - no credentials required

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
    // Set SPARK_MNEMONIC and SPARK_API_KEY environment variables to run
    #[tokio::test]
    async fn test_get_info() {
        if should_skip() {
            println!("Skipping test: SPARK_MNEMONIC or SPARK_API_KEY not set");
            return;
        }

        let node = get_node().await.expect("Failed to connect");
        match node.get_info().await {
            Ok(info) => {
                println!("info: {:?}", info);
            }
            Err(e) => {
                panic!("Failed to get info: {:?}", e);
            }
        }
        let _ = node.disconnect().await;
    }

    #[tokio::test]
    async fn test_create_invoice() {
        if should_skip() {
            println!("Skipping test: SPARK_MNEMONIC or SPARK_API_KEY not set");
            return;
        }

        let node = get_node().await.expect("Failed to connect");
        let params = CreateInvoiceParams {
            invoice_type: Some(InvoiceType::Bolt11),
            amount_msats: Some(1000),
            offer: None,
            description: Some("Test invoice".to_string()),
            description_hash: None,
            expiry: Some(3600),
            ..Default::default()
        };

        match node.create_invoice(params).await {
            Ok(txn) => {
                println!("txn: {:?}", txn);
                assert!(!txn.invoice.is_empty(), "Invoice should not be empty");
            }
            Err(e) => {
                panic!("Failed to make invoice: {:?}", e);
            }
        }
        let _ = node.disconnect().await;
    }

    #[tokio::test]
    async fn test_pay_invoice() {
        if should_skip() {
            println!("Skipping test: SPARK_MNEMONIC or SPARK_API_KEY not set");
            return;
        }

        let node = get_node().await.expect("Failed to connect");
        // Note: This test requires a valid invoice to pay
        // For now we'll just test that the function exists and handles errors
        match node.pay_invoice(PayInvoiceParams {
            invoice: "lnbc1***".to_string(), // Invalid invoice for testing
            ..Default::default()
        }).await {
            Ok(txn) => {
                println!("txn: {:?}", txn);
                assert!(!txn.payment_hash.is_empty(), "Payment hash should not be empty");
            }
            Err(e) => {
                println!("Expected error for invalid invoice: {:?}", e);
                // This is expected to fail with an invalid invoice
            }
        }
        let _ = node.disconnect().await;
    }

    #[tokio::test]
    async fn test_list_transactions() {
        if should_skip() {
            println!("Skipping test: SPARK_MNEMONIC or SPARK_API_KEY not set");
            return;
        }

        let node = get_node().await.expect("Failed to connect");
        let params = ListTransactionsParams {
            from: 0,
            limit: 100,
            payment_hash: None,
            search: None,
        };

        match node.list_transactions(params).await {
            Ok(txns) => {
                println!("transactions: {:?}", txns);
                assert!(true, "Successfully fetched transactions");
            }
            Err(e) => {
                panic!("Failed to list transactions: {:?}", e);
            }
        }
        let _ = node.disconnect().await;
    }

    #[tokio::test]
    async fn test_lookup_invoice() {
        if should_skip() || TEST_PAYMENT_HASH.is_empty() {
            println!("Skipping test: credentials or SPARK_TEST_PAYMENT_HASH not set");
            return;
        }

        let node = get_node().await.expect("Failed to connect");
        match node.lookup_invoice(LookupInvoiceParams {
            payment_hash: Some(TEST_PAYMENT_HASH.to_string()),
            ..Default::default()
        }).await {
            Ok(txn) => {
                println!("txn: {:?}", txn);
                assert!(txn.amount_msats > 0, "Invoice should contain an amount");
            }
            Err(e) => {
                panic!("Failed to lookup invoice: {:?}", e);
            }
        }
        let _ = node.disconnect().await;
    }

    #[tokio::test]
    async fn test_get_spark_address() {
        if should_skip() {
            println!("Skipping test: SPARK_MNEMONIC or SPARK_API_KEY not set");
            return;
        }

        let node = get_node().await.expect("Failed to connect");
        match node.get_spark_address().await {
            Ok(address) => {
                println!("Spark address: {}", address);
                assert!(!address.is_empty(), "Spark address should not be empty");
            }
            Err(e) => {
                panic!("Failed to get Spark address: {:?}", e);
            }
        }
        let _ = node.disconnect().await;
    }

    #[tokio::test]
    async fn test_get_deposit_address() {
        if should_skip() {
            println!("Skipping test: SPARK_MNEMONIC or SPARK_API_KEY not set");
            return;
        }

        let node = get_node().await.expect("Failed to connect");
        match node.get_deposit_address().await {
            Ok(address) => {
                println!("Bitcoin deposit address: {}", address);
                assert!(!address.is_empty(), "Deposit address should not be empty");
            }
            Err(e) => {
                panic!("Failed to get deposit address: {:?}", e);
            }
        }
        let _ = node.disconnect().await;
    }

    #[tokio::test]
    async fn test_decode() {
        if should_skip() {
            println!("Skipping test: SPARK_MNEMONIC or SPARK_API_KEY not set");
            return;
        }

        let node = get_node().await.expect("Failed to connect");
        // Test decoding a BOLT11 invoice
        let test_invoice = "lnbc1..."; // You can put a valid invoice here for testing
        match node.decode(test_invoice.to_string()).await {
            Ok(decoded) => {
                println!("Decoded: {}", decoded);
            }
            Err(e) => {
                println!("Decode error (may be expected for invalid input): {:?}", e);
            }
        }
        let _ = node.disconnect().await;
    }

    #[tokio::test]
    async fn test_on_invoice_events() {
        if should_skip() || TEST_PAYMENT_HASH.is_empty() {
            println!("Skipping test: credentials or SPARK_TEST_PAYMENT_HASH not set");
            return;
        }

        struct TestInvoiceEventCallback;
        impl crate::types::OnInvoiceEventCallback for TestInvoiceEventCallback {
            fn success(&self, transaction: Option<Transaction>) {
                println!("success: {:?}", transaction);
            }
            fn pending(&self, transaction: Option<Transaction>) {
                println!("pending: {:?}", transaction);
            }
            fn failure(&self, transaction: Option<Transaction>) {
                println!("failure: {:?}", transaction);
            }
        }

        let node = get_node().await.expect("Failed to connect");
        let params = crate::types::OnInvoiceEventParams {
            search: Some(TEST_PAYMENT_HASH.to_string()),
            polling_delay_sec: 2,
            max_polling_sec: 6,
            ..Default::default()
        };
        let callback = TestInvoiceEventCallback;
        node.on_invoice_events(params, Arc::new(callback)).await;
        let _ = node.disconnect().await;
    }

    #[tokio::test]
    async fn test_create_new_wallet_and_invoice() {
        // Skip if no API key - we still need the API key for mainnet
        if API_KEY.is_empty() {
            println!("Skipping test: SPARK_API_KEY not set");
            return;
        }

        // 1. Generate a fresh mnemonic (creates a brand new wallet)
        let mnemonic_str = crate::generate_mnemonic(Some(12)).expect("Failed to generate mnemonic");
        println!("Generated new mnemonic: {}", mnemonic_str);

        // Verify it's a 12-word mnemonic
        let word_count = mnemonic_str.split_whitespace().count();
        assert_eq!(word_count, 12, "Expected 12-word mnemonic, got {}", word_count);

        // 2. Create a new SparkNode with the fresh mnemonic
        let config = SparkConfig {
            mnemonic: mnemonic_str,
            api_key: Some(API_KEY.clone()),
            storage_dir: format!("{}/new_wallet_{}", STORAGE_DIR.clone(), uuid::Uuid::new_v4()),
            network: Some("mainnet".to_string()),
            passphrase: None,
        };

        let node = SparkNode::new(config).await.expect("Failed to connect with new wallet");

        // 3. Create an invoice with the new wallet
        let invoice_params = CreateInvoiceParams {
            amount_msats: Some(1000), // 1 sat
            description: Some("Test invoice from new wallet".to_string()),
            expiry: Some(3600),
            invoice_type: Some(InvoiceType::Bolt11),
            ..Default::default()
        };

        let invoice = node.create_invoice(invoice_params).await.expect("Failed to create invoice");

        println!("Created invoice: {}", invoice.invoice);
        assert!(!invoice.invoice.is_empty(), "Invoice should not be empty");
        assert!(
            invoice.invoice.starts_with("lnbc") || invoice.invoice.starts_with("lntb"),
            "Invoice should be a valid bolt11 invoice"
        );

        // 4. Clean up
        let _ = node.disconnect().await;
    }
}
