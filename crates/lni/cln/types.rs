use serde::Deserialize;

use crate::PayCode;

#[derive(Debug, Deserialize)]
pub struct InfoResponse {
    // #[serde(rename = "nodeId")] // Handle JSON field `nodeId`
    // pub node_id: String,
    pub id: String,
    pub alias: String,
    pub color: String,
    pub network: String,
    pub blockheight: i64,
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
    pub payment_hash: String,
    pub expires_at: f64,
    pub bolt11: String,
    pub payment_secret: String,
    pub created_index: i32,
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
pub struct ChannelWrapper {
    #[serde(skip)]
    pub outputs: Vec<serde_json::Value>,
    pub channels: Vec<Channel>,
}

#[derive(Debug, Deserialize)]
pub struct Channel {
    pub peer_id: String,
    pub connected: bool,
    pub state: String,
    pub channel_id: String,
    pub short_channel_id: Option<String>,
    pub our_amount_msat: i64,
    pub amount_msat: i64,
    pub funding_txid: String,
    pub funding_output: i32,
}