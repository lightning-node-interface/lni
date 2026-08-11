use once_cell::sync::Lazy;
use std::time::Duration;

// Global Tokio runtime for async operations
// This is needed because UniFFI's async trait support requires a runtime that's always available
// Swift/Kotlin drive the outer future (UniFFI's bridging), while Tokio drives the actual async work
pub static TOKIO_RUNTIME: Lazy<tokio::runtime::Runtime> = Lazy::new(|| {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_name("lni-tokio")
        .build()
        .expect("Failed to create Tokio runtime for LNI")
});

#[cfg_attr(feature = "uniffi", derive(uniffi::Error))]
#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("HttpError: {reason}")]
    Http { reason: String },
    #[error("ApiError: {reason}")]
    Api { reason: String },
    #[error("JsonError: {reason}")]
    Json { reason: String },
    #[error("NetworkError: {0}")]
    NetworkError(String),
    #[error("InvalidInput: {0}")]
    InvalidInput(String),
    #[error("LnurlError: {0}")]
    LnurlError(String),
    #[error("NwcError: {code}: {message}")]
    Nwc { code: String, message: String },
}
impl From<serde_json::Error> for ApiError {
    fn from(e: serde_json::Error) -> Self {
        Self::Json {
            reason: e.to_string(),
        }
    }
}

/// Generate a new BIP39 mnemonic phrase for wallet creation.
/// Uses cryptographically secure randomness from the OS.
///
/// # Arguments
/// * `word_count` - Number of words: 12 (default) or 24. If None or invalid, defaults to 12.
///
/// # Returns
/// A space-separated mnemonic phrase
#[cfg_attr(feature = "uniffi", uniffi::export)]
pub fn generate_mnemonic(word_count: Option<u8>) -> Result<String, ApiError> {
    use bip39::{Language, Mnemonic};
    use rand::rngs::OsRng;
    use rand::RngCore;

    let entropy_size = match word_count {
        Some(24) => 32,
        _ => 16,
    };

    let mut entropy = vec![0u8; entropy_size];
    OsRng.fill_bytes(&mut entropy);

    let mnemonic =
        Mnemonic::from_entropy_in(Language::English, &entropy).map_err(|e| ApiError::Api {
            reason: format!("Failed to generate mnemonic: {}", e),
        })?;

    Ok(mnemonic.to_string())
}

