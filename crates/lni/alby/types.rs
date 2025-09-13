use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct AlbyInfoResponse {
    pub alias: String,
    pub color: String,
    pub pubkey: String,
    pub network: String,
    pub block_height: i64,
    pub block_hash: String,
    pub version: String,
}

#[derive(Debug, Deserialize)]
pub struct AlbyBalance {
    pub balance: i64,
    pub currency: String,
}

#[derive(Debug, Deserialize)]
pub struct AlbyBalancesResponse {
    pub balances: Vec<AlbyBalance>,
}

#[derive(Debug, Deserialize)]
pub struct AlbyCreateInvoiceResponse {
    pub payment_request: String,
    pub payment_hash: String,
    pub amount: i64,
    pub description: String,
    pub created_at: String,
    pub expires_at: String,
}

#[derive(Debug, Deserialize)]
pub struct AlbyPaymentResponse {
    pub payment_hash: String,
    pub payment_preimage: String,
    pub destination: String,
    pub amount: i64,
    pub fee: i64,
    pub status: String,
    pub created_at: String,
    pub settled_at: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AlbyTransactionResponse {
    pub payment_hash: String,
    pub payment_request: Option<String>,
    pub payment_preimage: Option<String>,
    pub amount: i64,
    pub fee: Option<i64>,
    pub status: String,
    pub created_at: String,
    pub settled_at: Option<String>,
    pub description: Option<String>,
    pub type_: String,
}

#[derive(Debug, Deserialize)]
pub struct AlbyTransactionsResponse {
    pub transactions: Vec<AlbyTransactionResponse>,
    pub total: Option<i64>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct AlbyLookupInvoiceResponse {
    pub payment_hash: String,
    pub payment_request: String,
    pub payment_preimage: Option<String>,
    pub amount: i64,
    pub fee: Option<i64>,
    pub status: String,
    pub created_at: String,
    pub settled_at: Option<String>,
    pub expires_at: String,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AlbyDecodeResponse {
    pub payment_hash: String,
    pub amount_msat: i64,
    pub description: String,
    pub destination: String,
    pub expiry: i64,
    pub timestamp: i64,
}

// Request types
#[derive(Debug, serde::Serialize)]
pub struct AlbyCreateInvoiceRequest {
    pub amount: i64,
    pub description: Option<String>,
    pub expiry: Option<i64>,
}

#[derive(Debug, serde::Serialize)]
pub struct AlbyPayInvoiceRequest {
    pub invoice: String,
    pub amount: Option<i64>,
}