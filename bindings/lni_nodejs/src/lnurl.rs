//! LNURL support for Node.js bindings
//!
//! Exposes LNURL resolution functions to JavaScript

use napi_derive::napi;

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
