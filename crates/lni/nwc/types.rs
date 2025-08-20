use serde::{Deserialize, Serialize};

// NWC Request types
#[derive(Debug, Serialize, Deserialize)]
pub struct NwcRequest {
    pub id: String,
    pub method: String,
    pub params: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NwcResponse {
    pub result_type: String,
    pub error: Option<NwcError>,
    pub result: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NwcError {
    pub code: String,
    pub message: String,
}

// NWC Method specific request/response types
#[derive(Debug, Serialize, Deserialize)]
pub struct GetInfoRequest {
    // Empty for get_info
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetInfoResponse {
    pub alias: String,
    pub color: String,
    pub pubkey: String,
    pub network: String,
    pub block_height: u64,
    pub block_hash: String,
    pub methods: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MakeInvoiceRequest {
    pub amount: Option<i64>, // in msats
    pub description: Option<String>,
    pub description_hash: Option<String>,
    pub expiry: Option<i64>, // in seconds
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MakeInvoiceResponse {
    #[serde(rename = "type")]
    pub type_: String,
    pub invoice: String,
    pub description: Option<String>,
    pub description_hash: Option<String>,
    pub preimage: Option<String>,
    pub payment_hash: String,
    pub amount: i64, // in msats
    pub fees_paid: i64, // in msats
    pub created_at: i64, // unix timestamp
    pub expires_at: i64, // unix timestamp
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PayInvoiceRequest {
    pub invoice: String,
    pub amount: Option<i64>, // in msats, for zero-amount invoices
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PayInvoiceResponse {
    pub preimage: String,
    pub payment_hash: String,
    pub fees_paid: i64, // in msats
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PayKeysendRequest {
    pub pubkey: String,
    pub amount: i64, // in msats
    pub preimage: Option<String>,
    pub tlv_records: Option<Vec<TlvRecord>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TlvRecord {
    #[serde(rename = "type")]
    pub type_: i64,
    pub value: String, // hex-encoded
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PayKeysendResponse {
    pub preimage: String,
    pub payment_hash: String,
    pub fees_paid: i64, // in msats
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LookupInvoiceRequest {
    pub payment_hash: Option<String>,
    pub invoice: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LookupInvoiceResponse {
    #[serde(rename = "type")]
    pub type_: String,
    pub invoice: Option<String>,
    pub description: Option<String>,
    pub description_hash: Option<String>,
    pub preimage: Option<String>,
    pub payment_hash: String,
    pub amount: i64, // in msats
    pub fees_paid: i64, // in msats
    pub created_at: i64, // unix timestamp
    pub expires_at: i64, // unix timestamp
    pub settled_at: Option<i64>, // unix timestamp
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ListTransactionsRequest {
    pub from: Option<i64>, // unix timestamp
    pub until: Option<i64>, // unix timestamp
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    pub unpaid: Option<bool>,
    #[serde(rename = "type")]
    pub type_: Option<String>, // "incoming" or "outgoing"
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ListTransactionsResponse {
    pub transactions: Vec<NwcTransaction>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NwcTransaction {
    #[serde(rename = "type")]
    pub type_: String, // "incoming" or "outgoing"
    pub invoice: Option<String>,
    pub description: Option<String>,
    pub description_hash: Option<String>,
    pub preimage: Option<String>,
    pub payment_hash: String,
    pub amount: i64, // in msats
    pub fees_paid: i64, // in msats
    pub created_at: i64, // unix timestamp
    pub expires_at: Option<i64>, // unix timestamp
    pub settled_at: Option<i64>, // unix timestamp
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetBalanceRequest {
    // Empty for get_balance
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetBalanceResponse {
    pub balance: i64, // in msats
}

// Nostr Event types for NWC communication
#[derive(Debug, Serialize, Deserialize)]
pub struct NostrEvent {
    pub id: String,
    pub pubkey: String,
    pub created_at: i64,
    pub kind: i64,
    pub tags: Vec<Vec<String>>,
    pub content: String,
    pub sig: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NostrReqMessage {
    #[serde(rename = "type")]
    pub type_: String, // "REQ"
    pub subscription_id: String,
    pub filters: Vec<NostrFilter>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NostrFilter {
    pub ids: Option<Vec<String>>,
    pub authors: Option<Vec<String>>,
    pub kinds: Option<Vec<i64>>,
    pub since: Option<i64>,
    pub until: Option<i64>,
    pub limit: Option<i64>,
    #[serde(rename = "#e")]
    pub e_tags: Option<Vec<String>>,
    #[serde(rename = "#p")]
    pub p_tags: Option<Vec<String>>,
}
