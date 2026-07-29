use super::BlinkConfig;
use crate::{
    ApiError, CreateInvoiceParams, Offer, OnInvoiceEventCallback, OnInvoiceEventParams,
    OnchainTransaction, PayInvoiceParams, PayInvoiceResponse, PayOnchainOptions,
    PayOnchainResponse, PrepareOnchainTransactionParams, Transaction,
};

fn galoy(config: &BlinkConfig) -> crate::galoy::GaloyConfig {
    config.into()
}

pub(crate) fn bolt12_not_implemented<T>() -> Result<T, ApiError> {
    Err(crate::error_normalization::nwc_error(
        "NOT_IMPLEMENTED",
        "Bolt12 not implemented for Blink",
    ))
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

pub async fn get_offer(_config: &BlinkConfig, _search: Option<String>) -> Result<Offer, ApiError> {
    bolt12_not_implemented()
}

pub async fn list_offers(
    _config: &BlinkConfig,
    _search: Option<String>,
) -> Result<Vec<Offer>, ApiError> {
    bolt12_not_implemented()
}

pub async fn create_offer(
    _config: &BlinkConfig,
    _amount_msats: Option<i64>,
    _description: Option<String>,
    _expiry: Option<i64>,
) -> Result<Transaction, ApiError> {
    bolt12_not_implemented()
}

pub async fn fetch_invoice_from_offer(
    _config: &BlinkConfig,
    _offer: String,
    _amount_msats: i64,
    _payer_note: Option<String>,
) -> Result<crate::cln::types::FetchInvoiceResponse, ApiError> {
    bolt12_not_implemented()
}

pub async fn pay_offer(
    _config: &BlinkConfig,
    _offer: String,
    _amount_msats: i64,
    _payer_note: Option<String>,
) -> Result<PayInvoiceResponse, ApiError> {
    bolt12_not_implemented()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn bolt12_errors_keep_the_legacy_blink_shape() {
        let error = get_offer(&BlinkConfig::default(), None)
            .await
            .expect_err("Blink Bolt12 should remain unsupported");

        assert!(matches!(
            error,
            ApiError::Nwc { ref code, ref message }
                if code == "NOT_IMPLEMENTED" && message == "Bolt12 not implemented for Blink"
        ));
    }
}
