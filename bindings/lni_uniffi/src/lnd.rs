use lni::lnd::LndConfig;
use lni::types::NodeInfo;
use lni::ApiError;

#[derive(uniffi::Object)]
pub struct LndNode {
    pub config: LndConfig,
}

#[uniffi::export]
impl LndNode {
    #[uniffi::constructor]
    fn new(
        url: String,
        macaroon: String,
        socks5_proxy: Option<String>,
        accept_invalid_certs: Option<bool>,
        http_timeout: Option<i64>,
    ) -> Self {
        Self {
            config: LndConfig {
                url,
                macaroon,
                socks5_proxy,
                accept_invalid_certs: Some(accept_invalid_certs.unwrap_or(false)),
                http_timeout: Some(http_timeout.unwrap_or(30)),
            },
        }
    }

    fn get_url(&self) -> String {
        self.config.url.clone()
    }

    fn get_macaroon(&self) -> String {
        self.config.macaroon.clone()
    }

    async fn get_info(&self) -> Result<NodeInfo, ApiError> {
        lni::lnd::api::get_info(&self.config).await
    }

    // pub async fn create_invoice(
    //     &self,
    //     params: CreateInvoiceParams,
    // ) -> napi::Result<lni::Transaction> {
    //     let txn = lni::lnd::api::create_invoice(&self.inner, params)
    //         .await
    //         .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    //     Ok(txn)
    // }

    // pub async fn pay_invoice(
    //     &self,
    //     params: PayInvoiceParams,
    // ) -> Result<lni::types::PayInvoiceResponse> {
    //     let invoice = lni::lnd::api::pay_invoice(&self.inner, params)
    //         .await
    //         .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    //     Ok(invoice)
    // }

    // pub async fn get_offer(&self, search: Option<String>) -> Result<lni::types::PayCode> {
    //     let offer = lni::lnd::api::get_offer(&self.inner, search)
    //         .await
    //         .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    //     Ok(offer)
    // }

    // pub async fn list_offers(&self, search: Option<String>) -> Result<Vec<lni::types::PayCode>> {
    //     let offers = lni::lnd::api::list_offers(&self.inner, search)
    //         .await
    //         .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    //     Ok(offers)
    // }

    // pub async fn pay_offer(
    //     &self,
    //     offer: String,
    //     amount_msats: i64,
    //     payer_note: Option<String>,
    // ) -> napi::Result<lni::PayInvoiceResponse> {
    //     let offer = lni::lnd::api::pay_offer(&self.inner, offer, amount_msats, payer_note)
    //         .await
    //         .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    //     Ok(offer)
    // }

    // pub async fn lookup_invoice(&self, payment_hash: String) -> napi::Result<lni::Transaction> {
    //     let txn = lni::lnd::api::lookup_invoice(&self.inner, Some(payment_hash))
    //         .await
    //         .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    //     Ok(txn)
    // }

    // pub async fn list_transactions(
    //     &self,
    //     params: lni::types::ListTransactionsParams,
    // ) -> napi::Result<Vec<lni::Transaction>> {
    //     let txns = lni::lnd::api::list_transactions(&self.inner, params.from, params.limit)
    //         .await
    //         .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    //     Ok(txns)
    // }

    // pub async fn decode(&self, str: String) -> Result<String> {
    //     let decoded = lni::lnd::api::decode(&self.inner, str)
    //         .await
    //         .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    //     Ok(decoded)
    // }
}
