#![deny(clippy::all)]

extern crate napi_derive;
use napi_derive::napi;

pub use lni::ApiError;
pub use lni::types::*;
pub use lni::utils::*;
pub use lni::types::{Transaction, InvoiceType, ListTransactionsParams, PayInvoiceResponse};

mod phoenixd;
pub use phoenixd::PhoenixdNode;

mod cln;
pub use cln::ClnNode;

mod lnd;
pub use lnd::LndNode;

mod blink;
pub use blink::BlinkNode;

mod nwc;
pub use nwc::NwcNode;

mod strike;
pub use strike::StrikeNode;

mod speed;
pub use speed::SpeedNode;

mod spark;
pub use spark::SparkNode;

use std::time::Duration;

/// Generate a BIP39 mnemonic phrase
/// 
/// @param wordCount - Optional number of words (12 or 24). Defaults to 12.
/// @returns A space-separated mnemonic phrase
#[napi]
pub fn generate_mnemonic(word_count: Option<u8>) -> napi::Result<String> {
    use bip39::{Language, Mnemonic};
    use rand::rngs::OsRng;
    use rand::RngCore;

    let entropy_size = match word_count {
        Some(24) => 32,
        _ => 16,
    };

    let mut entropy = vec![0u8; entropy_size];
    OsRng.fill_bytes(&mut entropy);

    match Mnemonic::from_entropy_in(Language::English, &entropy) {
        Ok(mnemonic) => Ok(mnemonic.to_string()),
        Err(e) => Err(napi::Error::from_reason(format!("Failed to generate mnemonic: {}", e))),
    }
}

// Make an HTTP request to get IP address and simulate latency with optional SOCKS5 proxy
#[napi]
pub async fn say_after_with_tokio(ms: u16, who: String, url: String, socks5_proxy: Option<String>, header_key: Option<String>, header_value: Option<String>) -> napi::Result<String> {
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
    
    
    Ok(format!("Hello, {who}! Your IP address is: {page_content} (with Tokio after {ms}ms delay)"))
}
// =============================================================================
// LNURL Support
// =============================================================================

/// Payment destination info for confirmation flows
#[napi(object)]
pub struct PaymentInfo {
    pub destination_type: String,
    pub destination: String,
    pub amount_msats: Option<i64>,
    pub min_sendable_msats: Option<i64>,
    pub max_sendable_msats: Option<i64>,
    pub description: Option<String>,
}

/// Check what type of payment destination this is
/// Returns: "bolt11", "bolt12", "lnurl", or "lightning_address"
#[napi]
pub fn detect_payment_type(destination: String) -> napi::Result<String> {
    match lni::lnurl::PaymentDestination::parse(&destination) {
        Ok(dest) => match dest {
            lni::lnurl::PaymentDestination::Bolt11(_) => Ok("bolt11".to_string()),
            lni::lnurl::PaymentDestination::Bolt12(_) => Ok("bolt12".to_string()),
            lni::lnurl::PaymentDestination::LnurlPay(_) => Ok("lnurl".to_string()),
            lni::lnurl::PaymentDestination::LightningAddress { .. } => Ok("lightning_address".to_string()),
        },
        Err(e) => Err(napi::Error::from_reason(format!("{}", e))),
    }
}

/// Check if a payment destination needs LNURL resolution
/// (Lightning Address or LNURL need to be resolved to BOLT11 first)
#[napi]
pub fn needs_resolution(destination: String) -> bool {
    lni::lnurl::needs_resolution(&destination)
}

/// Resolve any payment destination to a BOLT11 invoice
/// - BOLT11: Returns as-is
/// - Lightning Address: Fetches LNURL endpoint, requests invoice
/// - LNURL: Decodes, fetches endpoint, requests invoice
/// - BOLT12: Returns error (use pay_offer instead)
#[napi]
pub async fn resolve_to_bolt11(destination: String, amount_msats: Option<i64>) -> napi::Result<String> {
    lni::lnurl::resolve_to_bolt11(&destination, amount_msats)
        .await
        .map_err(|e| napi::Error::from_reason(format!("{}", e)))
}

/// Get payment info for a destination (for confirmation flows)
/// Fetches LNURL metadata if needed to get min/max amounts
#[napi]
pub async fn get_payment_info(destination: String, amount_msats: Option<i64>) -> napi::Result<PaymentInfo> {
    let info = lni::lnurl::get_payment_info(&destination, amount_msats)
        .await
        .map_err(|e| napi::Error::from_reason(format!("{}", e)))?;
    
    Ok(PaymentInfo {
        destination_type: info.destination_type,
        destination: info.destination,
        amount_msats: info.amount_msats,
        min_sendable_msats: info.min_sendable_msats,
        max_sendable_msats: info.max_sendable_msats,
        description: info.description,
    })
}
