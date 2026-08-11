//! LNURL support for Lightning Address and LNURL-pay
//!
//! Implements:
//! - Lightning Address (user@domain) → LNURL-pay
//! - LNURL-pay (lnurl1...) → BOLT11 invoice

use crate::ApiError;
use lightning_invoice::Bolt11Invoice;
use serde::Deserialize;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::str::FromStr;

/// LNURL-pay response from the service
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LnurlPayResponse {
    pub callback: String,
    pub max_sendable: i64, // msats
    pub min_sendable: i64, // msats
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
    pub pr: String, // BOLT11 invoice
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
            return Err(ApiError::InvalidInput(
                "Invalid Lightning Address format".to_string(),
            ));
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
        return Err(ApiError::InvalidInput(
            "LNURL must have 'lnurl' prefix".to_string(),
        ));
    }

    // bech32 0.11 returns Vec<u8> directly (already 8-bit)
    String::from_utf8(data)
        .map_err(|e| ApiError::InvalidInput(format!("LNURL contains invalid UTF-8: {}", e)))
}

fn validate_public_https_url(raw: &str) -> Result<reqwest::Url, ApiError> {
    let url = reqwest::Url::parse(raw)
        .map_err(|e| ApiError::InvalidInput(format!("Invalid LNURL URL: {}", e)))?;

    if url.scheme() != "https" {
        return Err(ApiError::InvalidInput(
            "LNURL endpoints must use HTTPS".to_string(),
        ));
    }

    if !url.username().is_empty() || url.password().is_some() {
        return Err(ApiError::InvalidInput(
            "LNURL endpoints must not include credentials".to_string(),
        ));
    }

    let Some(hostname) = url.host_str() else {
        return Err(ApiError::InvalidInput(
            "Invalid LNURL URL: missing hostname".to_string(),
        ));
    };
    if is_private_or_local_hostname(hostname) {
        return Err(ApiError::InvalidInput(
            "LNURL endpoints must use a public hostname".to_string(),
        ));
    }

    Ok(url)
}

fn is_private_or_local_hostname(hostname: &str) -> bool {
    let host = hostname
        .trim_matches(|char| char == '[' || char == ']')
        .trim_end_matches('.')
        .to_ascii_lowercase();

    if host.is_empty()
        || host == "localhost"
        || host.ends_with(".localhost")
        || host.ends_with(".local")
        || host.ends_with(".internal")
    {
        return true;
    }

    match host.parse::<IpAddr>() {
        Ok(ip) => !is_public_ip(ip),
        Err(_) => false,
    }
}

fn is_public_ipv4(ip: Ipv4Addr) -> bool {
    let [first, second, third, _] = ip.octets();
    !(first == 0
        || first == 10
        || first == 127
        || (first == 100 && (64..=127).contains(&second))
        || (first == 169 && second == 254)
        || (first == 172 && (16..=31).contains(&second))
        || (first == 192 && second == 0 && third == 0)
        || (first == 192 && second == 0 && third == 2)
        || (first == 192 && second == 88 && third == 99)
        || (first == 192 && second == 168)
        || (first == 198 && (18..=19).contains(&second))
        || (first == 198 && second == 51 && third == 100)
        || (first == 203 && second == 0 && third == 113)
        || first >= 224)
}

fn is_public_ipv6(ip: Ipv6Addr) -> bool {
    if let Some(ipv4) = ip.to_ipv4_mapped() {
        return is_public_ipv4(ipv4);
    }

    let segments = ip.segments();
    !(ip.is_loopback()
        || ip.is_unspecified()
        || ip.is_multicast()
        || segments[0] & 0xfe00 == 0xfc00
        || segments[0] & 0xffc0 == 0xfe80
        || segments[0] & 0xffc0 == 0xfec0
        || (segments[0] == 0x0064 && segments[1] == 0xff9b)
        || (segments[0] == 0x0100 && segments[1] == 0 && segments[2] == 0 && segments[3] == 0)
        || (segments[0] == 0x2001 && segments[1] == 0)
        || (segments[0] == 0x2001 && segments[1] == 2)
        || (segments[0] == 0x2001 && segments[1] == 0x0db8)
        || segments[0] == 0x2002)
}

