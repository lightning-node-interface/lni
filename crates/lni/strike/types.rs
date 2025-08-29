use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct Balance {
    pub available: Amount,
    pub pending: Amount,
    pub reserved: Amount,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Amount {
    pub amount: String,
    pub currency: String,
}

#[derive(Debug, Deserialize)]
pub struct BalancesResponse {
    pub data: Vec<Balance>,
}

// Strike API v1/balances response structure
#[derive(Debug, Deserialize)]
pub struct StrikeBalance {
    pub currency: String,
    pub current: String,
    pub pending: String,
    pub outgoing: String,
    pub reserved: String,
    pub available: String,
    pub total: String,
}

#[derive(Debug, Deserialize)]
pub struct Account {
    pub id: String,
    pub handle: Option<String>,
    pub name: Option<String>,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AccountProfileResponse {
    pub data: Account,
}

#[derive(Debug, Deserialize)]
pub struct Invoice {
    pub id: String,
    pub amount: Amount,
    pub state: String, // "UNPAID", "PAID", "CANCELLED", "EXPIRED"
    pub created: String,
    pub correlation_id: Option<String>,
    pub description: Option<String>,
    pub issued_at: Option<String>,
    pub received_at: Option<String>,
    pub payment_hash: Option<String>,
    pub payment_request: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct InvoiceResponse {
    pub data: Invoice,
}

#[derive(Debug, Deserialize)]
pub struct InvoicesResponse {
    pub data: Vec<Invoice>,
    pub count: i32,
}

#[derive(Debug, Serialize)]
pub struct CreateInvoiceRequest {
    pub correlation_id: Option<String>,
    pub description: Option<String>,
    pub amount: CreateInvoiceAmount,
}

#[derive(Debug, Serialize)]
pub struct CreateInvoiceAmount {
    pub amount: String,
    pub currency: String,
}

#[derive(Debug, Deserialize)]
pub struct InvoiceQuote {
    pub id: String,
    pub lightning_invoice: LightningInvoice,
    pub target_amount: Amount,
    pub source_amount: Amount,
    pub conversion_rate: ConversionRate,
    pub created: String,
    pub expiry: String,
}

#[derive(Debug, Deserialize)]
pub struct LightningInvoice {
    pub payment_request: String,
    pub payment_hash: String,
    pub amount: String,
    pub description: Option<String>,
    pub created_at: String,
    pub expires_at: String,
}

#[derive(Debug, Deserialize)]
pub struct ConversionRate {
    pub amount: String,
    pub source_currency: String,
    pub target_currency: String,
}

#[derive(Debug, Deserialize)]
pub struct InvoiceQuoteResponse {
    pub data: InvoiceQuote,
}

#[derive(Debug, Deserialize)]
pub struct Payment {
    pub id: String,
    pub amount: Amount,
    pub state: String, // "PENDING", "COMPLETED", "FAILED"
    pub created: String,
    pub completed: Option<String>,
    pub correlation_id: Option<String>,
    pub description: Option<String>,
    pub lightning: Option<LightningPayment>,
    pub onchain: Option<OnchainPayment>,
}

#[derive(Debug, Deserialize)]
pub struct LightningPayment {
    pub network_fee: Option<Amount>,
    pub payment_hash: Option<String>,
    pub payment_request: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct OnchainPayment {
    pub txn_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PaymentResponse {
    pub data: Payment,
}

#[derive(Debug, Deserialize)]
pub struct PaymentsResponse {
    pub data: Vec<Payment>,
    pub count: i32,
}

#[derive(Debug, Serialize)]
pub struct PaymentQuoteRequest {
    #[serde(rename = "lnInvoice")]
    pub ln_invoice: String,
    #[serde(rename = "sourceCurrency")]
    pub source_currency: String,
    pub amount: Option<PaymentQuoteAmount>,
}

#[derive(Debug, Serialize)]
pub struct PaymentQuoteAmount {
    pub amount: String,
    pub currency: String,
}

#[derive(Debug, Deserialize)]
pub struct PaymentQuoteResponse {
    #[serde(rename = "lightningNetworkFee")]
    pub lightning_network_fee: Amount,
    #[serde(rename = "paymentQuoteId")]
    pub payment_quote_id: String,
    #[serde(rename = "validUntil")]
    pub valid_until: String,
    #[serde(rename = "conversionRate")]
    pub conversion_rate: Option<ConversionRate>,
    pub amount: Amount,
    #[serde(rename = "totalFee")]
    pub total_fee: Amount,
    #[serde(rename = "totalAmount")]
    pub total_amount: Amount,
}

#[derive(Debug, Deserialize)]
pub struct LightningDetails {
    #[serde(rename = "networkFee")]
    pub network_fee: Amount,
}

#[derive(Debug, Deserialize)]
pub struct PaymentExecutionResponse {
    #[serde(rename = "paymentId")]
    pub payment_id: String,
    pub state: String,
    pub result: String,
    pub completed: Option<String>,
    pub delivered: Option<String>,
    pub amount: Amount,
    #[serde(rename = "totalFee")]
    pub total_fee: Amount,
    #[serde(rename = "lightningNetworkFee")]
    pub lightning_network_fee: Amount,
    #[serde(rename = "totalAmount")]
    pub total_amount: Amount,
    pub lightning: Option<LightningDetails>,
}

#[derive(Debug, Deserialize)]
pub struct PaymentExecution {
    pub payment_id: String,
    #[serde(rename = "conversionRate")]
    pub conversion_rate: ConversionRate,
}

#[derive(Debug, Deserialize)]
pub struct ErrorResponse {
    pub data: ErrorData,
}

#[derive(Debug, Deserialize)]
pub struct ErrorData {
    pub status: i32,
    pub code: String,
    pub message: String,
}

#[derive(Debug, Deserialize)]
pub struct RatesTicker {
    pub data: Vec<Rate>,
}

#[derive(Debug, Deserialize)]
pub struct Rate {
    pub source_currency: String,
    pub target_currency: String,
    pub amount: String,
}

#[derive(Debug, Deserialize)]
pub struct AccountLimits {
    pub currency: String,
    pub send_daily: LimitDetails,
    pub send_weekly: LimitDetails,
    pub send_monthly: LimitDetails,
    pub receive_daily: LimitDetails,
    pub receive_weekly: LimitDetails,
    pub receive_monthly: LimitDetails,
    pub balance_maximum: LimitDetails,
}

#[derive(Debug, Deserialize)]
pub struct LimitDetails {
    pub amount: String,
    pub used: String,
    pub remaining: String,
}

#[derive(Debug, Deserialize)]
pub struct AccountLimitsResponse {
    pub data: Vec<AccountLimits>,
}

// Receive Request types
#[derive(Debug, Serialize)]
pub struct CreateReceiveRequestRequest {
    pub bolt11: Option<ReceiveRequestBolt11>,
    pub onchain: Option<ReceiveRequestOnchain>,
    #[serde(rename = "targetCurrency")]
    pub target_currency: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ReceiveRequestBolt11 {
    pub amount: Option<Amount>,
    pub description: Option<String>,
    #[serde(rename = "descriptionHash")]
    pub description_hash: Option<String>,
    #[serde(rename = "expiryInSeconds")]
    pub expiry_in_seconds: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct ReceiveRequestOnchain {
    pub amount: Option<Amount>,
}

#[derive(Debug, Deserialize)]
pub struct ReceiveRequest {
    pub id: String,
    pub state: String, // "CREATED", "PARTIALLY_RECEIVED", "RECEIVED", "CANCELLED", "EXPIRED"
    pub created: String,
    pub expires_at: Option<String>,
    pub bolt11: Option<ReceiveRequestBolt11Info>,
    pub onchain: Option<ReceiveRequestOnchainInfo>,
    pub target_currency: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ReceiveRequestBolt11Info {
    pub payment_request: String,
    pub payment_hash: String,
    pub amount: Option<Amount>,
    pub description: Option<String>,
    pub description_hash: Option<String>,
    pub expires_at: String,
}

#[derive(Debug, Deserialize)]
pub struct ReceiveRequestOnchainInfo {
    pub address: String,
    pub amount: Option<Amount>,
}

// Actual Strike API receive request response structure
#[derive(Debug, Deserialize)]
pub struct StrikeReceiveRequestResponse {
    #[serde(rename = "receiveRequestId")]
    pub receive_request_id: String,
    pub created: String,
    #[serde(rename = "targetCurrency")]
    pub target_currency: String,
    pub bolt11: Option<StrikeBolt11Info>,
    pub onchain: Option<StrikeOnchainInfo>,
}

#[derive(Debug, Deserialize)]
pub struct StrikeBolt11Info {
    pub invoice: String,
    #[serde(rename = "requestedAmount")]
    pub requested_amount: Option<Amount>,
    #[serde(rename = "btcAmount")]
    pub btc_amount: String,
    pub description: Option<String>,
    #[serde(rename = "paymentHash")]
    pub payment_hash: String,
    pub expires: String,
}

#[derive(Debug, Deserialize)]
pub struct StrikeOnchainInfo {
    pub address: String,
    #[serde(rename = "requestedAmount")]
    pub requested_amount: Option<Amount>,
    #[serde(rename = "btcAmount")]
    pub btc_amount: String,
}

// Strike API receives response (for completed transactions)
#[derive(Debug, Deserialize)]
pub struct StrikeReceivesResponse {
    pub items: Vec<StrikeReceive>,
}

// Strike API receives response with count (for filtered queries)
#[derive(Debug, Deserialize)]
pub struct StrikeReceivesWithCountResponse {
    pub items: Vec<StrikeReceive>,
    pub count: i32,
}

#[derive(Debug, Deserialize)]
pub struct StrikeReceive {
    #[serde(rename = "receiveId")]
    pub receive_id: String,
    #[serde(rename = "receiveRequestId")]
    pub receive_request_id: String,
    #[serde(rename = "type")]
    pub type_: String, // "LIGHTNING", "ONCHAIN"
    pub state: String, // "COMPLETED", "PENDING", etc.
    #[serde(rename = "amountReceived")]
    pub amount_received: Amount,
    #[serde(rename = "amountCredited")]
    pub amount_credited: Amount,
    pub created: String,
    pub completed: Option<String>,
    pub lightning: Option<StrikeReceiveLightning>,
}

#[derive(Debug, Deserialize)]
pub struct StrikeReceiveLightning {
    pub invoice: String,
    pub preimage: String,
    pub description: Option<String>,
    #[serde(rename = "descriptionHash")]
    pub description_hash: Option<String>,
    #[serde(rename = "paymentHash")]
    pub payment_hash: String,
}

// Strike API lookup response for receive requests (returns items array)
#[derive(Debug, Deserialize)]
pub struct StrikeReceiveRequestsLookupResponse {
    pub items: Vec<StrikeReceiveRequest>,
    pub count: i32,
}

#[derive(Debug, Deserialize)]
pub struct StrikeReceiveRequest {
    #[serde(rename = "receiveRequestId")]
    pub receive_request_id: String,
    pub created: String,
    #[serde(rename = "targetCurrency")]
    pub target_currency: String,
    pub bolt11: Option<StrikeBolt11Info>,
    pub onchain: Option<StrikeOnchainInfo>,
}

// Keep the old types for compatibility, but add the new ones
#[derive(Debug, Deserialize)]
pub struct ReceiveRequestResponse {
    pub data: ReceiveRequest,
}

#[derive(Debug, Deserialize)]
pub struct ReceiveRequestsResponse {
    pub data: Vec<ReceiveRequest>,
    pub count: i32,
}