/// Macro to implement LightningNode trait by delegating to inherent methods.
/// This avoids code duplication between UniFFI exports and trait implementations.
/// The macro works for both UniFFI and non-UniFFI builds.
///
/// For UniFFI builds, the async work is spawned onto the global TOKIO_RUNTIME
/// since Swift/Kotlin drive the outer future but Tokio needs to drive the actual async work.
#[macro_export]
macro_rules! impl_lightning_node {
    ($node_type:ty) => {
        #[async_trait::async_trait]
        impl crate::LightningNode for $node_type {
            async fn get_permissions(&self) -> Result<crate::Permissions, crate::ApiError> {
                let this = self.clone();
                crate::TOKIO_RUNTIME
                    .spawn(async move { <$node_type>::get_permissions(&this).await })
                    .await
                    .unwrap()
            }

            async fn get_info(&self) -> Result<crate::NodeInfo, crate::ApiError> {
                let this = self.clone();
                crate::TOKIO_RUNTIME
                    .spawn(async move { <$node_type>::get_info(&this).await })
                    .await
                    .unwrap()
            }

            async fn create_invoice(
                &self,
                params: crate::CreateInvoiceParams,
            ) -> Result<crate::Transaction, crate::ApiError> {
                let this = self.clone();
                crate::TOKIO_RUNTIME
                    .spawn(async move { <$node_type>::create_invoice(&this, params).await })
                    .await
                    .unwrap()
            }

            async fn pay_invoice(
                &self,
                params: crate::PayInvoiceParams,
            ) -> Result<crate::PayInvoiceResponse, crate::ApiError> {
                let this = self.clone();
                crate::TOKIO_RUNTIME
                    .spawn(async move { <$node_type>::pay_invoice(&this, params).await })
                    .await
                    .unwrap()
            }

            async fn create_offer(
                &self,
                params: crate::CreateOfferParams,
            ) -> Result<crate::Offer, crate::ApiError> {
                let this = self.clone();
                crate::TOKIO_RUNTIME
                    .spawn(async move { <$node_type>::create_offer(&this, params).await })
                    .await
                    .unwrap()
            }

            async fn get_offer(
                &self,
                search: Option<String>,
            ) -> Result<crate::Offer, crate::ApiError> {
                let this = self.clone();
                crate::TOKIO_RUNTIME
                    .spawn(async move { <$node_type>::get_offer(&this, search).await })
                    .await
                    .unwrap()
            }

            async fn list_offers(
                &self,
                search: Option<String>,
            ) -> Result<Vec<crate::Offer>, crate::ApiError> {
                let this = self.clone();
                crate::TOKIO_RUNTIME
                    .spawn(async move { <$node_type>::list_offers(&this, search).await })
                    .await
                    .unwrap()
            }

            async fn pay_offer(
                &self,
                offer: String,
                amount_msats: i64,
                payer_note: Option<String>,
            ) -> Result<crate::PayInvoiceResponse, crate::ApiError> {
                let this = self.clone();
                crate::TOKIO_RUNTIME
                    .spawn(async move {
                        <$node_type>::pay_offer(&this, offer, amount_msats, payer_note).await
                    })
                    .await
                    .unwrap()
            }

            async fn lookup_invoice(
                &self,
                params: crate::LookupInvoiceParams,
            ) -> Result<crate::Transaction, crate::ApiError> {
                let this = self.clone();
                crate::TOKIO_RUNTIME
                    .spawn(async move { <$node_type>::lookup_invoice(&this, params).await })
                    .await
                    .unwrap()
            }

            async fn list_transactions(
                &self,
                params: crate::ListTransactionsParams,
            ) -> Result<Vec<crate::Transaction>, crate::ApiError> {
                let this = self.clone();
                crate::TOKIO_RUNTIME
                    .spawn(async move { <$node_type>::list_transactions(&this, params).await })
                    .await
                    .unwrap()
            }

            async fn decode(&self, str: String) -> Result<String, crate::ApiError> {
                let this = self.clone();
                crate::TOKIO_RUNTIME
                    .spawn(async move { <$node_type>::decode(&this, str).await })
                    .await
                    .unwrap()
            }

            async fn decode_offer(&self, offer: String) -> Result<String, crate::ApiError> {
                let this = self.clone();
                crate::TOKIO_RUNTIME
                    .spawn(async move { <$node_type>::decode_offer(&this, offer).await })
                    .await
                    .unwrap()
            }

            async fn on_invoice_events(
                &self,
                params: crate::types::OnInvoiceEventParams,
                callback: std::sync::Arc<dyn crate::types::OnInvoiceEventCallback>,
            ) {
                let this = self.clone();
                let handle = crate::TOKIO_RUNTIME.spawn(async move {
                    <$node_type>::on_invoice_events(&this, params, callback).await
                });
                handle.await.unwrap()
            }
        }
    };
}

pub mod phoenixd {
    pub mod api;
    pub mod lib;
    pub mod types;
    pub use lib::{PhoenixdConfig, PhoenixdNode};
}

pub mod cln {
    pub mod api;
    pub mod lib;
    pub mod types;
    pub use lib::{ClnConfig, ClnNode};
}

pub mod lnd {
    pub mod api;
    pub mod lib;
    pub mod types;
    pub use lib::{LndConfig, LndNode};
}

pub mod nwc {
    pub mod api;
    pub mod lib;
    pub mod types;
    pub use lib::{NwcConfig, NwcLightningAddress, NwcNode};
}

pub mod strike {
    pub mod api;
    pub mod lib;
    pub mod types;
    pub use lib::{StrikeConfig, StrikeNode};
}

pub mod blink {
    pub mod api;
    pub mod lib;
    pub mod types;
    pub use lib::{BlinkConfig, BlinkNode};
}

pub mod galoy {
    pub mod api;
    pub mod lib;
    pub mod types;
    pub use lib::{
        GaloyCapabilities, GaloyConfig, GaloyInvoiceOperation, GaloyInvoiceOperationsConfig,
        GaloyNode, GaloyPaymentConfig, GaloyPaymentOutcome, GaloyPaymentResponse,
        GaloyPaymentState, GaloyPaymentStatusMapping, GaloyPermissionsMode, GaloyProvider,
        GaloyWalletConfig,
    };
}

