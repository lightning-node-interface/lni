use serde::Deserialize;
use crate::PayCode;

#[derive(Debug, Deserialize)]
pub struct Chain {
    pub chain: String,
    pub network: String,
}

#[derive(Debug, Deserialize)]
pub struct GetInfoResponse {
    pub version: String,
    pub commit_hash: String,
    pub identity_pubkey: String,
    pub alias: String,
    pub color: String,
    pub num_pending_channels: i64,
    pub num_active_channels: i64,
    pub num_inactive_channels: i64,
    pub num_peers: i64,
    pub block_height: i64,
    pub block_hash: String,
    pub best_header_timestamp: String,
    pub synced_to_chain: bool,
    pub synced_to_graph: bool,
    pub testnet: bool,
    pub chains: Vec<Chain>,
    pub uris: Vec<String>,
    pub features: serde_json::Value,
    pub require_htlc_interceptor: bool,
    pub store_final_htlc_resolutions: bool,
}

#[derive(Debug, Deserialize)]
pub struct FetchInvoiceResponse {
    pub invoice: String,
}

#[derive(Debug, Deserialize)]
pub struct PayResponse {
    pub destination: String,
    pub payment_hash: String,
    pub created_at: f64,
    pub parts: i32,
    pub amount_msat: i64,
    pub amount_sent_msat: i64,
    pub payment_preimage: String,
    pub status: String,
}

#[derive(Debug, Deserialize)]
pub struct PaidOutpoint {
    pub txid: String,
    pub outnum: i32,
}

#[derive(Debug, Deserialize)]
pub struct Invoice {
    pub label: String,
    pub bolt11: Option<String>,
    pub bolt12: Option<String>,
    pub payment_hash: String,
    pub status: String, // "paid" "unpaid" "expired"
    pub pay_index: Option<i32>,
    pub amount_received_msat: Option<i64>,
    pub paid_at: Option<i64>,
    pub payment_preimage: Option<String>,
    pub description: Option<String>,
    pub expires_at: i64,
    pub created_index: i32,
    pub updated_index: Option<i32>,
    pub amount_msat: Option<i64>,
    pub local_offer_id: Option<String>,
    pub invreq_payer_note: Option<String>,
    pub paid_outpoint: Option<PaidOutpoint>,
}

#[derive(Debug, Deserialize)]
pub struct InvoicesResponse {
    pub invoices: Vec<Invoice>,
}

#[derive(Debug, Deserialize)]
pub struct Bolt11Resp {
    pub r_hash: String,
    pub payment_request: String,
    pub add_index: String,
    pub payment_addr: String,
}

#[derive(Debug, Deserialize)]
pub struct Bolt12Resp {
    pub offer_id: Option<String>,
    pub bolt12: String,
    pub active: bool,
    pub single_use: bool,
    pub used: bool,
    pub created: bool,
}

#[derive(Debug, Deserialize)]
pub struct ListOffersResponse {
    pub offers: Vec<PayCode>,
}

#[derive(Debug, Deserialize)]
pub struct ListInvoiceResponseWrapper {
    pub invoices: Vec<ListInvoiceResponse>,
    pub last_index_offset: Option<String>,
    pub first_index_offset: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ListInvoiceResponse {
    pub memo: Option<String>,
    pub r_preimage: Option<String>,
    pub r_hash: Option<String>,
    pub value: Option<String>,
    pub value_msat: Option<String>,
    pub settled: Option<bool>,
    pub creation_date: Option<String>,
    pub settle_date: Option<String>,
    pub payment_request: Option<String>,
    pub description: Option<String>,
    pub description_hash_hex: Option<String>,
    pub description_hash_b64: Option<String>,
    pub description_hash: Option<String>,
    pub expiry: Option<String>,
    pub fallback_addr: Option<String>,
    pub cltv_expiry: Option<String>,
    pub route_hints: Option<serde_json::Value>,
    pub private: Option<bool>,
    pub add_index: Option<String>,
    pub settle_index: Option<String>,
    pub amt_paid: Option<String>,
    pub amt_paid_sat: Option<String>,
    pub amt_paid_msat: Option<String>,
    pub state: Option<String>,
    pub htlcs: Option<serde_json::Value>,
    pub features: Option<serde_json::Value>,
    pub is_keysend: Option<bool>,
    pub payment_addr: Option<String>,
    pub payment_addr_hash: Option<String>,
    pub is_amp: Option<bool>,
    pub amp_invoice_state: Option<serde_json::Value>,
    pub is_blinded: Option<bool>,
    pub blinded_path_config: Option<serde_json::Value>,
}
