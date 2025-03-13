use serde::Deserialize;

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
