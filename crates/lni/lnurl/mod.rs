//! LNURL support for Lightning Address and LNURL-pay
//!
//! Implements:
//! - Lightning Address (user@domain) → LNURL-pay
//! - LNURL-pay (lnurl1...) → BOLT11 invoice

use crate::ApiError;
use lightning_invoice::Bolt11Invoice;
use serde::Deserialize;
use std::str::FromStr;

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

#[derive(Debug, Deserialize)]
struct LnurlVerifyPayResponse {
    pub pr: String,
    pub verify: String,
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
        let lower = input.to_lowercase();
        
        // Lightning Address: user@domain (but not LNURL which may contain @)
        if input.contains('@') && !lower.starts_with("lnurl") {
            let parts: Vec<&str> = input.split('@').collect();
            if parts.len() == 2 && !parts[0].is_empty() && !parts[1].is_empty() {
                return Ok(PaymentDestination::LightningAddress {
                    user: parts[0].to_string(),
                    domain: parts[1].to_string(),
                });
            }
            return Err(ApiError::InvalidInput("Invalid Lightning Address format".to_string()));
        }
        
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
    
    validate_invoice_amount(&invoice_resp.pr, amount_msats)?;

    Ok(invoice_resp.pr)
}

fn handle_lnurl_error_value(value: &serde_json::Value) -> Result<(), ApiError> {
    let Some(object) = value.as_object() else {
        return Ok(());
    };

    if object
        .get("status")
        .and_then(|status| status.as_str())
        .map(|status| status.eq_ignore_ascii_case("ERROR"))
        .unwrap_or(false)
    {
        if let Some(reason) = object.get("reason").and_then(|reason| reason.as_str()) {
            return Err(ApiError::LnurlError(reason.to_string()));
        }
    }

    if object
        .get("error")
        .and_then(|error| error.as_bool())
        .unwrap_or(false)
    {
        if let Some(message) = object.get("message").and_then(|message| message.as_str()) {
            return Err(ApiError::LnurlError(message.to_string()));
        }
    }

    Ok(())
}

fn handle_lnurl_ok_value(value: &serde_json::Value, endpoint_label: &str) -> Result<(), ApiError> {
    handle_lnurl_error_value(value)?;

    let status_ok = value
        .as_object()
        .and_then(|object| object.get("status"))
        .and_then(|status| status.as_str())
        .map(|status| status == "OK")
        .unwrap_or(false);
    if status_ok {
        Ok(())
    } else {
        Err(ApiError::InvalidInput(format!("{} response status is not OK", endpoint_label)))
    }
}

fn callback_url_with_amount(callback_url: &str, amount_msats: i64) -> Result<String, ApiError> {
    let mut url = reqwest::Url::parse(callback_url)
        .map_err(|e| ApiError::InvalidInput(format!("Invalid LNURL callback URL: {}", e)))?;
    url.query_pairs_mut().append_pair("amount", &amount_msats.to_string());
    Ok(url.to_string())
}

async fn fetch_lnurl_json_value(url: &str) -> Result<serde_json::Value, ApiError> {
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

    let value = serde_json::from_str::<serde_json::Value>(&text)
        .map_err(|e| ApiError::InvalidInput(format!("Invalid LNURL JSON response: {} - {}", e, &text[..text.len().min(200)])))?;
    handle_lnurl_error_value(&value)?;
    Ok(value)
}

/// Verify whether a Lightning Address LNURL-pay endpoint supports LNURL-verify.
pub async fn verify_lightning_address_pay_request(lightning_address: &str) -> Result<(), ApiError> {
    let PaymentDestination::LightningAddress { user, domain } = PaymentDestination::parse(lightning_address)? else {
        return Err(ApiError::InvalidInput("Expected Lightning Address".to_string()));
    };
    let well_known = fetch_lnurl_pay(&lightning_address_to_url(&user, &domain)).await?;
    let amount_msats = std::cmp::min(
        std::cmp::max(100_000, well_known.min_sendable),
        well_known.max_sendable,
    );

    if amount_msats < well_known.min_sendable || amount_msats > well_known.max_sendable {
        return Err(ApiError::InvalidInput("Invalid LNURL sendable amount range".to_string()));
    }

    let callback_response = fetch_lnurl_json_value(&callback_url_with_amount(&well_known.callback, amount_msats)?).await?;
    let verify_response: LnurlVerifyPayResponse = serde_json::from_value(callback_response)
        .map_err(|_| ApiError::InvalidInput("LNURL-verify endpoint is not supported".to_string()))?;

    if verify_response.pr.is_empty() || verify_response.verify.is_empty() {
        return Err(ApiError::InvalidInput("LNURL-verify endpoint is not supported".to_string()));
    }

    let verify_result = fetch_lnurl_json_value(&verify_response.verify).await?;
    handle_lnurl_ok_value(&verify_result, "LNURL verify")
}

