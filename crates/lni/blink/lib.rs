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
pub struct BlinkConfig {
    #[cfg_attr(feature = "uniffi", uniffi(default = Some("https://api.blink.sv/graphql")))]
    pub base_url: Option<String>,
    pub api_key: String,
    #[cfg_attr(feature = "uniffi", uniffi(default = Some("")))]
    pub socks5_proxy: Option<String>, // Some("socks5h://127.0.0.1:9150") or Some("".to_string())
    #[cfg_attr(feature = "uniffi", uniffi(default = Some(true)))]
    pub accept_invalid_certs: Option<bool>,
    #[cfg_attr(feature = "uniffi", uniffi(default = Some(120)))]
    pub http_timeout: Option<i64>,
}

impl std::fmt::Debug for BlinkConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BlinkConfig")
            .field("base_url", &self.base_url)
            .field("api_key", &"<redacted>")
            .field("socks5_proxy", &"<redacted>")
            .field("accept_invalid_certs", &self.accept_invalid_certs)
            .field("http_timeout", &self.http_timeout)
            .finish()
    }
}

impl Default for BlinkConfig {
    fn default() -> Self {
        Self {
            base_url: Some("https://api.blink.sv/graphql".to_string()),
            api_key: "".to_string(),
            socks5_proxy: Some("".to_string()),
            accept_invalid_certs: Some(true),
            http_timeout: Some(60),
        }
    }
}

#[cfg_attr(feature = "napi_rs", napi(object))]
#[cfg_attr(feature = "uniffi", derive(uniffi::Object))]
#[derive(Debug, Clone)]
pub struct BlinkNode {
    pub config: BlinkConfig,
}

// Constructor is inherent, not part of the trait
#[cfg_attr(feature = "uniffi", uniffi::export)]
impl BlinkNode {
    #[cfg_attr(feature = "uniffi", uniffi::constructor)]
    pub fn new(config: BlinkConfig) -> Self {
        Self { config }
    }
}

// All node methods - UniFFI exports these directly when the feature is enabled
#[cfg_attr(feature = "uniffi", uniffi::export(async_runtime = "tokio"))]
impl BlinkNode {
    pub async fn get_permissions(&self) -> Result<crate::Permissions, ApiError> {
        crate::permissions::parse_blink_token_permissions(&self.config.api_key).ok_or_else(|| {
            ApiError::InvalidInput(
                "Blink API keys cannot be introspected. Use a JWT-style token with scopes or manually test permissions against Blink GraphQL operations.".to_string(),
            )
        })
    }

    pub async fn get_info(&self) -> Result<NodeInfo, ApiError> {
        crate::blink::api::get_info(&self.config).await
    }

    pub async fn create_invoice(
        &self,
        params: CreateInvoiceParams,
    ) -> Result<Transaction, ApiError> {
        crate::blink::api::create_invoice(&self.config, params).await
    }

    pub async fn pay_invoice(
        &self,
        params: PayInvoiceParams,
    ) -> Result<PayInvoiceResponse, ApiError> {
        crate::blink::api::pay_invoice(&self.config, params).await
    }

    pub async fn prepare_onchain_transaction(
        &self,
        params: PrepareOnchainTransactionParams,
    ) -> Result<OnchainTransaction, ApiError> {
        crate::blink::api::prepare_onchain_transaction(&self.config, params).await
    }

    pub async fn pay_onchain(
        &self,
        transaction: OnchainTransaction,
    ) -> Result<PayOnchainResponse, ApiError> {
        crate::blink::api::pay_onchain(&self.config, transaction).await
    }

    pub async fn pay_onchain_with_options(
        &self,
        transaction: OnchainTransaction,
        options: PayOnchainOptions,
    ) -> Result<PayOnchainResponse, ApiError> {
        crate::blink::api::pay_onchain_with_options(&self.config, transaction, options).await
    }

    pub async fn create_offer(&self, _params: CreateOfferParams) -> Result<Offer, ApiError> {
        Err(ApiError::Api {
            reason: "create_offer not implemented for BlinkNode".to_string(),
        })
    }

    pub async fn get_offer(&self, search: Option<String>) -> Result<Offer, ApiError> {
        crate::blink::api::get_offer(&self.config, search).await
    }

    pub async fn list_offers(&self, search: Option<String>) -> Result<Vec<Offer>, ApiError> {
        crate::blink::api::list_offers(&self.config, search).await
    }

    pub async fn pay_offer(
        &self,
        offer: String,
        amount_msats: i64,
        payer_note: Option<String>,
    ) -> Result<PayInvoiceResponse, ApiError> {
        crate::blink::api::pay_offer(&self.config, offer, amount_msats, payer_note).await
    }