fn is_public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => is_public_ipv4(ip),
        IpAddr::V6(ip) => is_public_ipv6(ip),
    }
}

fn validate_resolved_lnurl_addresses(addresses: &[SocketAddr]) -> Result<(), ApiError> {
    if addresses.is_empty() || addresses.iter().any(|address| !is_public_ip(address.ip())) {
        return Err(ApiError::InvalidInput(
            "LNURL endpoints must resolve only to public addresses".to_string(),
        ));
    }
    Ok(())
}

async fn lnurl_http_client(url: &reqwest::Url) -> Result<reqwest::Client, ApiError> {
    let hostname = url
        .host_str()
        .ok_or_else(|| ApiError::InvalidInput("Invalid LNURL URL: missing hostname".to_string()))?;
    let port = url.port_or_known_default().unwrap_or(443);
    let literal_ip = hostname
        .trim_matches(|character| character == '[' || character == ']')
        .parse::<IpAddr>()
        .ok();
    let addresses = if let Some(ip) = literal_ip {
        vec![SocketAddr::new(ip, port)]
    } else {
        tokio::net::lookup_host((hostname, port))
            .await
            .map_err(|_| ApiError::NetworkError("Failed to resolve LNURL hostname".to_string()))?
            .collect::<Vec<_>>()
    };
    validate_resolved_lnurl_addresses(&addresses)?;

    let mut builder = reqwest::Client::builder()
        .https_only(true)
        .redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(30));
    if literal_ip.is_none() {
        builder = builder.resolve_to_addrs(hostname, &addresses);
    }
    builder
        .build()
        .map_err(|e| ApiError::NetworkError(e.to_string()))
}

fn require_successful_lnurl_response(
    response: reqwest::Response,
) -> Result<reqwest::Response, ApiError> {
    if response.status().is_success() {
        Ok(response)
    } else {
        Err(ApiError::NetworkError(format!(
            "LNURL request returned HTTP status {}",
            response.status()
        )))
    }
}

/// Fetch LNURL-pay metadata from a URL
pub async fn fetch_lnurl_pay(url: &str) -> Result<LnurlPayResponse, ApiError> {
    let url = validate_public_https_url(url)?;
    let client = lnurl_http_client(&url).await?;

    let response = require_successful_lnurl_response(
        client
            .get(url)
            .header("Accept", "application/json")
            .send()
            .await
            .map_err(|e| ApiError::NetworkError(format!("Failed to fetch LNURL: {}", e)))?,
    )?;

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

    serde_json::from_str(&text).map_err(|e| {
        ApiError::InvalidInput(format!(
            "Invalid LNURL-pay response: {} - {}",
            e,
            &text[..text.len().min(200)]
        ))
    })
}

/// Request an invoice from LNURL-pay callback
pub async fn request_invoice(callback_url: &str, amount_msats: i64) -> Result<String, ApiError> {
    let url = callback_url_with_amount(callback_url, amount_msats)?;
    let client = lnurl_http_client(&url).await?;

    let response = require_successful_lnurl_response(
        client
            .get(url)
            .header("Accept", "application/json")
            .send()
            .await
            .map_err(|e| ApiError::NetworkError(format!("Failed to request invoice: {}", e)))?,
    )?;

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

    let invoice_resp: LnurlInvoiceResponse = serde_json::from_str(&text).map_err(|e| {
        ApiError::InvalidInput(format!(
            "Invalid invoice response: {} - {}",
            e,
            &text[..text.len().min(200)]
        ))
    })?;

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
        Err(ApiError::InvalidInput(format!(
            "{} response status is not OK",
            endpoint_label
        )))
    }
}

fn callback_url_with_amount(
    callback_url: &str,
    amount_msats: i64,
) -> Result<reqwest::Url, ApiError> {
    let mut url = validate_public_https_url(callback_url)?;
    url.query_pairs_mut()
        .append_pair("amount", &amount_msats.to_string());
    Ok(url)
}

