use serde::{Deserialize, Serialize};

// LNBits API response types based on the Payments API

#[derive(Debug, Deserialize, Serialize)]
pub struct CreateInvoiceRequest {
    pub out: bool,
    pub amount: i64,
    pub memo: Option<String>,
    pub unit: String,
    pub expiry: Option<i64>,
    pub webhook: Option<String>,
    pub internal: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct CreateInvoiceResponse {
    pub payment_hash: String,
    pub payment_request: String,
    pub checking_id: String,
    pub lnurl_response: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PaymentStatus {
    pub paid: bool,
    pub preimage: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Payment {
    pub payment_hash: String,
    pub payment_request: String,
    pub checking_id: String,
    pub amount: i64,
    pub fee: Option<i64>,
    pub memo: Option<String>,
    pub time: i64,
    pub bolt11: String,
    pub preimage: Option<String>,
    pub pending: bool,
    pub expiry: Option<i64>,
    pub extra: Option<serde_json::Value>,
    pub wallet_id: String,
    pub webhook: Option<String>,
    pub webhook_status: Option<i32>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PayInvoiceRequest {
    pub out: bool,
    pub bolt11: String,
}

#[derive(Debug, Deserialize)]
pub struct LnBitsPayInvoiceResponse {
    pub payment_hash: String,
    pub checking_id: String,
}

#[derive(Debug, Deserialize)]
pub struct WalletDetails {
    pub id: String,
    pub name: String,
    pub user: String,
    pub adminkey: String,
    pub inkey: String,
    pub balance_msat: i64,
}

#[derive(Debug, Deserialize)]
pub struct ApiInfo {
    pub version: String,
    pub node: Option<String>,
    pub network: Option<String>,
    pub lightning_implementation: Option<String>,
}

// Error response from LNBits API
#[derive(Debug, Deserialize)]
pub struct LnBitsError {
    pub detail: String,
}

impl std::fmt::Display for LnBitsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "LNBits API Error: {}", self.detail)
    }
}

impl std::error::Error for LnBitsError {}