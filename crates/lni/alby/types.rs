use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct AlbyInfoResponse {
    #[serde(rename = "backendType")]
    pub backend_type: String,
    #[serde(rename = "setupCompleted")]
    pub setup_completed: bool,
    #[serde(rename = "oauthRedirect")]
    pub oauth_redirect: bool,
    pub running: bool,
    pub unlocked: bool,
    #[serde(rename = "albyAuthUrl")]
    pub alby_auth_url: String,
    #[serde(rename = "nextBackupReminder")]
    pub next_backup_reminder: String,
    #[serde(rename = "albyUserIdentifier")]
    pub alby_user_identifier: String,
    #[serde(rename = "albyAccountConnected")]
    pub alby_account_connected: bool,
    pub version: String,
    pub network: String,
    #[serde(rename = "enableAdvancedSetup")]
    pub enable_advanced_setup: bool,
    #[serde(rename = "ldkVssEnabled")]
    pub ldk_vss_enabled: bool,
    #[serde(rename = "vssSupported")]
    pub vss_supported: bool,
    #[serde(rename = "startupState")]
    pub startup_state: String,
    #[serde(rename = "startupError")]
    pub startup_error: String,
    #[serde(rename = "startupErrorTime")]
    pub startup_error_time: String,
    #[serde(rename = "autoUnlockPasswordSupported")]
    pub auto_unlock_password_supported: bool,
    #[serde(rename = "autoUnlockPasswordEnabled")]
    pub auto_unlock_password_enabled: bool,
    pub currency: String,
    pub relay: String,
    #[serde(rename = "nodeAlias")]
    pub node_alias: Option<String>, // This can be empty/missing, so using Option
    #[serde(rename = "mempoolUrl")]
    pub mempool_url: String,
}

#[derive(Debug, Deserialize)]
pub struct AlbyBalance {
    pub balance: i64,
}

#[derive(Debug, Deserialize)]
pub struct AlbyBalancesResponse {
    #[serde(rename = "balance")]
    pub balance: Option<i64>,
    #[serde(rename = "unit")]
    pub unit: Option<String>,
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