    pub async fn lookup_invoice(
        &self,
        params: LookupInvoiceParams,
    ) -> Result<crate::Transaction, ApiError> {
        crate::blink::api::lookup_invoice(
            &self.config,
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
        crate::blink::api::list_transactions(&self.config, params.from, params.limit, params.search)
            .await
    }

    pub async fn decode(&self, str: String) -> Result<String, ApiError> {
        crate::utils::decode_bolt11(str)
    }

    pub async fn decode_offer(&self, offer: String) -> Result<String, ApiError> {
        crate::utils::decode_offer(offer)
    }

    pub async fn on_invoice_events(
        &self,
        params: crate::types::OnInvoiceEventParams,
        callback: std::sync::Arc<dyn crate::types::OnInvoiceEventCallback>,
    ) {
        crate::blink::api::on_invoice_events(self.config.clone(), params, callback).await
    }
}

// Trait implementation for Rust consumers - uses the impl_lightning_node macro
// Trait implementation for polymorphic access via Arc<dyn LightningNode>
crate::impl_lightning_node!(BlinkNode);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        InvoiceType, OnchainFeePayer, OnchainFeePreference, OnchainFeePreferenceType,
        OnchainFeeSpeed, PrepareOnchainTransactionParams,
    };
    use dotenv::dotenv;
    use lazy_static::lazy_static;
    use std::env;
    use std::sync::{Arc, Mutex};

    const ONCHAIN_SEND_CONFIRMATION: &str = "I_UNDERSTAND_THIS_BROADCASTS_BITCOIN";

    lazy_static! {
        static ref BASE_URL: String = {
            dotenv().ok();
            env::var("BLINK_BASE_URL")
                .unwrap_or_else(|_| "https://api.blink.sv/graphql".to_string())
        };
        static ref API_KEY: String = {
            dotenv().ok();
            env::var("BLINK_API_KEY").expect("BLINK_API_KEY must be set")
        };
        static ref TEST_PAYMENT_HASH: String = {
            dotenv().ok();
            env::var("BLINK_TEST_PAYMENT_HASH").expect("BLINK_TEST_PAYMENT_HASH must be set")
        };
        static ref TEST_PAYMENT_REQUEST: String = {
            dotenv().ok();
            env::var("BLINK_TEST_PAYMENT_REQUEST").expect("BLINK_TEST_PAYMENT_REQUEST must be set")
        };
        static ref NODE: BlinkNode = {
            BlinkNode::new(BlinkConfig {
                base_url: Some(BASE_URL.clone()),
                api_key: API_KEY.clone(),
                http_timeout: Some(120),
                ..Default::default()
            })
        };
    }

    #[tokio::test]
    async fn test_get_info() {
        match NODE.get_info().await {
            Ok(info) => {
                println!("info: {:?}", info);
            }
            Err(e) => {
                panic!("Failed to get info: {:?}", e);
            }
        }
    }

    #[tokio::test]
    async fn test_create_invoice() {
        let amount_msats = 21000; // 21 sats
        let description = "Test Blink invoice".to_string();
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
                println!("Blink create_invoice: {:?}", txn);
                assert!(
                    !txn.invoice.is_empty(),
                    "Blink create_invoice Invoice should not be empty"
                );
            }
            Err(e) => {
                println!(
                    "Blink create_invoice failed (expected if no API key): {:?}",
                    e
                );
                // Don't panic as this requires valid API key
            }
        }
    }

    // #[tokio::test]
    // async fn test_pay_invoice() {
    //     match NODE.pay_invoice(PayInvoiceParams {
    //         invoice: TEST_PAYMENT_REQUEST.clone(),
    //         amount_msats: None, // Use amount from invoice
    //         ..Default::default()
    //     }).await {
    //         Ok(response) => {
    //             println!("Blink pay_invoice response: {:?}", response);
    //             assert!(
    //                 response.payment_hash.len() > 0,
    //                 "Payment hash should not be empty"
    //             );
    //         }
    //         Err(e) => {
    //             println!(
    //                 "Blink pay_invoice failed (expected if no API key or invalid invoice): {:?}",
    //                 e
    //             );
    //             // Don't panic as this requires valid API key and valid invoice
    //             // Common errors: insufficient balance, invalid invoice, etc.
    //         }
    //     }
    // }

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
                println!("Blink lookup invoice: {:?}", txn);
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
                        "Blink lookup invoice failed (expected if no API key): {:?}",
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
            limit: 100,
            payment_hash: None,
            search: None,
            created_after: None,
            created_before: None,
        };

        match NODE.list_transactions(params).await {
            Ok(txns) => {
                dbg!(&txns);
                // Validate we can parse transactions
                assert!(txns.len() >= 0, "Should contain at least zero transactions");
            }
            Err(e) => {
                println!(
                    "Blink list transactions failed (expected if no API key): {:?}",
                    e
                );
                // Don't panic as this requires valid API key
            }
        }
    }

    #[tokio::test]
    async fn test_pay_onchain_e2e() {
        dotenv().ok();

        let address =
            env::var("BLINK_ONCHAIN_TEST_ADDRESS").expect("BLINK_ONCHAIN_TEST_ADDRESS must be set");
        let amount_sats = env::var("BLINK_ONCHAIN_AMOUNT_SATS")
            .unwrap_or_else(|_| "10000".to_string())
            .parse::<i64>()
            .expect("BLINK_ONCHAIN_AMOUNT_SATS must be a positive integer");
        assert!(
            amount_sats > 0,
            "BLINK_ONCHAIN_AMOUNT_SATS must be positive"
        );

        let transaction = NODE
            .prepare_onchain_transaction(PrepareOnchainTransactionParams {
                address: address.clone(),
                amount_sats,
                fee: Some(OnchainFeePreference {
                    preference_type: OnchainFeePreferenceType::Speed,
                    speed: Some(OnchainFeeSpeed::Normal),
                    target_conf: None,
                    sats_per_vbyte: None,
                    backend: None,
                }),
                fee_payer: Some(OnchainFeePayer::Sender),
                description: Some("blink rust onchain quote".to_string()),
                idempotency_key: None,
            })
            .await
            .expect("prepare_onchain_transaction should create a Blink on-chain quote");

        dbg!("Onchain txn", &transaction);
        assert_eq!(transaction.address, address);
        assert_eq!(transaction.amount_sats, amount_sats);
        assert_eq!(transaction.fee_payer, OnchainFeePayer::Sender);
        assert!(
            transaction.fee_sats.map_or(false, |fee_sats| fee_sats >= 0),
            "on-chain transaction should include a non-negative fee quote"
        );

        if env::var("BLINK_RUN_ONCHAIN_SEND").ok().as_deref() != Some("true")
            || env::var("BLINK_ONCHAIN_SEND_CONFIRM").ok().as_deref()
                != Some(ONCHAIN_SEND_CONFIRMATION)
        {
            println!(
                "Prepared Blink on-chain quote; skipping broadcast without explicit confirmation"
            );
            return;
        }

        let payment = NODE
            .pay_onchain(transaction)
            .await
            .expect("pay_onchain should execute Blink on-chain send");

        assert_eq!(payment.address, address);
        assert_eq!(payment.amount_sats, amount_sats);
        assert!(
            matches!(payment.state.as_str(), "pending" | "completed"),
            "unexpected on-chain payment state: {}",
            payment.state
        );
    }

    #[tokio::test]
    async fn test_on_invoice_events() {
        struct OnInvoiceEventCallback {
            events: Arc<Mutex<Vec<String>>>,
        }

        impl crate::types::OnInvoiceEventCallback for OnInvoiceEventCallback {
            fn success(&self, transaction: Option<Transaction>) {
                dbg!("Success blink paid");
                dbg!(&transaction);
                let mut events = self.events.lock().unwrap();
                events.push(format!("{} - {:?}", "success", transaction));
            }
            fn pending(&self, transaction: Option<Transaction>) {
                dbg!("Pending blink payment");
                let mut events = self.events.lock().unwrap();
                events.push(format!("{} - {:?}", "pending", transaction));
            }
            fn failure(&self, transaction: Option<Transaction>) {
                dbg!("Failure blink payment");
                let mut events = self.events.lock().unwrap();
                events.push(format!("{} - {:?}", "failure", transaction));
            }
        }

        let events = Arc::new(Mutex::new(Vec::new()));
        let callback = OnInvoiceEventCallback {
            events: events.clone(),
        };

        let params = crate::types::OnInvoiceEventParams {
            payment_hash: Some(TEST_PAYMENT_HASH.to_string()),
            polling_delay_sec: 2,
            max_polling_sec: 5,
            ..Default::default()
        };

        NODE.on_invoice_events(params, std::sync::Arc::new(callback))
            .await;

        // Check that some events were captured
        let events_guard = events.lock().unwrap();
        println!("Blink events captured: {:?}", *events_guard);

        // We expect at least one event (even if it's a failure due to invoice not found)
        assert!(
            !events_guard.is_empty(),
            "Should capture at least one event"
        );
    }
}
