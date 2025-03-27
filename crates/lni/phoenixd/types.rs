use serde::{Deserialize, Serialize};
use crate::InvoiceType;
#[cfg(feature = "napi_rs")]
use napi_derive::napi;


#[derive(Debug, Deserialize)]
pub struct InfoResponse {
    #[serde(rename = "nodeId")] // Handle JSON field `nodeId`
    pub node_id: String,
    pub channels: Vec<Channel>,
}

#[derive(Debug, Deserialize)]
pub struct Channel {
    #[serde(rename = "state")]
    pub state: String, // Normal
    #[serde(rename = "channelId")]
    pub channel_id: String,
    #[serde(rename = "balanceSat")]
    pub balance_sat: i64,
    #[serde(rename = "inboundLiquiditySat")]
    pub inbound_liquidity_sat: i64,
    #[serde(rename = "capacitySat")]
    pub capacity_sat: i64,
    #[serde(rename = "fundingTxId")]
    pub funding_tx_id: String,
}

#[derive(Debug, Deserialize)]
pub struct GetBalanceResponse {
    #[serde(rename = "balanceSat")]
    pub balance_sat: i64,
    #[serde(rename = "feeCreditSat")]
    pub fee_credit_sat: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Bolt11Req {
    #[serde(rename = "amountSat")]
    pub amount_sat: i64,
    #[serde(rename = "expirySeconds")]
    pub expiry_seconds: i64,
    #[serde(rename = "externalId")]
    pub external_id: Option<String>,
    #[serde(rename = "description")]
    pub description: Option<String>,
    #[serde(rename = "webhookUrl")]
    pub webhook_url: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InvoiceResponse {
    #[serde(rename = "preimage")]
    pub preimage: String,
    #[serde(rename = "paymentHash")]
    pub payment_hash: String,
    #[serde(rename = "receivedSat")]
    pub received_sat: i64,
    #[serde(rename = "fees")]
    pub fees: i64,
    #[serde(rename = "completedAt")]
    pub completed_at: i64,
    #[serde(rename = "createdAt")]
    pub created_at: i64,
    #[serde(rename = "isPaid")]
    pub is_paid: bool,
    #[serde(rename = "payerKey")]
    pub payer_key: Option<String>,
    #[serde(rename = "invoice")]
    pub invoice: Option<String>,
    #[serde(rename = "description")]
    pub description: Option<String>,
    #[serde(rename = "payerNote")]
    pub payer_note: Option<String>, // used in bolt12
    #[serde(rename = "externalId")]
    pub external_id: Option<String>, // used in bolt11
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OutgoingPaymentResponse {
    #[serde(rename = "paymentId")]
    pub payment_id: Option<String>,
    #[serde(rename = "preimage")]
    pub preimage: String,
    #[serde(rename = "paymentHash")]
    pub payment_hash: String,
    #[serde(rename = "sent")]
    pub sent: i64,
    #[serde(rename = "fees")]
    pub fees: i64,
    #[serde(rename = "createdAt")]
    pub created_at: i64,
    #[serde(rename = "completedAt")]
    pub completed_at: i64,
    #[serde(rename = "isPaid")]
    pub is_paid: bool,
    #[serde(rename = "payerNote")]
    pub payer_note: Option<String>, // used in bolt12
    #[serde(rename = "externalId")]
    pub external_id: Option<String>, // used in bolt11
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PayResponse {
    #[serde(rename = "paymentId")]
    pub payment_id: Option<String>,
    #[serde(rename = "paymentPreimage")]
    pub preimage: String,
    #[serde(rename = "paymentHash")]
    pub payment_hash: String,
    #[serde(rename = "routingFeeSat")]
    pub routing_fee_sat: i64,
}


#[cfg_attr(feature = "napi_rs", napi(object))]
#[derive(Debug, Serialize, Deserialize)]
pub struct Bolt11Resp {
    #[serde(rename = "amountSat")]
    pub amount_sat: i64,
    #[serde(rename = "paymentHash")]
    pub payment_hash: String,
    #[serde(rename = "serialized")]
    pub serialized: String,
}



#[cfg_attr(feature = "napi_rs", napi(object))]
#[derive(Debug, Serialize, Deserialize)]
pub struct PhoenixPayInvoiceResp {
    #[serde(rename = "recipientAmountSat")]
    pub amount_sat: i64,
    #[serde(rename = "routingFeeSat")]
    pub routing_fee_sat: i64,
    #[serde(rename = "paymentId")]
    pub payment_id: String,
    #[serde(rename = "paymentHash")]
    pub payment_hash: String,
    #[serde(rename = "paymentPreimage")]
    pub preimage: String,
}