use std::{path::PathBuf, sync::Arc};

use lexe::{
    config::WalletEnvConfig,
    types::auth::{ClientCredentials, CredentialsRef},
    wallet::LexeWallet,
};

use crate::{
    ApiError, CreateInvoiceParams, CreateOfferParams, ListTransactionsParams, LookupInvoiceParams,
    NodeInfo, Offer, PayInvoiceParams, PayInvoiceResponse, Permissions, Transaction,
};

#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
#[derive(Clone)]
pub struct LexeConfig {
    /// Portable client credentials exported by the Lexe app.
    pub client_credentials: String,
    /// Base directory for Lexe's local payment cache.
    #[cfg_attr(feature = "uniffi", uniffi(default = None))]
    pub data_dir: Option<String>,
    /// `mainnet` (default), `testnet`, or `testnet3`.
    #[cfg_attr(feature = "uniffi", uniffi(default = Some("mainnet")))]
    pub network: Option<String>,
}

impl Default for LexeConfig {
    fn default() -> Self {
        Self {
            client_credentials: String::new(),
            data_dir: None,
            network: Some("mainnet".to_owned()),
        }
    }
}

impl std::fmt::Debug for LexeConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LexeConfig")
            .field("client_credentials", &"<redacted>")
            .field("data_dir", &"<redacted>")
            .field("network", &self.network)
            .finish()
    }
}

#[cfg_attr(feature = "uniffi", derive(uniffi::Object))]
#[derive(Clone)]
pub struct LexeNode {
    pub config: LexeConfig,
    wallet: Arc<LexeWallet>,
    network: String,
}

impl std::fmt::Debug for LexeNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LexeNode")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

#[cfg_attr(feature = "uniffi", uniffi::export)]
impl LexeNode {
    #[cfg_attr(feature = "uniffi", uniffi::constructor)]
    pub fn new(config: LexeConfig) -> Result<Self, ApiError> {
        if config.client_credentials.trim().is_empty() {
            return Err(ApiError::InvalidInput(
                "client_credentials must not be empty".to_owned(),
            ));
        }

        let (env_config, network) = match config.network.as_deref() {
            None | Some("") | Some("mainnet") | Some("bitcoin") => {
                (WalletEnvConfig::mainnet(), "mainnet")
            }
            Some("testnet") | Some("testnet3") => (WalletEnvConfig::testnet3(), "testnet3"),
            Some(network) => {
                return Err(ApiError::InvalidInput(format!(
                    "Unsupported Lexe network: {network}"
                )));
            }
        };

        let credentials =
            ClientCredentials::from_string(&config.client_credentials).map_err(|error| {
                ApiError::InvalidInput(format!("Invalid Lexe client credentials: {error}"))
            })?;
        let data_dir = config
            .data_dir
            .as_deref()
            .filter(|path| !path.is_empty())
            .map(PathBuf::from);
        let wallet =
            LexeWallet::load_or_fresh(env_config, CredentialsRef::from(&credentials), data_dir)
                .map_err(|error| ApiError::Api {
                    reason: format!("Failed to initialize Lexe wallet: {error}"),
                })?;

        Ok(Self {
            config,
            wallet: Arc::new(wallet),
            network: network.to_owned(),
        })
    }
}

#[cfg_attr(feature = "uniffi", uniffi::export(async_runtime = "tokio"))]
impl LexeNode {
    pub async fn get_permissions(&self) -> Result<Permissions, ApiError> {
        Ok(crate::lexe::api::permissions())
    }

    pub async fn get_info(&self) -> Result<NodeInfo, ApiError> {
        crate::lexe::api::get_info(&self.wallet, &self.network).await
    }

    pub async fn create_invoice(
        &self,
        params: CreateInvoiceParams,
    ) -> Result<Transaction, ApiError> {
        crate::lexe::api::create_invoice(&self.wallet, params).await
    }

    pub async fn pay_invoice(
        &self,
        params: PayInvoiceParams,
    ) -> Result<PayInvoiceResponse, ApiError> {
        crate::lexe::api::pay_invoice(&self.wallet, params).await
    }