pub async fn lightning_address_lnurl_verify_supported(lightning_address: &str) -> bool {
    verify_lightning_address_pay_request(lightning_address).await.is_ok()
}

fn validate_invoice_amount(invoice: &str, expected_amount_msats: i64) -> Result<(), ApiError> {
    if expected_amount_msats < 0 {
        return Err(ApiError::InvalidInput(
            "LNURL invoice amount must be non-negative".to_string(),
        ));
    }

    let invoice = Bolt11Invoice::from_str(invoice)
        .map_err(|e| ApiError::InvalidInput(format!("Invalid LNURL invoice: {}", e)))?;
    let actual_amount = invoice.amount_milli_satoshis().ok_or_else(|| {
        ApiError::InvalidInput("LNURL invoice is missing an amount".to_string())
    })?;
    let expected_amount = expected_amount_msats as u64;

    if actual_amount != expected_amount {
        return Err(ApiError::InvalidInput(format!(
            "LNURL invoice amount {} msats does not match requested amount {} msats",
            actual_amount, expected_amount
        )));
    }

    Ok(())
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
    fn test_parse_lnurl() {
        // Lowercase
        let result = PaymentDestination::parse("lnurl1test");
        assert!(matches!(result, Ok(PaymentDestination::LnurlPay(_))));
        
        // Uppercase - should NOT be parsed as Lightning Address
        let result = PaymentDestination::parse("LNURL1TEST");
        assert!(matches!(result, Ok(PaymentDestination::LnurlPay(_))));
        
        // Mixed case with @ - should still be LNURL, not Lightning Address
        let result = PaymentDestination::parse("LNURL1test@fake");
        assert!(matches!(result, Ok(PaymentDestination::LnurlPay(_))));
    }
    
    #[test]
    fn test_lightning_address_to_url() {
        let url = lightning_address_to_url("nick", "strike.me");
        assert_eq!(url, "https://strike.me/.well-known/lnurlp/nick");
    }

    #[test]
    fn test_validate_lnurl_invoice_amount_matches_requested_amount() {
        let invoice = "lnbc2500u1pvjluezsp5zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zygspp5qqqsyqcyq5rqwzqfqqqsyqcyq5rqwzqfqqqsyqcyq5rqwzqfqypqdq5xysxxatsyp3k7enxv4jsxqzpu9qrsgquk0rl77nj30yxdy8j9vdx85fkpmdla2087ne0xh8nhedh8w27kyke0lp53ut353s06fv3qfegext0eh0ymjpf39tuven09sam30g4vgpfna3rh";

        assert!(validate_invoice_amount(invoice, 250_000_000).is_ok());
    }

    #[test]
    fn test_validate_lnurl_invoice_amount_rejects_mismatch() {
        let invoice = "lnbc2500u1pvjluezsp5zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zygspp5qqqsyqcyq5rqwzqfqqqsyqcyq5rqwzqfqqqsyqcyq5rqwzqfqypqdq5xysxxatsyp3k7enxv4jsxqzpu9qrsgquk0rl77nj30yxdy8j9vdx85fkpmdla2087ne0xh8nhedh8w27kyke0lp53ut353s06fv3qfegext0eh0ymjpf39tuven09sam30g4vgpfna3rh";

        let error = validate_invoice_amount(invoice, 1_000).expect_err("amount mismatch should fail");
        assert!(format!("{:?}", error).contains("does not match requested amount"));
    }
}
