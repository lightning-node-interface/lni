#[cfg(feature = "napi_rs")]
use napi_derive::napi;

use serde::{Deserialize, Serialize};

#[cfg_attr(feature = "napi_rs", napi(object))]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
#[derive(Debug, Clone)]
pub struct SpeedConfig {
    pub base_url: String,
    pub api_key: String,
    #[cfg_attr(feature = "uniffi", uniffi(default = Some(30)))]
    pub http_timeout: Option<i64>,
}

impl Default for SpeedConfig {
    fn default() -> Self {
        Self {
            base_url: "https://api.tryspeed.com".to_string(),
            api_key: "".to_string(),
            http_timeout: Some(30),
        }
    }
}

// Speed Payment object based on actual API response
#[derive(Debug, Deserialize, Serialize)]
pub struct SpeedPayment {
    pub id: String,
    pub object: String, // "payment"
    pub status: String, // "paid", "pending", "failed", etc.
    pub currency: String, // "SATS"
    pub amount: f64,
    pub conversion: Option<f64>,
    pub exchange_rate: Option<f64>,
    pub target_currency: Option<String>,
    pub target_amount: Option<f64>,
    pub target_amount_paid: Option<f64>,
    pub target_amount_paid_at: Option<i64>, // Unix timestamp in millis
    pub target_amount_paid_by: Option<String>,
    pub payment_method_paid_by: Option<String>, // "lightning"
    pub confirmations: Option<i64>,
    pub payment_methods: Option<Vec<String>>,
    pub payment_method_options: Option<serde_json::Value>, // Complex nested object
    pub transfers: Option<Vec<serde_json::Value>>,
    pub ttl: Option<i64>,
    pub expires_at: Option<i64>, // Unix timestamp in millis
    pub statement_descriptor: Option<String>,
    pub metadata: Option<serde_json::Value>,
    pub speed_fee: Option<SpeedFee>,
    pub payment_request_paid_by: Option<String>,
    pub net_target_amount_paid: Option<f64>,
    pub created: i64, // Unix timestamp in millis
    pub modified: Option<i64>, // Unix timestamp in millis
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SpeedFee {
    pub percentage: Option<f64>,
    pub amount: Option<f64>,
}

// Response for payment list endpoint
#[derive(Debug, Deserialize, Serialize)]
pub struct SpeedPaymentList {
    pub object: String, // "list"
    pub data: Vec<SpeedPayment>,
    pub has_more: bool,
}

// Response for creating a payment
#[derive(Debug, Deserialize, Serialize)]
pub struct SpeedCreatePaymentResponse {
    #[serde(flatten)]
    pub payment: SpeedPayment,
}

// Request for creating a payment
#[derive(Debug, Serialize)]
pub struct SpeedCreatePaymentRequest {
    pub amount: f64,
    pub currency: String,
    pub memo: Option<String>,
    pub external_id: Option<String>,
}

// Request for paying an invoice using Speed's instant send endpoint
#[derive(Debug, Serialize)]
pub struct SpeedPayInvoiceRequest {
    pub amount: f64,
    pub currency: String,
    pub target_currency: String,
    pub withdraw_method: String,
    pub withdraw_request: String,
    pub note: Option<String>,
    pub external_id: Option<String>,
}

// Request for filtering send transactions
#[derive(Debug, Serialize)]
pub struct SpeedSendFilterRequest {
    pub status: Option<Vec<String>>,
    pub withdraw_request: Option<String>,
}

// Response from Speed's send filter endpoint
#[derive(Debug, Deserialize)]
pub struct SpeedSendFilterResponse {
    pub has_more: bool,
    pub object: String,
    pub data: Vec<SpeedSendResponse>,
}
#[derive(Debug, Deserialize)]
pub struct SpeedSendResponse {
    pub id: String,
    pub object: String,
    pub status: String,
    pub withdraw_id: String,
    pub amount: f64,
    pub currency: String,
    pub target_amount: f64,
    pub target_currency: String,
    pub fees: Option<i64>,
    pub speed_fee: SpeedSendFee,
    pub exchange_rate: f64,
    pub conversion: f64,
    pub withdraw_method: String,
    pub withdraw_request: String,
    pub withdraw_type: String,
    pub note: Option<String>,
    pub failure_reason: Option<String>,
    pub explorer_link: Option<String>,
    pub promo_code: Option<String>,
    #[serde(rename = "contactPaymentAddressId")]
    pub contact_payment_address_id: Option<String>,
    pub conversion_fee: Option<String>,
    pub created: i64,
    pub modified: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct SpeedSendFee {
    pub percentage: i64,
    pub amount: i64,
}

// Error response from Speed API
#[derive(Debug, Deserialize, Serialize)]
pub struct SpeedError {
    pub error: SpeedErrorDetail,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SpeedErrorDetail {
    pub code: String,
    pub message: String,
    pub param: Option<String>,
    pub r#type: String, // "invalid_request_error", "api_error", etc.
}