pub mod flash {
    pub mod lib;
    pub use lib::{FlashConfig, FlashNode, DEFAULT_FLASH_GRAPHQL_URL};
}

pub mod speed {
    pub mod api;
    pub mod lib;
    pub mod types;
    pub use lib::{SpeedConfig, SpeedNode};
}

pub mod spark {
    pub mod api;
    pub mod lib;
    pub mod types;
    pub use lib::{SparkConfig, SparkNode};
}

pub mod lexe {
    pub mod api;
    pub mod lib;
    pub use lib::{LexeConfig, LexeHumanBitcoinAddress, LexeNode};
}

pub mod lnurl;

pub mod error_normalization;

pub mod permissions;

pub mod types;
pub use types::*;

pub mod utils;
pub use utils::*;

pub mod database;
pub use database::{Db, DbError, Payment};

pub(crate) fn http_client_builder() -> reqwest::ClientBuilder {
    reqwest::Client::builder().redirect(reqwest::redirect::Policy::none())
}

pub(crate) fn default_http_client() -> reqwest::Client {
    http_client_builder()
        .build()
        .expect("default HTTP client must build")
}

fn demo_http_client(socks5_proxy: Option<&str>) -> Result<reqwest::Client, ApiError> {
    let Some(proxy_url) = socks5_proxy.filter(|url| !url.is_empty()) else {
        return Ok(default_http_client());
    };

    let proxy = reqwest::Proxy::all(proxy_url).map_err(|_| ApiError::Http {
        reason: "Invalid SOCKS5 proxy configuration".to_string(),
    })?;
    http_client_builder()
        .proxy(proxy)
        .build()
        .map_err(|_| ApiError::Http {
            reason: "Failed to build SOCKS5 proxy client".to_string(),
        })
}

// Make an HTTP request to get IP address and simulate latency with optional SOCKS5 proxy
#[cfg_attr(feature = "uniffi", uniffi::export(async_runtime = "tokio"))]
pub async fn say_after_with_tokio(
    ms: u16,
    who: String,
    url: String,
    socks5_proxy: Option<String>,
    header_key: Option<String>,
    header_value: Option<String>,
) -> String {
    let client = match demo_http_client(socks5_proxy.as_deref()) {
        Ok(client) => client,
        Err(_) => return "Failed to configure HTTP client".to_string(),
    };

    // Create request with optional header
    let mut request = client.get(&url);

    if let (Some(key), Some(value)) = (header_key, header_value) {
        request = request.header(&key, &value);
    }

    // Make HTTP request
    let ip_result = request
        .send()
        .await
        .and_then(|response| response.error_for_status());

    let page_content = match ip_result {
        Ok(response) => match response.text().await {
            Ok(html) => html,
            Err(_) => "Failed to read response text".to_string(),
        },
        Err(_) => "Failed to make HTTP request".to_string(),
    };

    // Simulate latency
    tokio::time::sleep(Duration::from_millis(ms.into())).await;

    format!("Hello, {who}! Your IP address is: {page_content} (with Tokio after {ms}ms delay)")
}

// Factory functions for creating nodes as Arc<dyn LightningNode>
// These enable polymorphic access in Kotlin/Swift without manual wrapper code

use std::sync::Arc;

/// Create a Strike node as a polymorphic LightningNode
#[cfg_attr(feature = "uniffi", uniffi::export)]
pub fn create_strike_node(config: strike::StrikeConfig) -> Arc<dyn LightningNode> {
    Arc::new(strike::StrikeNode::new(config))
}

/// Create a Speed node as a polymorphic LightningNode
#[cfg_attr(feature = "uniffi", uniffi::export)]
pub fn create_speed_node(config: speed::SpeedConfig) -> Arc<dyn LightningNode> {
    Arc::new(speed::SpeedNode::new(config))
}

/// Create a Blink node as a polymorphic LightningNode
#[cfg_attr(feature = "uniffi", uniffi::export)]
pub fn create_blink_node(config: blink::BlinkConfig) -> Arc<dyn LightningNode> {
    Arc::new(blink::BlinkNode::new(config))
}

