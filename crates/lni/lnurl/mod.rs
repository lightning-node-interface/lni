//! LNURL support for Lightning Address and LNURL-pay
//!
//! Implements:
//! - Lightning Address (user@domain) → LNURL-pay
//! - LNURL-pay (lnurl1...) → BOLT11 invoice

use serde::{Deserialize, Serialize};
use crate::ApiError;

/// LNURL-pay response from the service
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LnurlPayResponse {
    pub callback: String,
    pub max_sendable: i64,  // msats
    pub min_sendable: i64,  // msats
    pub metadata: String,
    pub tag: String,
    #[serde(default)]
    pub allows_nostr: Option<bool>,
    #[serde(default)]
    pub nostr_pubkey: Option<String>,
}

/// Response when requesting invoice from callback
#[derive(Debug, Deserialize)]
pub struct LnurlInvoiceResponse {
    pub pr: String,  // BOLT11 invoice
    #[serde(default)]
    pub routes: Option<Vec<serde_json::Value>>,
}

/// Error response from LNURL service
#[derive(Debug, Deserialize)]
pub struct LnurlErrorResponse {
    pub status: String,
    pub reason: String,
}

/// Detect the type of payment destination
#[derive(Debug, Clone, PartialEq)]
pub enum PaymentDestination {
    Bolt11(String),
    Bolt12(String),
    LnurlPay(String),
    LightningAddress { user: String, domain: String },
}

impl PaymentDestination {
    /// Parse a payment destination string and detect its type
    pub fn parse(input: &str) -> Result<Self, ApiError> {
        let input = input.trim();
        
        // Lightning Address: user@domain
        if input.contains('@') && !input.starts_with("lnurl") {
            let parts: Vec<&str> = input.split('@').collect();
            if parts.len() == 2 && !parts[0].is_empty() && !parts[1].is_empty() {
                return Ok(PaymentDestination::LightningAddress {
                    user: parts[0].to_string(),
                    domain: parts[1].to_string(),
                });
            }
            return Err(ApiError::InvalidInput("Invalid Lightning Address format".to_string()));
        }
        
        let lower = input.to_lowercase();
        
        // BOLT11: lnbc, lntb, lntbs (mainnet, testnet, signet)
        if lower.starts_with("lnbc") || lower.starts_with("lntb") || lower.starts_with("lntbs") {
            return Ok(PaymentDestination::Bolt11(input.to_string()));
        }
        
        // BOLT12 offer: lno1
        if lower.starts_with("lno1") {
            return Ok(PaymentDestination::Bolt12(input.to_string()));
        }
        
        // LNURL: lnurl1
        if lower.starts_with("lnurl1") {
            return Ok(PaymentDestination::LnurlPay(input.to_string()));
        }
        
        Err(ApiError::InvalidInput(format!(
            "Unknown payment destination format. Expected: BOLT11 (lnbc...), BOLT12 (lno1...), LNURL (lnurl1...), or Lightning Address (user@domain)"
        )))
    }
}

/// Resolve a Lightning Address to its LNURL endpoint
pub fn lightning_address_to_url(user: &str, domain: &str) -> String {
    format!("https://{}/.well-known/lnurlp/{}", domain, user)
}

/// Decode a bech32-encoded LNURL to its URL
pub fn decode_lnurl(lnurl: &str) -> Result<String, ApiError> {
    let lnurl_lower = lnurl.to_lowercase();
    
    // Try to decode as bech32
    let (hrp, data) = bech32::decode(&lnurl_lower)
        .map_err(|e| ApiError::InvalidInput(format!("Invalid LNURL encoding: {}", e)))?;
    
    if hrp.to_string() != "lnurl" {
        return Err(ApiError::InvalidInput("LNURL must have 'lnurl' prefix".to_string()));
    }
    
    // bech32 0.11 returns Vec<u8> directly (already 8-bit)
    String::from_utf8(data)
        .map_err(|e| ApiError::InvalidInput(format!("LNURL contains invalid UTF-8: {}", e)))
}

