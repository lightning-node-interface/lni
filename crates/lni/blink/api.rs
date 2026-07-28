use super::BlinkConfig;
use crate::{
    ApiError, CreateInvoiceParams, Offer, OnInvoiceEventCallback, OnInvoiceEventParams,
    OnchainTransaction, PayInvoiceParams, PayInvoiceResponse, PayOnchainOptions,
    PayOnchainResponse, PrepareOnchainTransactionParams, Transaction,
};

fn galoy(config: &BlinkConfig) -> crate::galoy::GaloyConfig {
    config.into()
}

pub async fn get_info(config: &BlinkConfig) -> Result<crate::NodeInfo, ApiError> {
    crate::galoy::api::get_info(&galoy(config)).await
}

pub async fn create_invoice(
    config: &BlinkConfig,
    params: CreateInvoiceParams,
) -> Result<Transaction, ApiError> {
    crate::galoy::api::create_invoice(&galoy(config), params).await
}

pub async fn pay_invoice(
    config: &BlinkConfig,
    params: PayInvoiceParams,
) -> Result<PayInvoiceResponse, ApiError> {
    crate::galoy::api::pay_invoice(&galoy(config), params).await
}

pub async fn prepare_onchain_transaction(
    config: &BlinkConfig,
    params: PrepareOnchainTransactionParams,
) -> Result<OnchainTransaction, ApiError> {
    crate::galoy::api::prepare_onchain_transaction(&galoy(config), params).await
}

pub async fn pay_onchain(
    config: &BlinkConfig,
    transaction: OnchainTransaction,
) -> Result<PayOnchainResponse, ApiError> {
    crate::galoy::api::pay_onchain(&galoy(config), transaction).await
}

pub async fn pay_onchain_with_options(
    config: &BlinkConfig,
    transaction: OnchainTransaction,
    options: PayOnchainOptions,
) -> Result<PayOnchainResponse, ApiError> {
    crate::galoy::api::pay_onchain_with_options(&galoy(config), transaction, options).await
}

pub async fn decode(value: String) -> Result<String, ApiError> {
    crate::galoy::api::decode(value).await
}

pub async fn decode_offer(offer: String) -> Result<String, ApiError> {
    crate::galoy::api::decode_offer(offer).await
}

pub async fn get_offer(config: &BlinkConfig, search: Option<String>) -> Result<Offer, ApiError> {
    crate::galoy::api::get_offer(&galoy(config), search).await
}

pub async fn list_offers(
    config: &BlinkConfig,
    search: Option<String>,
) -> Result<Vec<Offer>, ApiError> {
    crate::galoy::api::list_offers(&galoy(config), search).await
}

pub async fn create_offer(
    config: &BlinkConfig,
    amount_msats: Option<i64>,
    description: Option<String>,
    expiry: Option<i64>,
) -> Result<Transaction, ApiError> {
    crate::galoy::api::create_offer(&galoy(config), amount_msats, description, expiry).await
}

pub async fn fetch_invoice_from_offer(
    config: &BlinkConfig,
    offer: String,
    amount_msats: i64,
    payer_note: Option<String>,
) -> Result<crate::cln::types::FetchInvoiceResponse, ApiError> {
    crate::galoy::api::fetch_invoice_from_offer(&galoy(config), offer, amount_msats, payer_note)
        .await
}

pub async fn pay_offer(
    config: &BlinkConfig,
    offer: String,
    amount_msats: i64,
    payer_note: Option<String>,
) -> Result<PayInvoiceResponse, ApiError> {
    crate::galoy::api::pay_offer(&galoy(config), offer, amount_msats, payer_note).await
}

pub async fn lookup_invoice(
    config: &BlinkConfig,
    payment_hash: Option<String>,
    from: Option<i64>,
    limit: Option<i64>,
    search: Option<String>,
) -> Result<Transaction, ApiError> {
    crate::galoy::api::lookup_invoice(&galoy(config), payment_hash, from, limit, search).await
}

pub async fn list_transactions(
    config: &BlinkConfig,
    from: i64,
    limit: i64,
    search: Option<String>,
) -> Result<Vec<Transaction>, ApiError> {
    crate::galoy::api::list_transactions(&galoy(config), from, limit, search).await
}

pub async fn poll_invoice_events<F>(config: &BlinkConfig, params: OnInvoiceEventParams, callback: F)
where
    F: FnMut(String, Option<Transaction>),
{
    crate::galoy::api::poll_invoice_events(&galoy(config), params, callback).await
}

pub async fn on_invoice_events(
    config: BlinkConfig,
    params: OnInvoiceEventParams,
    callback: std::sync::Arc<dyn OnInvoiceEventCallback>,
) {
    crate::galoy::api::on_invoice_events(galoy(&config), params, callback).await
}
