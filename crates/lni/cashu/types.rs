use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct CashuMintInfo {
    pub name: Option<String>,
    pub version: Option<String>,
    pub description: Option<String>,
    pub pubkey: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct CashuBalance {
    pub available: i64,
    pub pending: i64,
}