async fn fetch_lnurl_json_value(url: &str) -> Result<serde_json::Value, ApiError> {
    let url = validate_public_https_url(url)?;
    let client = lnurl_http_client(&url).await?;

    let response = require_successful_lnurl_response(
        client
            .get(url)
            .header("Accept", "application/json")
            .send()
            .await
            .map_err(|e| ApiError::NetworkError(format!("Failed to fetch LNURL: {}", e)))?,
    )?;

    let text = response
        .text()
        .await
        .map_err(|e| ApiError::NetworkError(format!("Failed to read LNURL response: {}", e)))?;

    let value = serde_json::from_str::<serde_json::Value>(&text).map_err(|e| {
        ApiError::InvalidInput(format!(
            "Invalid LNURL JSON response: {} - {}",
            e,
            &text[..text.len().min(200)]
        ))
    })?;
    handle_lnurl_error_value(&value)?;
    Ok(value)
}

/// Verify whether a Lightning Address LNURL-pay endpoint supports LNURL-verify.
pub async fn verify_lightning_address_pay_request(lightning_address: &str) -> Result<(), ApiError> {
    let PaymentDestination::LightningAddress { user, domain } =
        PaymentDestination::parse(lightning_address)?
    else {
        return Err(ApiError::InvalidInput(
            "Expected Lightning Address".to_string(),
        ));
    };
    let well_known = fetch_lnurl_pay(&lightning_address_to_url(&user, &domain)).await?;
    let amount_msats = std::cmp::min(
        std::cmp::max(100_000, well_known.min_sendable),
        well_known.max_sendable,
    );

    if amount_msats < well_known.min_sendable || amount_msats > well_known.max_sendable {
        return Err(ApiError::InvalidInput(
            "Invalid LNURL sendable amount range".to_string(),
        ));
    }

    let callback_url = callback_url_with_amount(&well_known.callback, amount_msats)?;
    let callback_response = fetch_lnurl_json_value(callback_url.as_str()).await?;
    let verify_response: LnurlVerifyPayResponse = serde_json::from_value(callback_response)
        .map_err(|_| {
            ApiError::InvalidInput("LNURL-verify endpoint is not supported".to_string())
        })?;

    if verify_response.pr.is_empty() || verify_response.verify.is_empty() {
        return Err(ApiError::InvalidInput(
            "LNURL-verify endpoint is not supported".to_string(),
        ));
    }

    let verify_url = validate_public_https_url(&verify_response.verify)?;
    let verify_result = fetch_lnurl_json_value(verify_url.as_str()).await?;
    handle_lnurl_ok_value(&verify_result, "LNURL verify")
}

pub async fn lightning_address_lnurl_verify_supported(lightning_address: &str) -> bool {
    verify_lightning_address_pay_request(lightning_address)
        .await
        .is_ok()
}