/// Fetch LNURL-pay metadata from a URL
pub async fn fetch_lnurl_pay(url: &str) -> Result<LnurlPayResponse, ApiError> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| ApiError::NetworkError(e.to_string()))?;
    
    let response = client
        .get(url)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| ApiError::NetworkError(format!("Failed to fetch LNURL: {}", e)))?;
    
    let text = response
        .text()
        .await
        .map_err(|e| ApiError::NetworkError(format!("Failed to read LNURL response: {}", e)))?;
    
    // Check for error response
    if let Ok(error) = serde_json::from_str::<LnurlErrorResponse>(&text) {
        if error.status == "ERROR" {
            return Err(ApiError::LnurlError(error.reason));
        }
    }
    
    serde_json::from_str(&text)
        .map_err(|e| ApiError::InvalidInput(format!("Invalid LNURL-pay response: {} - {}", e, &text[..text.len().min(200)])))
}

/// Request an invoice from LNURL-pay callback
pub async fn request_invoice(callback_url: &str, amount_msats: i64) -> Result<String, ApiError> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| ApiError::NetworkError(e.to_string()))?;
    
    // Add amount to callback URL
    let url = if callback_url.contains('?') {
        format!("{}&amount={}", callback_url, amount_msats)
    } else {
        format!("{}?amount={}", callback_url, amount_msats)
    };
    
    let response = client
        .get(&url)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| ApiError::NetworkError(format!("Failed to request invoice: {}", e)))?;
    
    let text = response
        .text()
        .await
        .map_err(|e| ApiError::NetworkError(format!("Failed to read invoice response: {}", e)))?;
    
    // Check for error response
    if let Ok(error) = serde_json::from_str::<LnurlErrorResponse>(&text) {
        if error.status == "ERROR" {
            return Err(ApiError::LnurlError(error.reason));
        }
    }
    
    let invoice_resp: LnurlInvoiceResponse = serde_json::from_str(&text)
        .map_err(|e| ApiError::InvalidInput(format!("Invalid invoice response: {} - {}", e, &text[..text.len().min(200)])))?;
    
    Ok(invoice_resp.pr)
}

/// Resolve any payment destination to a BOLT11 invoice
/// 
/// This handles:
/// - BOLT11: Returns as-is
/// - Lightning Address: Fetches LNURL endpoint, requests invoice
/// - LNURL: Decodes, fetches endpoint, requests invoice
/// - BOLT12: Returns error (not supported in this flow, use pay_offer)
pub async fn resolve_to_bolt11(
    destination: &str,
    amount_msats: Option<i64>,
) -> Result<String, ApiError> {
    let parsed = PaymentDestination::parse(destination)?;
    
    match parsed {
        PaymentDestination::Bolt11(invoice) => Ok(invoice),
        
        PaymentDestination::Bolt12(_) => {
            Err(ApiError::InvalidInput(
                "BOLT12 offers require amount and should use pay_offer method".to_string()
            ))
        }
        
        PaymentDestination::LightningAddress { user, domain } => {
            let amount = amount_msats.ok_or_else(|| {
                ApiError::InvalidInput("Lightning Address requires amount_msats".to_string())
            })?;
            
            let url = lightning_address_to_url(&user, &domain);
            let lnurl_data = fetch_lnurl_pay(&url).await?;
            
            // Validate amount
            if amount < lnurl_data.min_sendable {
                return Err(ApiError::InvalidInput(format!(
                    "Amount {} msats is below minimum {} msats",
                    amount, lnurl_data.min_sendable
                )));
            }
            if amount > lnurl_data.max_sendable {
                return Err(ApiError::InvalidInput(format!(
                    "Amount {} msats exceeds maximum {} msats",
                    amount, lnurl_data.max_sendable
                )));
            }
            
            request_invoice(&lnurl_data.callback, amount).await
        }
        
        PaymentDestination::LnurlPay(lnurl) => {
            let amount = amount_msats.ok_or_else(|| {
                ApiError::InvalidInput("LNURL requires amount_msats".to_string())
            })?;
            
            let url = decode_lnurl(&lnurl)?;
            let lnurl_data = fetch_lnurl_pay(&url).await?;
            
            // Validate amount
            if amount < lnurl_data.min_sendable {
                return Err(ApiError::InvalidInput(format!(
                    "Amount {} msats is below minimum {} msats",
                    amount, lnurl_data.min_sendable
                )));
            }
            if amount > lnurl_data.max_sendable {
                return Err(ApiError::InvalidInput(format!(
                    "Amount {} msats exceeds maximum {} msats",
                    amount, lnurl_data.max_sendable
                )));
            }
            
            request_invoice(&lnurl_data.callback, amount).await
        }
    }
}