    pub async fn create_offer(&self, params: CreateOfferParams) -> Result<Offer, ApiError> {
        crate::lexe::api::create_offer(&self.wallet, params).await
    }

    pub async fn get_offer(&self, _search: Option<String>) -> Result<Offer, ApiError> {
        Err(ApiError::Api {
            reason: "get_offer is not available in the Lexe Rust SDK".to_owned(),
        })
    }

    pub async fn list_offers(&self, _search: Option<String>) -> Result<Vec<Offer>, ApiError> {
        Err(ApiError::Api {
            reason: "list_offers is not available in the Lexe Rust SDK".to_owned(),
        })
    }

    pub async fn pay_offer(
        &self,
        offer: String,
        amount_msats: i64,
        payer_note: Option<String>,
    ) -> Result<PayInvoiceResponse, ApiError> {
        crate::lexe::api::pay_offer(&self.wallet, offer, amount_msats, payer_note).await
    }

    pub async fn lookup_invoice(
        &self,
        params: LookupInvoiceParams,
    ) -> Result<Transaction, ApiError> {
        crate::lexe::api::lookup_invoice(&self.wallet, params).await
    }

    pub async fn list_transactions(
        &self,
        params: ListTransactionsParams,
    ) -> Result<Vec<Transaction>, ApiError> {
        crate::lexe::api::list_transactions(&self.wallet, params).await
    }

    pub async fn decode(&self, value: String) -> Result<String, ApiError> {
        crate::utils::decode_bolt11(value)
    }

    pub async fn decode_offer(&self, offer: String) -> Result<String, ApiError> {
        crate::utils::decode_offer(offer)
    }

    pub async fn on_invoice_events(
        &self,
        params: crate::OnInvoiceEventParams,
        callback: Arc<dyn crate::OnInvoiceEventCallback>,
    ) {
        crate::lexe::api::on_invoice_events(self.wallet.clone(), params, callback).await
    }
}

crate::impl_lightning_node!(LexeNode);

#[cfg(test)]
mod tests {
    use sha2::Digest;

    use super::*;

    fn optional_test_env_var(name: &str) -> Option<String> {
        dotenv::dotenv().ok();
        std::env::var(name).ok().filter(|value| !value.is_empty())
    }

    fn test_env_var(name: &str) -> Option<String> {
        let value = optional_test_env_var(name);
        if value.is_none() {
            eprintln!("Skipping Lexe integration test: {name} is not set");
        }
        value
    }

    fn integration_node() -> Option<(tempfile::TempDir, LexeNode)> {
        let client_credentials = test_env_var("LEXE_CLIENT_CREDENTIALS")?;
        let data_dir = tempfile::tempdir().expect("tempdir");
        let node = LexeNode::new(LexeConfig {
            client_credentials,
            data_dir: Some(data_dir.path().display().to_string()),
            network: Some("mainnet".to_owned()),
        })
        .expect("valid Lexe credentials should initialize");

        Some((data_dir, node))
    }

    fn integration_payment_hash() -> Option<String> {
        if let Some(payment_hash) = optional_test_env_var("LEXE_TEST_PAYMENT_HASH") {
            return Some(payment_hash);
        }

        let Some(invoice) = optional_test_env_var("LEXE_TEST_PAYMENT_REQUEST") else {
            eprintln!(
                "Skipping Lexe integration test: LEXE_TEST_PAYMENT_HASH or \
                 LEXE_TEST_PAYMENT_REQUEST is not set"
            );
            return None;
        };
        let invoice = invoice
            .parse::<lexe::types::bitcoin::Invoice>()
            .expect("LEXE_TEST_PAYMENT_REQUEST must be a valid BOLT 11 invoice");
        Some(invoice.payment_hash().to_string())
    }

    fn assert_preimage_matches_hash(preimage: &str, payment_hash: &str) {
        let preimage = hex::decode(preimage).expect("preimage must be hex encoded");
        assert_eq!(preimage.len(), 32, "preimage must contain 32 bytes");
        let actual_hash = hex::encode(sha2::Sha256::digest(preimage));
        assert_eq!(actual_hash, payment_hash.to_ascii_lowercase());
    }

