#[cfg(feature = "napi_rs")]
use napi_derive::napi;
#[cfg(feature = "napi_rs")]
use napi::bindgen_prelude::*;

use std::time::Duration;

#[cfg_attr(feature = "uniffi", derive(uniffi::Object))]
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
    // Re-export the standalone sync function for uniffi
    pub use api::lnd_get_info_sync;
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

pub mod types;
pub use types::*;

pub mod utils;
pub use utils::*;

pub mod database;
pub use database::{Db, DbError, Payment};

// Re-export standalone functions at crate level for uniffi
pub use lnd::api::lnd_get_info_sync;

// Make an HTTP request to get IP address and simulate latency with optional SOCKS5 proxy
#[uniffi::export(async_runtime = "tokio")]
pub async fn say_after_with_tokio(ms: u16, who: String, url: String, socks5_proxy: Option<String>) -> String {
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
    
    // Make HTTP request to get IP address
    let ip_result = client
        .get(&url)
        .send()
        .await;
    
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

#[cfg(feature = "uniffi")]
uniffi::setup_scaffolding!();
