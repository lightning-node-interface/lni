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