    #[test]
    fn config_debug_redacts_credentials_and_path() {
        let config = LexeConfig {
            client_credentials: "secret-client-credential".to_owned(),
            data_dir: Some("/private/wallet/path".to_owned()),
            network: Some("mainnet".to_owned()),
        };
        let debug = format!("{config:?}");

        assert!(!debug.contains("secret-client-credential"));
        assert!(!debug.contains("/private/wallet/path"));
        assert!(debug.contains("<redacted>"));
    }

    #[test]
    fn empty_credentials_are_rejected() {
        let error = LexeNode::new(LexeConfig::default()).unwrap_err();
        assert!(matches!(error, ApiError::InvalidInput(_)));
    }

    #[tokio::test]
    async fn integration_get_info_with_client_credentials() {
        let Some((_data_dir, node)) = integration_node() else {
            return;
        };

        let info = node.get_info().await.expect("Lexe get_info should succeed");
        assert!(!info.pubkey.is_empty());
        assert_eq!(info.network, "mainnet");
    }

    /// Requires a fresh `LEXE_TEST_PAYMENT_REQUEST` and sends a real payment.
    #[tokio::test]
    #[ignore = "sends a real Lightning payment"]
    async fn integration_pay_invoice_with_client_credentials() {
        let Some(invoice) = test_env_var("LEXE_TEST_PAYMENT_REQUEST") else {
            return;
        };
        let Some((_data_dir, node)) = integration_node() else {
            return;
        };
        dotenv::dotenv().ok();
        let amount_msats = std::env::var("LEXE_TEST_AMOUNT_MSATS")
            .ok()
            .filter(|amount| !amount.is_empty())
            .map(|amount| {
                amount
                    .parse::<i64>()
                    .expect("LEXE_TEST_AMOUNT_MSATS must be a signed integer")
            });

        let payment = node
            .pay_invoice(PayInvoiceParams {
                invoice,
                amount_msats,
                ..Default::default()
            })
            .await
            .expect("Lexe pay_invoice should pay the configured test invoice");
        println!("Lexe payment preimage: {}", payment.preimage);
        dbg!(&payment);
        assert!(!payment.payment_hash.is_empty());
        assert!(!payment.preimage.is_empty());
        assert!(payment.fee_msats >= 0);
        assert_preimage_matches_hash(&payment.preimage, &payment.payment_hash);
    }

    #[tokio::test]
    async fn integration_lookup_invoice_returns_preimage() {
        let Some(payment_hash) = integration_payment_hash() else {
            return;
        };
        let Some((_data_dir, node)) = integration_node() else {
            return;
        };
        let lookup = node
            .lookup_invoice(LookupInvoiceParams {
                payment_hash: Some(payment_hash.clone()),
                search: None,
            })
            .await
            .expect("Lexe lookup_invoice should find the completed payment");
        dbg!(&lookup);
        assert_eq!(lookup.payment_hash, payment_hash);
        assert_preimage_matches_hash(&lookup.preimage, &lookup.payment_hash);
    }

    #[tokio::test]
    async fn integration_list_transactions_returns_preimage() {
        let Some(payment_hash) = integration_payment_hash() else {
            return;
        };
        let Some((_data_dir, node)) = integration_node() else {
            return;
        };
        let transactions = node
            .list_transactions(ListTransactionsParams {
                from: 0,
                limit: 10,
                payment_hash: Some(payment_hash.clone()),
                search: None,
                created_after: None,
                created_before: None,
            })
            .await
            .expect("Lexe list_transactions should find the completed payment");
        let listed = transactions
            .into_iter()
            .find(|transaction| transaction.payment_hash == payment_hash)
            .expect("completed payment should be present in Lexe transactions");
        dbg!(&listed);
        assert_preimage_matches_hash(&listed.preimage, &listed.payment_hash);
    }
}