fn validate_invoice_amount(invoice: &str, expected_amount_msats: i64) -> Result<(), ApiError> {
    if expected_amount_msats < 0 {
        return Err(ApiError::InvalidInput(
            "LNURL invoice amount must be non-negative".to_string(),
        ));
    }

    let invoice = Bolt11Invoice::from_str(invoice)
        .map_err(|e| ApiError::InvalidInput(format!("Invalid LNURL invoice: {}", e)))?;
    let actual_amount = invoice
        .amount_milli_satoshis()
        .ok_or_else(|| ApiError::InvalidInput("LNURL invoice is missing an amount".to_string()))?;
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

        PaymentDestination::Bolt12(_) => Err(ApiError::InvalidInput(
            "BOLT12 offers require amount and should use pay_offer method".to_string(),
        )),

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
            let amount = amount_msats
                .ok_or_else(|| ApiError::InvalidInput("LNURL requires amount_msats".to_string()))?;

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

        PaymentDestination::Bolt12(offer) => Ok(PaymentInfo {
            destination_type: "bolt12".to_string(),
            destination: destination.to_string(),
            amount_msats,
            min_sendable_msats: None,
            max_sendable_msats: None,
            description: None,
        }),

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

    #[tokio::test]
    async fn test_hardened_lnurl_http_client_builds() {
        let url = validate_public_https_url("https://93.184.216.34/lnurl")
            .expect("public HTTPS URL should be accepted");
        lnurl_http_client(&url)
            .await
            .expect("hardened LNURL HTTP client should build");
    }

    #[test]
    fn test_resolved_lnurl_addresses_must_all_be_public() {
        let public = SocketAddr::from(([93, 184, 216, 34], 443));
        let private = SocketAddr::from(([169, 254, 169, 254], 443));

        assert!(validate_resolved_lnurl_addresses(&[public]).is_ok());
        assert!(validate_resolved_lnurl_addresses(&[public, private]).is_err());
        assert!(validate_resolved_lnurl_addresses(&[]).is_err());
    }

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
        assert!(matches!(
            result,
            Ok(PaymentDestination::LightningAddress { .. })
        ));

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
    fn test_validate_public_https_url_accepts_public_https_url() {
        let url = validate_public_https_url("https://pay.example/lnurl?tag=pay")
            .expect("public HTTPS URL should be accepted");

        assert_eq!(url.as_str(), "https://pay.example/lnurl?tag=pay");
    }

    #[test]
    fn test_validate_public_https_url_rejects_unsafe_urls() {
        let cases = [
            "http://pay.example/lnurl",
            "https://user:pass@pay.example/lnurl",
            "https://localhost/lnurl",
            "https://wallet.local/lnurl",
            "https://10.0.0.1/lnurl",
            "https://127.0.0.1/lnurl",
            "https://169.254.169.254/latest/meta-data",
            "https://[::1]/lnurl",
            "https://[fd00::1]/lnurl",
        ];

        for url in cases {
            assert!(
                validate_public_https_url(url).is_err(),
                "{url} should be rejected before fetching"
            );
        }
    }

    #[test]
    fn test_callback_url_with_amount_rejects_private_callback_url() {
        let error = callback_url_with_amount("https://192.168.1.10/callback", 100_000)
            .expect_err("private callback URL should be rejected");

        assert!(format!("{:?}", error).contains("public hostname"));
    }

    #[test]
    fn test_callback_url_with_amount_preserves_existing_query() {
        let url = callback_url_with_amount("https://pay.example/callback?nonce=abc", 100_000)
            .expect("public callback URL should be accepted");

        assert_eq!(
            url.as_str(),
            "https://pay.example/callback?nonce=abc&amount=100000"
        );
    }

    #[test]
    fn test_validate_lnurl_invoice_amount_matches_requested_amount() {
        let invoice = "lnbc2500u1pvjluezsp5zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zygspp5qqqsyqcyq5rqwzqfqqqsyqcyq5rqwzqfqqqsyqcyq5rqwzqfqypqdq5xysxxatsyp3k7enxv4jsxqzpu9qrsgquk0rl77nj30yxdy8j9vdx85fkpmdla2087ne0xh8nhedh8w27kyke0lp53ut353s06fv3qfegext0eh0ymjpf39tuven09sam30g4vgpfna3rh";

        assert!(validate_invoice_amount(invoice, 250_000_000).is_ok());
    }

    #[test]
    fn test_validate_lnurl_invoice_amount_rejects_mismatch() {
        let invoice = "lnbc2500u1pvjluezsp5zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zygspp5qqqsyqcyq5rqwzqfqqqsyqcyq5rqwzqfqqqsyqcyq5rqwzqfqypqdq5xysxxatsyp3k7enxv4jsxqzpu9qrsgquk0rl77nj30yxdy8j9vdx85fkpmdla2087ne0xh8nhedh8w27kyke0lp53ut353s06fv3qfegext0eh0ymjpf39tuven09sam30g4vgpfna3rh";

        let error =
            validate_invoice_amount(invoice, 1_000).expect_err("amount mismatch should fail");
        assert!(format!("{:?}", error).contains("does not match requested amount"));
    }
}
