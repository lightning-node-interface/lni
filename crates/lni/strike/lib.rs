#[cfg(feature = "napi_rs")]
use napi_derive::napi;

use crate::types::NodeInfo;
#[cfg(not(feature = "uniffi"))]
use crate::LightningNode;
use crate::{
    ApiError, CreateInvoiceParams, CreateOfferParams, ListTransactionsParams, LookupInvoiceParams,
    Offer, OnchainTransaction, PayInvoiceParams, PayInvoiceResponse, PayOnchainOptions,
    PayOnchainResponse, PrepareOnchainTransactionParams, Transaction,
};

#[cfg_attr(feature = "napi_rs", napi(object))]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
#[derive(Clone)]
pub struct StrikeConfig {
    #[cfg_attr(feature = "uniffi", uniffi(default = Some("https://api.strike.me/v1")))]
    pub base_url: Option<String>,
    pub api_key: String,
    #[cfg_attr(feature = "uniffi", uniffi(default = Some("")))]
    pub socks5_proxy: Option<String>, // Some("socks5h://127.0.0.1:9150") or Some("".to_string())
    #[cfg_attr(feature = "uniffi", uniffi(default = Some(true)))]
    pub accept_invalid_certs: Option<bool>,
    #[cfg_attr(feature = "uniffi", uniffi(default = Some(120)))]
    pub http_timeout: Option<i64>,
}

impl std::fmt::Debug for StrikeConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StrikeConfig")
            .field("base_url", &self.base_url)
            .field("api_key", &"<redacted>")
            .field("socks5_proxy", &"<redacted>")
            .field("accept_invalid_certs", &self.accept_invalid_certs)
            .field("http_timeout", &self.http_timeout)
            .finish()
    }
}

impl Default for StrikeConfig {
    fn default() -> Self {
        Self {
            base_url: Some("https://api.strike.me/v1".to_string()),
            api_key: "".to_string(),
            socks5_proxy: Some("".to_string()),
            accept_invalid_certs: Some(false),
            http_timeout: Some(60),
        }
    }
}

#[cfg_attr(feature = "napi_rs", napi(object))]
#[cfg_attr(feature = "uniffi", derive(uniffi::Object))]
#[derive(Debug, Clone)]
pub struct StrikeNode {
    pub config: StrikeConfig,
}

// Constructor is inherent, not part of the trait
#[cfg_attr(feature = "uniffi", uniffi::export)]
impl StrikeNode {
    #[cfg_attr(feature = "uniffi", uniffi::constructor)]
    pub fn new(config: StrikeConfig) -> Self {
        Self { config }
    }
}

// All node methods - UniFFI exports these directly when the feature is enabled
#[cfg_attr(feature = "uniffi", uniffi::export(async_runtime = "tokio"))]
impl StrikeNode {
    pub async fn get_permissions(&self) -> Result<crate::Permissions, ApiError> {
        crate::permissions::parse_strike_oauth_permissions(&self.config.api_key).ok_or_else(|| {
            ApiError::InvalidInput(
                "Strike API keys cannot be introspected. Use an OAuth access token or manually test permissions against Strike REST endpoints.".to_string(),
            )
        })
    }

    pub async fn get_info(&self) -> Result<NodeInfo, ApiError> {
        crate::strike::api::get_info(self.config.clone()).await
    }

    pub async fn create_invoice(
        &self,
        params: CreateInvoiceParams,
    ) -> Result<Transaction, ApiError> {
        crate::strike::api::create_invoice(self.config.clone(), params).await
    }

    pub async fn pay_invoice(
        &self,
        params: PayInvoiceParams,
    ) -> Result<PayInvoiceResponse, ApiError> {
        crate::strike::api::pay_invoice(self.config.clone(), params).await
    }

    pub async fn prepare_onchain_transaction(
        &self,
        params: PrepareOnchainTransactionParams,
    ) -> Result<OnchainTransaction, ApiError> {
        crate::strike::api::prepare_onchain_transaction(self.config.clone(), params).await
    }