/// Create a configurable Galoy GraphQL node as a polymorphic LightningNode.
#[cfg_attr(feature = "uniffi", uniffi::export)]
pub fn create_galoy_node(config: galoy::GaloyConfig) -> Arc<dyn LightningNode> {
    Arc::new(galoy::GaloyNode::new(config))
}

/// Create a Flash node backed by the generic Galoy implementation.
#[cfg_attr(feature = "uniffi", uniffi::export)]
pub fn create_flash_node(config: flash::FlashConfig) -> Arc<dyn LightningNode> {
    Arc::new(flash::FlashNode::new(config))
}

/// Create a Phoenixd node as a polymorphic LightningNode
#[cfg_attr(feature = "uniffi", uniffi::export)]
pub fn create_phoenixd_node(config: phoenixd::PhoenixdConfig) -> Arc<dyn LightningNode> {
    Arc::new(phoenixd::PhoenixdNode::new(config))
}

/// Create a CLN node as a polymorphic LightningNode
#[cfg_attr(feature = "uniffi", uniffi::export)]
pub fn create_cln_node(config: cln::ClnConfig) -> Arc<dyn LightningNode> {
    Arc::new(cln::ClnNode::new(config))
}

/// Create an LND node as a polymorphic LightningNode
#[cfg_attr(feature = "uniffi", uniffi::export)]
pub fn create_lnd_node(config: lnd::LndConfig) -> Arc<dyn LightningNode> {
    Arc::new(lnd::LndNode::new(config))
}

/// Create an NWC node as a polymorphic LightningNode
#[cfg(feature = "uniffi")]
#[uniffi::export]
pub fn create_nwc_node(config: nwc::NwcConfig) -> Arc<dyn LightningNode> {
    Arc::new(nwc::NwcNode::new(config))
}

/// Create a Spark node as a polymorphic LightningNode
#[cfg(feature = "uniffi")]
#[uniffi::export(async_runtime = "tokio")]
pub async fn create_spark_node(
    config: spark::SparkConfig,
) -> Result<Arc<dyn LightningNode>, ApiError> {
    let node = spark::SparkNode::new(config).await?;
    Ok(Arc::new(node))
}

/// Create a Lexe node backed by revocable client credentials.
#[cfg_attr(feature = "uniffi", uniffi::export)]
pub fn create_lexe_node(config: lexe::LexeConfig) -> Result<Arc<dyn LightningNode>, ApiError> {
    Ok(Arc::new(lexe::LexeNode::new(config)?))
}

#[cfg(feature = "uniffi")]
uniffi::setup_scaffolding!();

#[cfg(test)]
mod debug_redaction_tests {
    #[test]
    fn demo_client_does_not_bypass_invalid_proxy() {
        assert!(super::demo_http_client(Some("://invalid")).is_err());
    }

    fn assert_redacted(label: &str, output: &str, secrets: &[&str]) {
        assert!(
            output.contains("<redacted>"),
            "{} Debug output should include a redaction marker: {}",
            label,
            output
        );
        for secret in secrets {
            assert!(
                !output.contains(secret),
                "{} Debug output leaked secret {:?}: {}",
                label,
                secret,
                output
            );
        }
    }