/// Check if invoice needs LNURL resolution
pub fn needs_resolution(invoice: &str) -> bool {
    let lower = invoice.to_lowercase().trim().to_string();
    invoice.contains('@') || lower.starts_with("lnurl1")
}

/// Get info about what type of payment this is (for confirmation flows)
pub async fn get_payment_info(
    destination: &str,
    amount_msats: Option<i64>,
) -> Result<PaymentInfo, ApiError> {
    let parsed = PaymentDestination::parse(destination)?;
    
    match parsed {
        PaymentDestination::Bolt11(invoice) => {
            // TODO: Could decode invoice to get amount
            Ok(PaymentInfo {
                destination_type: "bolt11".to_string(),
                destination: destination.to_string(),
                amount_msats,
                min_sendable_msats: None,
                max_sendable_msats: None,
                description: None,
            })
        }
        
        PaymentDestination::Bolt12(offer) => {
            Ok(PaymentInfo {
                destination_type: "bolt12".to_string(),
                destination: destination.to_string(),
                amount_msats,
                min_sendable_msats: None,
                max_sendable_msats: None,
                description: None,
            })
        }
        
        PaymentDestination::LightningAddress { user, domain } => {
            let url = lightning_address_to_url(&user, &domain);
            let lnurl_data = fetch_lnurl_pay(&url).await?;
            
            let description = lnurl_data.metadata.clone();
            
            Ok(PaymentInfo {
                destination_type: "lightning_address".to_string(),
                destination: destination.to_string(),
                amount_msats,
                min_sendable_msats: Some(lnurl_data.min_sendable),
                max_sendable_msats: Some(lnurl_data.max_sendable),
                description: Some(description),
            })
        }
        
        PaymentDestination::LnurlPay(lnurl) => {
            let url = decode_lnurl(&lnurl)?;
            let lnurl_data = fetch_lnurl_pay(&url).await?;
            
            Ok(PaymentInfo {
                destination_type: "lnurl".to_string(),
                destination: destination.to_string(),
                amount_msats,
                min_sendable_msats: Some(lnurl_data.min_sendable),
                max_sendable_msats: Some(lnurl_data.max_sendable),
                description: Some(lnurl_data.metadata),
            })
        }
    }
}

/// Payment info for confirmation flows
#[derive(Debug, Clone)]
pub struct PaymentInfo {
    pub destination_type: String,
    pub destination: String,
    pub amount_msats: Option<i64>,
    pub min_sendable_msats: Option<i64>,
    pub max_sendable_msats: Option<i64>,
    pub description: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_parse_bolt11() {
        let result = PaymentDestination::parse("lnbc10u1ptest");
        assert!(matches!(result, Ok(PaymentDestination::Bolt11(_))));
    }
    
    #[test]
    fn test_parse_bolt12() {
        let result = PaymentDestination::parse("lno1qtest");
        assert!(matches!(result, Ok(PaymentDestination::Bolt12(_))));
    }
    
    #[test]
    fn test_parse_lightning_address() {
        let result = PaymentDestination::parse("test@example.com");
        assert!(matches!(result, Ok(PaymentDestination::LightningAddress { .. })));
        
        if let Ok(PaymentDestination::LightningAddress { user, domain }) = result {
            assert_eq!(user, "test");
            assert_eq!(domain, "example.com");
        }
    }
    
    #[test]
    fn test_lightning_address_to_url() {
        let url = lightning_address_to_url("nick", "strike.me");
        assert_eq!(url, "https://strike.me/.well-known/lnurlp/nick");
    }
}
