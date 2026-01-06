#[cfg(feature = "napi_rs")]
use napi_derive::napi;
#[cfg(feature = "napi_rs")]
use napi::bindgen_prelude::*;

use std::time::Duration;
use once_cell::sync::Lazy;

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
}
impl From<serde_json::Error> for ApiError {
    fn from(e: serde_json::Error) -> Self {
        Self::Json {
            reason: e.to_string(),
        }
    }
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
            async fn get_info(&self) -> Result<crate::NodeInfo, crate::ApiError> {
                let this = self.clone();
                crate::TOKIO_RUNTIME.spawn(async move {
                    <$node_type>::get_info(&this).await
                }).await.unwrap()
            }

            async fn create_invoice(&self, params: crate::CreateInvoiceParams) -> Result<crate::Transaction, crate::ApiError> {
                let this = self.clone();
                crate::TOKIO_RUNTIME.spawn(async move {
                    <$node_type>::create_invoice(&this, params).await
                }).await.unwrap()
            }

            async fn pay_invoice(&self, params: crate::PayInvoiceParams) -> Result<crate::PayInvoiceResponse, crate::ApiError> {
                let this = self.clone();
                crate::TOKIO_RUNTIME.spawn(async move {
                    <$node_type>::pay_invoice(&this, params).await
                }).await.unwrap()
            }

            async fn create_offer(&self, params: crate::CreateOfferParams) -> Result<crate::Offer, crate::ApiError> {
                let this = self.clone();
                crate::TOKIO_RUNTIME.spawn(async move {
                    <$node_type>::create_offer(&this, params).await
                }).await.unwrap()
            }

            async fn get_offer(&self, search: Option<String>) -> Result<crate::Offer, crate::ApiError> {
                let this = self.clone();
                crate::TOKIO_RUNTIME.spawn(async move {
                    <$node_type>::get_offer(&this, search).await
                }).await.unwrap()
            }

            async fn list_offers(&self, search: Option<String>) -> Result<Vec<crate::Offer>, crate::ApiError> {
                let this = self.clone();
                crate::TOKIO_RUNTIME.spawn(async move {
                    <$node_type>::list_offers(&this, search).await
                }).await.unwrap()
            }

            async fn pay_offer(
                &self,
                offer: String,
                amount_msats: i64,
                payer_note: Option<String>,
            ) -> Result<crate::PayInvoiceResponse, crate::ApiError> {
                let this = self.clone();
                crate::TOKIO_RUNTIME.spawn(async move {
                    <$node_type>::pay_offer(&this, offer, amount_msats, payer_note).await
                }).await.unwrap()
            }

            async fn lookup_invoice(&self, params: crate::LookupInvoiceParams) -> Result<crate::Transaction, crate::ApiError> {
                let this = self.clone();
                crate::TOKIO_RUNTIME.spawn(async move {
                    <$node_type>::lookup_invoice(&this, params).await
                }).await.unwrap()
            }

            async fn list_transactions(
                &self,
                params: crate::ListTransactionsParams,
            ) -> Result<Vec<crate::Transaction>, crate::ApiError> {
                let this = self.clone();
                crate::TOKIO_RUNTIME.spawn(async move {
                    <$node_type>::list_transactions(&this, params).await
                }).await.unwrap()
            }

            async fn decode(&self, str: String) -> Result<String, crate::ApiError> {
                let this = self.clone();
                crate::TOKIO_RUNTIME.spawn(async move {
                    <$node_type>::decode(&this, str).await
                }).await.unwrap()
            }

            async fn on_invoice_events(
                &self,
                params: crate::types::OnInvoiceEventParams,
                callback: std::sync::Arc<dyn crate::types::OnInvoiceEventCallback>,
            ) {
                let this = self.clone();
                crate::TOKIO_RUNTIME.spawn(async move {
                    <$node_type>::on_invoice_events(&this, params, callback).await
                }).await.unwrap()
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
    pub use lib::{NwcConfig, NwcNode};
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

pub mod speed {
    pub mod api;
    pub mod lib;
    pub mod types;
    pub use lib::{SpeedConfig, SpeedNode};
}

pub mod lnbits {
    pub mod api;
    pub mod lib;
    pub mod types;
    pub use lib::{LnBitsConfig, LnBitsNode};
}

pub mod types;
pub use types::*;

pub mod utils;
pub use utils::*;

pub mod database;
pub use database::{Db, DbError, Payment};

// Make an HTTP request to get IP address and simulate latency with optional SOCKS5 proxy
#[cfg_attr(feature = "uniffi", uniffi::export(async_runtime = "tokio"))]
pub async fn say_after_with_tokio(ms: u16, who: String, url: String, socks5_proxy: Option<String>, header_key: Option<String>, header_value: Option<String>) -> String {
    // Create HTTP client with optional SOCKS5 proxy
    let client = if let Some(proxy_url) = socks5_proxy {
        // Ignore certificate errors when using SOCKS5 proxy
        let client_builder = reqwest::Client::builder().danger_accept_invalid_certs(true);
        
        match reqwest::Proxy::all(&proxy_url) {
            Ok(proxy) => {
                match client_builder.proxy(proxy).build() {
                    Ok(client) => client,
                    Err(_) => reqwest::Client::new() // Fallback to default client on error
                }
            }
            Err(_) => reqwest::Client::new() // Fallback to default client on error
        }
    } else {
        reqwest::Client::builder().build().unwrap_or_else(|_| reqwest::Client::new())
    };
    
    // Create request with optional header
    let mut request = client.get(&url);
    
    if let (Some(key), Some(value)) = (header_key, header_value) {
        request = request.header(&key, &value);
    }
    
    // Make HTTP request
    let ip_result = request.send().await;
    
    let page_content = match ip_result {
        Ok(response) => {
            match response.text().await {
                Ok(html) => html,
                Err(_) => "Failed to read response text".to_string()
            }
        }
        Err(_) => "Failed to make HTTP request".to_string()
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
#[cfg_attr(feature = "uniffi", uniffi::export)]
pub fn create_nwc_node(config: nwc::NwcConfig) -> Arc<dyn LightningNode> {
    Arc::new(nwc::NwcNode::new(config))
}

#[cfg(feature = "uniffi")]
uniffi::setup_scaffolding!();