    #[test]
    fn debug_output_redacts_secret_config_fields() {
        let proxy = "socks5h://proxy-user:proxy-pass@127.0.0.1:9150";

        let lnd = crate::lnd::LndConfig {
            url: "https://lnd.example".to_string(),
            macaroon: "lnd-macaroon-secret".to_string(),
            socks5_proxy: Some(proxy.to_string()),
            accept_invalid_certs: Some(false),
            http_timeout: Some(30),
        };
        assert_redacted(
            "LndConfig",
            &format!("{:?}", lnd),
            &["lnd-macaroon-secret", proxy],
        );
        assert_redacted(
            "LndNode",
            &format!("{:?}", crate::lnd::LndNode::new(lnd)),
            &["lnd-macaroon-secret", proxy],
        );

        let cln = crate::cln::ClnConfig {
            url: "https://cln.example".to_string(),
            rune: "cln-rune-secret".to_string(),
            socks5_proxy: Some(proxy.to_string()),
            accept_invalid_certs: Some(false),
            http_timeout: Some(30),
        };
        assert_redacted(
            "ClnConfig",
            &format!("{:?}", cln),
            &["cln-rune-secret", proxy],
        );
        assert_redacted(
            "ClnNode",
            &format!("{:?}", crate::cln::ClnNode::new(cln)),
            &["cln-rune-secret", proxy],
        );

        let nwc = crate::nwc::NwcConfig {
            nwc_uri:
                "nostr+walletconnect://wallet-pubkey?relay=wss://relay.example&secret=nwc-secret"
                    .to_string(),
            socks5_proxy: Some(proxy.to_string()),
            accept_invalid_certs: Some(false),
            http_timeout: Some(30),
        };
        assert_redacted("NwcConfig", &format!("{:?}", nwc), &["nwc-secret", proxy]);
        assert_redacted(
            "NwcNode",
            &format!("{:?}", crate::nwc::NwcNode::new(nwc)),
            &["nwc-secret", proxy],
        );

        let phoenixd = crate::phoenixd::PhoenixdConfig {
            url: "https://phoenixd.example".to_string(),
            password: "phoenixd-password-secret".to_string(),
            socks5_proxy: Some(proxy.to_string()),
            accept_invalid_certs: Some(false),
            http_timeout: Some(30),
        };
        assert_redacted(
            "PhoenixdConfig",
            &format!("{:?}", phoenixd),
            &["phoenixd-password-secret", proxy],
        );
        assert_redacted(
            "PhoenixdNode",
            &format!("{:?}", crate::phoenixd::PhoenixdNode::new(phoenixd)),
            &["phoenixd-password-secret", proxy],
        );

        let strike = crate::strike::StrikeConfig {
            base_url: Some("https://strike.example".to_string()),
            api_key: "strike-api-key-secret".to_string(),
            socks5_proxy: Some(proxy.to_string()),
            accept_invalid_certs: Some(false),
            http_timeout: Some(30),
        };
        assert_redacted(
            "StrikeConfig",
            &format!("{:?}", strike),
            &["strike-api-key-secret", proxy],
        );
        assert_redacted(
            "StrikeNode",
            &format!("{:?}", crate::strike::StrikeNode::new(strike)),
            &["strike-api-key-secret", proxy],
        );

        let blink = crate::blink::BlinkConfig {
            base_url: Some("https://blink.example".to_string()),
            api_key: "blink-api-key-secret".to_string(),
            socks5_proxy: Some(proxy.to_string()),
            accept_invalid_certs: Some(false),
            http_timeout: Some(30),
        };
        assert_redacted(
            "BlinkConfig",
            &format!("{:?}", blink),
            &["blink-api-key-secret", proxy],
        );
        assert_redacted(
            "BlinkNode",
            &format!("{:?}", crate::blink::BlinkNode::new(blink)),
            &["blink-api-key-secret", proxy],
        );

        let speed = crate::speed::SpeedConfig {
            base_url: Some("https://speed.example".to_string()),
            api_key: "speed-api-key-secret".to_string(),
            socks5_proxy: Some(proxy.to_string()),
            accept_invalid_certs: Some(false),
            http_timeout: Some(30),
        };
        assert_redacted(
            "SpeedConfig",
            &format!("{:?}", speed),
            &["speed-api-key-secret", proxy],
        );
        assert_redacted(
            "SpeedNode",
            &format!("{:?}", crate::speed::SpeedNode::new(speed)),
            &["speed-api-key-secret", proxy],
        );

        let spark = crate::spark::SparkConfig {
            mnemonic: "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about".to_string(),
            passphrase: Some("spark-passphrase-secret".to_string()),
            api_key: Some("spark-api-key-secret".to_string()),
            storage_dir: "/tmp/lni-spark-secret-path".to_string(),
            network: Some("mainnet".to_string()),
        };
        assert_redacted(
            "SparkConfig",
            &format!("{:?}", spark),
            &[
                "abandon abandon abandon",
                "spark-passphrase-secret",
                "spark-api-key-secret",
                "/tmp/lni-spark-secret-path",
            ],
        );
    }
}
