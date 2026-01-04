use serde::{Deserialize, Serialize};

/// Spark SDK Network type
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SparkNetwork {
    Mainnet,
    Regtest,
}

impl Default for SparkNetwork {
    fn default() -> Self {
        Self::Mainnet
    }
}

/// Payment type from Spark SDK
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum SparkPaymentType {
    Send,
    Receive,
}

/// Payment status from Spark SDK
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum SparkPaymentStatus {
    Completed,
    Pending,
    Failed,
}

/// Payment details from Spark SDK
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SparkPayment {
    pub id: String,
    pub payment_type: SparkPaymentType,
    pub status: SparkPaymentStatus,
    pub amount_sats: u64,
    pub fees_sats: u64,
    pub timestamp: u64,
    pub invoice: Option<String>,
    pub payment_hash: Option<String>,
    pub preimage: Option<String>,
    pub description: Option<String>,
}
