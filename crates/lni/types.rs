use serde::{Deserialize, Serialize};
#[cfg(feature = "napi_rs")]
use napi_derive::napi;

#[cfg_attr(feature = "napi_rs", napi(string_enum))]
#[derive(Debug, Serialize, Deserialize)]
pub enum InvoiceType {
    Bolt11,
    Bolt12,
}

#[cfg_attr(feature = "napi_rs", napi(object))]
#[derive(Debug, Serialize, Deserialize)]
pub struct TLVRecord {
    #[serde(rename = "type")]
    pub type_: i64,
    // hex-encoded value
    pub value: String,
}
#[cfg_attr(feature = "napi_rs", napi(object))]
#[derive(Debug, Serialize, Deserialize)]
pub struct NodeInfo {
    pub alias: String,
    pub color: String,
    pub pubkey: String,
    pub network: String,
    pub block_height: i64,
    pub block_hash: String,
}

#[cfg_attr(feature = "napi_rs", napi(object))]
#[derive(Debug, Serialize, Deserialize)]
pub struct Transaction {
    pub type_: String,
    pub invoice: String,
    pub description: String,
    pub description_hash: String,
    pub preimage: String,
    pub payment_hash: String,
    pub amount_msats: i64,
    pub fees_paid: i64,
    pub created_at: i64,
    pub expires_at: i64,
    pub settled_at: i64,
    pub payer_note: Option<String>, // used in bolt12 (on phoenixd)
    pub external_id: Option<String>, // used in bolt11 (on phoenixd)
}

#[cfg_attr(feature = "napi_rs", napi(object))]
#[derive(Debug, Serialize, Deserialize)]
pub struct NodeConnectionInfo {
    pub pubkey: String,
    pub address: String,
    pub port: i64,
}

#[cfg_attr(feature = "napi_rs", napi(object))]
#[derive(Debug, Serialize, Deserialize)]
pub struct Channel {
    pub local_balance: i64,
    pub local_spendable_balance: i64,
    pub remote_balance: i64,
    pub id: String,
    pub remote_pubkey: String,
    pub funding_tx_id: String,
    pub funding_tx_vout: i64,
    pub active: bool,
    pub public: bool,
    pub internal_channel: String, //serde_json::Value,
    pub confirmations: i64,
    pub confirmations_required: i64,
    pub forwarding_fee_base_msat: i64,
    pub unspendable_punishment_reserve: i64,
    pub counterparty_unspendable_punishment_reserve: i64,
    pub error: String,
    pub is_outbound: bool,
}

#[cfg_attr(feature = "napi_rs", napi(object))]
#[derive(Debug, Serialize, Deserialize)]
pub struct NodeStatus {
    #[serde(rename = "isReady")]
    pub is_ready: bool,
    pub internal_node_status: String, // serde_json::Value,
}

#[cfg_attr(feature = "napi_rs", napi(object))]
#[derive(Debug, Serialize, Deserialize)]
pub struct ConnectPeerRequest {
    pub pubkey: String,
    pub address: String,
    pub port: i64,
}

#[cfg_attr(feature = "napi_rs", napi(object))]
#[derive(Debug, Serialize, Deserialize)]
pub struct OpenChannelRequest {
    pub pubkey: String,
    pub amount_msats: i64,
    pub public: bool,
}

#[cfg_attr(feature = "napi_rs", napi(object))]
#[derive(Debug, Serialize, Deserialize)]
pub struct OpenChannelResponse {
    pub funding_tx_id: String,
}

#[cfg_attr(feature = "napi_rs", napi(object))]
#[derive(Debug, Serialize, Deserialize)]
pub struct CloseChannelRequest {
    pub channel_id: String,
    pub node_id: String,
    pub force: bool,
}

#[cfg_attr(feature = "napi_rs", napi(object))]
#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateChannelRequest {
    pub channel_id: String,
    pub node_id: String,
    pub forwarding_fee_base_msat: i64,
    pub max_dust_htlc_exposure_from_fee_rate_multiplier: i64,
}

#[cfg_attr(feature = "napi_rs", napi(object))]
#[derive(Debug, Serialize, Deserialize)]
pub struct CloseChannelResponse {}

#[cfg_attr(feature = "napi_rs", napi(object))]
#[derive(Debug, Serialize, Deserialize)]
pub struct PendingBalanceDetails {
    pub channel_id: String,
    pub node_id: String,
    pub amount_msats: i64,
    pub funding_tx_id: String,
    pub funding_tx_vout: i64,
}

#[cfg_attr(feature = "napi_rs", napi(object))]
#[derive(Debug, Serialize, Deserialize)]
pub struct OnchainBalanceResponse {
    pub spendable: i64,
    pub total: i64,
    pub reserved: i64,
    pub pending_balances_from_channel_closures: i64,
    pub pending_balances_details: Vec<PendingBalanceDetails>,
    pub internal_balances: String, // serde_json::Value,
}

#[cfg_attr(feature = "napi_rs", napi(object))]
#[derive(Debug, Serialize, Deserialize)]
pub struct PeerDetails {
    pub node_id: String,
    pub address: String,
    pub is_persisted: bool,
    pub is_connected: bool,
}

#[cfg_attr(feature = "napi_rs", napi(object))]
#[derive(Debug, Serialize, Deserialize)]
pub struct LightningBalanceResponse {
    pub total_spendable: i64,
    pub total_receivable: i64,
    pub next_max_spendable: i64,
    pub next_max_receivable: i64,
    pub next_max_spendable_mpp: i64,
    pub next_max_receivable_mpp: i64,
}

#[cfg_attr(feature = "napi_rs", napi(object))]
#[derive(Debug, Serialize, Deserialize)]
pub struct PayInvoiceResponse {
    pub payment_hash: String,
    pub preimage: String,
    pub fee: i64,
}

#[cfg_attr(feature = "napi_rs", napi(object))]
#[derive(Debug, Serialize, Deserialize)]
pub struct PayKeysendResponse {
    pub fee: i64,
}

#[cfg_attr(feature = "napi_rs", napi(object))]
#[derive(Debug, Serialize, Deserialize)]
pub struct BalancesResponse {
    pub onchain: OnchainBalanceResponse,
    pub lightning: LightningBalanceResponse,
}

// pub type NetworkGraphResponse = serde_json::Value;

#[cfg_attr(feature = "napi_rs", napi(object))]
#[derive(Debug, Serialize, Deserialize)]
pub struct PaymentFailedEventProperties {
    pub transaction: Transaction,
    pub reason: String,
}

pub const DEFAULT_INVOICE_EXPIRY: i64 = 86400;