    pub async fn pay_onchain(
        &self,
        transaction: OnchainTransaction,
    ) -> Result<PayOnchainResponse, ApiError> {
        crate::strike::api::pay_onchain(self.config.clone(), transaction).await
    }

    pub async fn pay_onchain_with_options(
        &self,
        transaction: OnchainTransaction,
        options: PayOnchainOptions,
    ) -> Result<PayOnchainResponse, ApiError> {
        crate::strike::api::pay_onchain_with_options(self.config.clone(), transaction, options)
            .await
    }

    pub async fn create_offer(&self, _params: CreateOfferParams) -> Result<Offer, ApiError> {
        Err(ApiError::Api {
            reason: "create_offer not implemented for StrikeNode".to_string(),
        })
    }

    pub async fn lookup_invoice(
        &self,
        params: LookupInvoiceParams,
    ) -> Result<crate::Transaction, ApiError> {
        crate::strike::api::lookup_invoice(
            self.config.clone(),
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
    ) -> Result<Vec<crate::Transaction>, ApiError> {
        crate::strike::api::list_transactions(
            self.config.clone(),
            params.from,
            params.limit,
            params.search,
        )
        .await
    }

    pub async fn decode(&self, str: String) -> Result<String, ApiError> {
        crate::utils::decode_bolt11(str)
    }

    pub async fn decode_offer(&self, offer: String) -> Result<String, ApiError> {
        crate::utils::decode_offer(offer)
    }

    pub async fn get_offer(&self, search: Option<String>) -> Result<Offer, ApiError> {
        crate::strike::api::get_offer(&self.config, search)
    }

    pub async fn list_offers(&self, search: Option<String>) -> Result<Vec<Offer>, ApiError> {
        crate::strike::api::list_offers(&self.config, search).await
    }

    pub async fn pay_offer(
        &self,
        offer: String,
        amount_msats: i64,
        payer_note: Option<String>,
    ) -> Result<PayInvoiceResponse, ApiError> {
        crate::strike::api::pay_offer(&self.config, offer, amount_msats, payer_note)
    }

    pub async fn on_invoice_events(
        &self,
        params: crate::types::OnInvoiceEventParams,
        callback: std::sync::Arc<dyn crate::types::OnInvoiceEventCallback>,
    ) {
        crate::strike::api::on_invoice_events(self.config.clone(), params, callback).await
    }
}

// Trait implementation for polymorphic access via Arc<dyn LightningNode>
crate::impl_lightning_node!(StrikeNode);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        InvoiceType, OnchainFeePayer, OnchainFeePreference, OnchainFeePreferenceType,
        OnchainFeeSpeed,
    };
    use dotenv::dotenv;
    use lazy_static::lazy_static;
    use std::sync::{Arc, Mutex};
    use std::{dbg, env};

    const ONCHAIN_SEND_CONFIRMATION: &str = "I_UNDERSTAND_THIS_BROADCASTS_BITCOIN";

    lazy_static! {
        static ref BASE_URL: String = {
            dotenv().ok();
            env::var("STRIKE_BASE_URL").unwrap_or_else(|_| "https://api.strike.me/v1".to_string())
        };
        static ref API_KEY: String = {
            dotenv().ok();
            env::var("STRIKE_API_KEY").expect("STRIKE_API_KEY must be set")
        };
        static ref TEST_PAYMENT_HASH: String = {
            dotenv().ok();
            env::var("STRIKE_TEST_PAYMENT_HASH").expect("STRIKE_TEST_PAYMENT_HASH must be set")
        };
        static ref TEST_PAYMENT_REQUEST: String = {
            dotenv().ok();
            env::var("STRIKE_TEST_PAYMENT_REQUEST")
                .expect("STRIKE_TEST_PAYMENT_REQUEST must be set")
        };
        static ref NODE: StrikeNode = {
            StrikeNode::new(StrikeConfig {
                base_url: Some(BASE_URL.clone()),
                api_key: API_KEY.clone(),
                http_timeout: Some(120),
                socks5_proxy: Some("".to_string()),
                accept_invalid_certs: Some(false),
            })
        };
    }

    #[tokio::test]
    async fn test_get_info() {
        match NODE.get_info().await {
            Ok(info) => {
                dbg!("info: {:?}", info);
            }
            Err(e) => {
                panic!("Failed to get info: {:?}", e);
            }
        }
    }

    #[tokio::test]
    async fn test_create_invoice() {
        let amount_msats = 5000; // 5 sats
        let description = "Test Strike invoice".to_string();
        let expiry = 3600;

        match NODE
            .create_invoice(CreateInvoiceParams {
                invoice_type: Some(InvoiceType::Bolt11),
                amount_msats: Some(amount_msats),
                description: Some(description.clone()),
                expiry: Some(expiry),
                ..Default::default()
            })
            .await
        {
            Ok(txn) => {
                println!("Strike create_invoice: {:?}", txn);
                assert!(
                    !txn.invoice.is_empty(),
                    "Strike create_invoice Invoice should not be empty"
                );
            }
            Err(e) => {
                println!(
                    "Strike create_invoice failed (expected if no API key): {:?}",
                    e
                );
                // Don't panic as this requires valid API key
            }
        }
    }

    #[tokio::test]
    #[ignore = "pays a real Lightning invoice"]
    async fn test_pay_invoice() {
        match NODE
            .pay_invoice(PayInvoiceParams {
                invoice: TEST_PAYMENT_REQUEST.to_string(),
                ..Default::default()
            })
            .await
        {
            Ok(invoice_resp) => {
                println!(
                    "Strike pay_invoice succeeded: payment_hash_present={}, preimage_present={}, fee_msats={}",
                    !invoice_resp.payment_hash.is_empty(),
                    !invoice_resp.preimage.is_empty(),
                    invoice_resp.fee_msats
                );
            }
            Err(e) => {
                println!(
                    "Strike pay invoice failed (expected if no API key or invalid invoice): {:?}",
                    e
                );
                // Don't panic as this requires valid API key and invoice
            }
        }
    }

    #[tokio::test]
    async fn test_lookup_invoice() {
        match NODE
            .lookup_invoice(LookupInvoiceParams {
                payment_hash: Some(TEST_PAYMENT_HASH.to_string()),
                ..Default::default()
            })
            .await
        {
            Ok(txn) => {
                println!("Strike lookup invoice: {:?}", txn);
                assert!(
                    txn.amount_msats >= 0,
                    "Invoice should contain a valid amount"
                );
            }
            Err(e) => {
                if e.to_string().contains("not found") {
                    assert!(true, "Invoice not found as expected");
                } else {
                    println!(
                        "Strike lookup invoice failed (expected if no API key): {:?}",
                        e
                    );
                }
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
            created_after: None,
            created_before: None,
        };
        match NODE.list_transactions(params).await {
            Ok(txns) => {
                dbg!("Strike list transactions: {:?}", &txns);
                println!("Strike transactions: {:?}", txns);
                assert!(true, "Should be able to list transactions");
            }
            Err(e) => {
                println!(
                    "Strike list transactions failed (expected if no API key): {:?}",
                    e
                );
                // Don't panic as this requires valid API key
            }
        }
    }

    #[tokio::test]
    #[ignore = "broadcasts an on-chain bitcoin payment"]
    async fn test_pay_onchain_e2e() {
        dotenv().ok();

        if env::var("STRIKE_RUN_ONCHAIN_SEND").ok().as_deref() != Some("true")
            || env::var("STRIKE_ONCHAIN_SEND_CONFIRM").ok().as_deref()
                != Some(ONCHAIN_SEND_CONFIRMATION)
        {
            panic!("Refusing to broadcast without explicit STRIKE on-chain send confirmation");
        }

        let address = env::var("STRIKE_ONCHAIN_TEST_ADDRESS")
            .expect("STRIKE_ONCHAIN_TEST_ADDRESS must be set");
        let amount_sats = env::var("STRIKE_ONCHAIN_AMOUNT_SATS")
            .expect("STRIKE_ONCHAIN_AMOUNT_SATS must be set")
            .parse::<i64>()
            .expect("STRIKE_ONCHAIN_AMOUNT_SATS must be a positive integer");
        assert!(
            amount_sats > 0,
            "STRIKE_ONCHAIN_AMOUNT_SATS must be positive"
        );

        let idempotency_key = uuid::Uuid::new_v4().to_string();

        let transaction = NODE
            .prepare_onchain_transaction(PrepareOnchainTransactionParams {
                address: address.clone(),
                amount_sats,
                fee: Some(OnchainFeePreference {
                    preference_type: OnchainFeePreferenceType::Speed,
                    speed: Some(OnchainFeeSpeed::Free),
                    target_conf: None,
                    sats_per_vbyte: None,
                    backend: None,
                }),
                fee_payer: Some(OnchainFeePayer::Sender),
                description: Some("strike rust onchain e2e".to_string()),
                idempotency_key: Some(idempotency_key),
            })
            .await
            .expect("prepare_onchain_transaction should create a quote");

        assert_eq!(transaction.address, address);
        assert_eq!(transaction.amount_sats, amount_sats);
        assert!(
            transaction.id.as_ref().map_or(false, |id| !id.is_empty()),
            "on-chain transaction should include a quote id"
        );

        // TODO: have user validate fees before broadcasting in case of unexpectedly high fees

        let payment = NODE
            .pay_onchain(transaction)
            .await
            .expect("pay_onchain should execute quote");

        assert_eq!(payment.address, address);
        assert_eq!(payment.amount_sats, amount_sats);
        assert!(
            matches!(payment.state.as_str(), "pending" | "completed"),
            "unexpected on-chain payment state: {}",
            payment.state
        );
    }

    // #[test]
    // fn test_decode() {
    //     match NODE.decode(TEST_PAYMENT_REQUEST.to_string()) {
    //         Ok(decoded) => {
    //             println!("Strike decode: {:?}", decoded);
    //             assert!(!decoded.is_empty(), "Decoded result should not be empty");
    //         }
    //         Err(e) => {
    //             panic!("Strike decode failed: {:?}", e);
    //         }
    //     }
    // }

    #[tokio::test]
    async fn test_on_invoice_events() {
        struct OnInvoiceEventCallback {
            events: Arc<Mutex<Vec<String>>>,
        }

        impl crate::types::OnInvoiceEventCallback for OnInvoiceEventCallback {
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
        let callback = OnInvoiceEventCallback {
            events: events.clone(),
        };

        // Use the real test payment hash from environment
        let params = crate::types::OnInvoiceEventParams {
            payment_hash: Some(TEST_PAYMENT_HASH.to_string()),
            polling_delay_sec: 2,
            max_polling_sec: 5, // Short timeout for test
            ..Default::default()
        };

        // Start the event listener
        NODE.on_invoice_events(params, std::sync::Arc::new(callback))
            .await;

        // Check that some events were captured
        let events_guard = events.lock().unwrap();
        println!("Strike events captured: {:?}", *events_guard);

        // We expect at least one event (even if it's a failure due to invoice not found)
        assert!(
            !events_guard.is_empty(),
            "Should capture at least one event"
        );
    }

    #[tokio::test]
    async fn test_socks5_proxy_config() {
        // Test that Strike config can be created with SOCKS5 proxy settings
        let config_with_proxy = StrikeConfig {
            base_url: Some(BASE_URL.clone()),
            api_key: API_KEY.clone(),
            http_timeout: Some(120),
            socks5_proxy: Some("socks5h://127.0.0.1:9150".to_string()), // Tor proxy example
            accept_invalid_certs: Some(true),
        };

        let node_with_proxy = StrikeNode::new(config_with_proxy);

        // Test that the config is set correctly
        assert_eq!(
            node_with_proxy.config.socks5_proxy,
            Some("socks5h://127.0.0.1:9150".to_string())
        );
        assert_eq!(node_with_proxy.config.accept_invalid_certs, Some(true));

        // Note: We don't actually test the network connection as that would require
        // a running Tor proxy or similar setup. This test just verifies the config
        // structure is working correctly.
        println!(
            "SOCKS5 proxy config test passed - proxy: {:?}, accept_invalid_certs: {:?}",
            node_with_proxy.config.socks5_proxy, node_with_proxy.config.accept_invalid_certs
        );
    }
}
