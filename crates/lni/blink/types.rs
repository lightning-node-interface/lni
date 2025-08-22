use serde::{Deserialize, Serialize};

// GraphQL request/response structures
#[derive(Debug, Serialize)]
pub struct GraphQLRequest {
    pub query: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variables: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct GraphQLResponse<T> {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub errors: Option<Vec<GraphQLError>>,
}

#[derive(Debug, Deserialize)]
pub struct GraphQLError {
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
}

// User and account structures
#[derive(Debug, Deserialize)]
pub struct MeQuery {
    pub me: User,
}

#[derive(Debug, Deserialize)]
pub struct User {
    #[serde(rename = "defaultAccount")]
    pub default_account: Account,
}

#[derive(Debug, Deserialize)]
pub struct Account {
    pub wallets: Vec<Wallet>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transactions: Option<TransactionConnection>,
}

#[derive(Debug, Deserialize)]
pub struct Wallet {
    pub id: String,
    #[serde(rename = "walletCurrency")]
    pub wallet_currency: String, // "BTC" or "USD"
    pub balance: i64,
}

// Invoice creation structures
#[derive(Debug, Serialize)]
pub struct LnInvoiceCreateInput {
    pub amount: String, // Amount in satoshis as string
    #[serde(rename = "walletId")]
    pub wallet_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memo: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct LnInvoiceCreateResponse {
    #[serde(rename = "lnInvoiceCreate")]
    pub ln_invoice_create: LnInvoiceCreateResult,
}

#[derive(Debug, Deserialize)]
pub struct LnInvoiceCreateResult {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub invoice: Option<Invoice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub errors: Option<Vec<GraphQLError>>,
}

#[derive(Debug, Deserialize)]
pub struct Invoice {
    #[serde(rename = "paymentRequest")]
    pub payment_request: String,
    #[serde(rename = "paymentHash")]
    pub payment_hash: String,
    #[serde(rename = "paymentSecret")]
    pub payment_secret: String,
    pub satoshis: i64,
}

// Payment structures
#[derive(Debug, Serialize)]
pub struct LnInvoicePaymentInput {
    #[serde(rename = "paymentRequest")]
    pub payment_request: String,
    #[serde(rename = "walletId")]
    pub wallet_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memo: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct LnInvoicePaymentSendResponse {
    #[serde(rename = "lnInvoicePaymentSend")]
    pub ln_invoice_payment_send: LnInvoicePaymentResult,
}

#[derive(Debug, Deserialize)]
pub struct LnInvoicePaymentResult {
    pub status: String, // "SUCCESS", "FAILURE", "PENDING"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub errors: Option<Vec<GraphQLError>>,
}

// Fee probe structures
#[derive(Debug, Serialize)]
pub struct LnInvoiceFeeProbeInput {
    #[serde(rename = "paymentRequest")]
    pub payment_request: String,
    #[serde(rename = "walletId")]
    pub wallet_id: String,
}

#[derive(Debug, Deserialize)]
pub struct LnInvoiceFeeProbeResponse {
    #[serde(rename = "lnInvoiceFeeProbe")]
    pub ln_invoice_fee_probe: LnInvoiceFeeProbeResult,
}

#[derive(Debug, Deserialize)]
pub struct LnInvoiceFeeProbeResult {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount: Option<i64>, // Fee amount in satoshis
    #[serde(skip_serializing_if = "Option::is_none")]
    pub errors: Option<Vec<GraphQLError>>,
}

// Transaction structures
#[derive(Debug, Deserialize)]
pub struct TransactionConnection {
    pub edges: Vec<TransactionEdge>,
    #[serde(rename = "pageInfo")]
    pub page_info: PageInfo,
}

#[derive(Debug, Deserialize)]
pub struct TransactionEdge {
    pub cursor: String,
    pub node: Transaction,
}

#[derive(Debug, Deserialize)]
pub struct Transaction {
    pub id: String,
    #[serde(rename = "createdAt")]
    pub created_at: i64, // Unix timestamp
    pub direction: String,  // "SEND" or "RECEIVE"
    pub status: String,     // "SUCCESS", "FAILURE", "PENDING"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memo: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "settlementAmount")]
    pub settlement_amount: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "settlementCurrency")]
    pub settlement_currency: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "settlementFee")]
    pub settlement_fee: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "settlementDisplayAmount")]
    pub settlement_display_amount: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "settlementDisplayCurrency")]
    pub settlement_display_currency: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "settlementDisplayFee")]
    pub settlement_display_fee: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "settlementPrice")]
    pub settlement_price: Option<Price>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "initiationVia")]
    pub initiation_via: Option<InitiationVia>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "settlementVia")]
    pub settlement_via: Option<SettlementVia>,
}

#[derive(Debug, Deserialize)]
pub struct Price {
    pub base: i64,
    pub offset: i64,
    #[serde(rename = "currencyUnit")]
    pub currency_unit: String,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "__typename")]
pub enum InitiationVia {
    #[serde(rename = "InitiationViaLn")]
    InitiationViaLn {
        #[serde(rename = "paymentHash")]
        payment_hash: String,
    },
    #[serde(rename = "InitiationViaOnChain")]
    InitiationViaOnChain {
        #[serde(skip_serializing_if = "Option::is_none")]
        address: Option<String>,
    },
    #[serde(rename = "InitiationViaIntraLedger")]
    InitiationViaIntraLedger {},
}

#[derive(Debug, Deserialize)]
#[serde(tag = "__typename")]
pub enum SettlementVia {
    #[serde(rename = "SettlementViaLn")]
    SettlementViaLn {
        #[serde(rename = "preImage")]
        pre_image: Option<String>,
    },
    #[serde(rename = "SettlementViaOnChain")]
    SettlementViaOnChain {
        #[serde(rename = "transactionHash")]
        transaction_hash: Option<String>,
    },
    #[serde(rename = "SettlementViaIntraLedger")]
    SettlementViaIntraLedger {},
}

#[derive(Debug, Deserialize)]
pub struct PageInfo {
    #[serde(rename = "hasNextPage")]
    pub has_next_page: bool,
    #[serde(rename = "hasPreviousPage")]
    pub has_previous_page: bool,
    #[serde(rename = "startCursor")]
    pub start_cursor: Option<String>,
    #[serde(rename = "endCursor")]
    pub end_cursor: Option<String>,
}

// Query variables for transactions
#[derive(Debug, Serialize)]
pub struct TransactionQueryVariables {
    pub first: Option<i32>,
    pub after: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TransactionsQuery {
    pub me: UserWithTransactions,
}

#[derive(Debug, Deserialize)]
pub struct UserWithTransactions {
    #[serde(rename = "defaultAccount")]
    pub default_account: AccountWithTransactions,
}

#[derive(Debug, Deserialize)]
pub struct AccountWithTransactions {
    pub transactions: TransactionConnection,
}