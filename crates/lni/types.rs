#[cfg(feature = "napi_rs")]
use napi_derive::napi;
use serde::{Deserialize, Serialize};

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
    pub send_balance_msat: i64, // Sum of channels send capacity
    pub receive_balance_msat: i64, // Sum of channels receive capacity
    pub fee_credit_balance_msat: i64, // used in Phoenixd, typically first 30,000 sats are a "fee credit" aka custodial, but cannot withdraw (balance is used for future fees). Then it opens channel when it gets above 30,000 sats.
    pub unsettled_send_balance_msat: i64, // Sum of channels send unsettled balances.
    pub unsettled_receive_balance_msat: i64, // Sum of channels receive unsettled balances.
    pub pending_open_send_balance: i64, // Sum of channels pending open send balances.
    pub pending_open_receive_balance: i64, // Sum of channels pending open receive balances.
}
impl Default for NodeInfo {
    fn default() -> Self {
        Self {
            alias: String::new(),
            color: String::new(),
            pubkey: String::new(),
            network: String::new(),
            block_height: 0,
            block_hash: String::new(),
            send_balance_msat: 0,
            receive_balance_msat: 0,
            fee_credit_balance_msat: 0,
            unsettled_send_balance_msat: 0,
            unsettled_receive_balance_msat: 0,
            pending_open_send_balance: 0,
            pending_open_receive_balance: 0,
        }
    }
}

#[cfg_attr(feature = "napi_rs", napi(object))]
#[derive(Debug, Serialize, Deserialize, Clone)]
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
    pub payer_note: Option<String>,  // used in bolt12 (on phoenixd)
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
    pub fee_msats: i64,
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

#[cfg_attr(feature = "napi_rs", napi(object))]
#[derive(Debug, Serialize, Deserialize)]
pub struct ListTransactionsParams {
    pub from: i64,
    pub limit: i64,
    pub payment_hash: Option<String>,
}

#[cfg_attr(feature = "napi_rs", napi(object))]
#[derive(Debug, Serialize, Deserialize)]
pub struct CreateInvoiceParams {
    pub invoice_type: InvoiceType,
    pub amount_msats: Option<i64>,
    pub offer: Option<String>,
    pub description: Option<String>,
    pub description_hash: Option<String>,
    pub expiry: Option<i64>,
    pub r_preimage: Option<String>,
    pub is_blinded: Option<bool>,
    pub is_keysend: Option<bool>,
    pub is_amp: Option<bool>,
    pub is_private: Option<bool>,
    // pub route_hints: Option<Vec<HopHint>>, TODO
}
impl Default for CreateInvoiceParams {
    fn default() -> Self {
        Self {
            invoice_type: InvoiceType::Bolt11,
            amount_msats: None,
            offer: None,
            description: None,
            description_hash: None,
            expiry: None,
            r_preimage: None,
            is_blinded: Some(false),
            is_keysend: Some(false),
            is_amp: Some(false),
            is_private: Some(false),
        }
    }
}

// Pay Code aka BOLT12 Offer
#[cfg_attr(feature = "napi_rs", napi(object))]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PayCode {
    pub offer_id: String,
    pub bolt12: String,
    pub label: Option<String>,
    pub active: Option<bool>,
    pub single_use: Option<bool>,
    pub used: Option<bool>,
}

#[cfg_attr(feature = "napi_rs", napi(object))]
#[derive(Debug, Serialize, Deserialize)]
pub struct PayInvoiceParams {
    pub invoice: String,
    pub fee_limit_msat: Option<i64>, // mutually exclusive with fee_limit_percentage (only set one or the other)
    pub fee_limit_percentage: Option<f64>, // mutually exclusive with fee_limit_msat
    pub timeout_seconds: Option<i64>,
    pub amount_msats: Option<i64>, // used the specify the amount for zero amount invoices

    pub max_parts: Option<i64>, // The maximum number of partial payments that may be use to complete the full amount.
    pub first_hop_pubkey: Option<String>,
    pub last_hop_pubkey: Option<String>,
    pub allow_self_payment: Option<bool>, // circular payments
    pub is_amp: Option<bool>,             // enable atomic multipath payments
}
impl Default for PayInvoiceParams {
    fn default() -> Self {
        Self {
            invoice: "".to_string(),
            fee_limit_msat: None,
            fee_limit_percentage: None, // 0.2% is a sensible default
            timeout_seconds: Some(60),  // default to 60 seconds timeout
            amount_msats: None,

            max_parts: None,
            first_hop_pubkey: None,
            last_hop_pubkey: None,
            allow_self_payment: None, // allow self (circurlar) payments
            is_amp: None,
        }
    }
}
