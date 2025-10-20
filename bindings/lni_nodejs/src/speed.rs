use lni::{speed::lib::SpeedConfig, CreateInvoiceParams, CreateOfferParams, LookupInvoiceParams, PayInvoiceParams};
use napi::bindgen_prelude::*;
use napi_derive::napi;

#[napi]
pub struct SpeedNode {
  inner: SpeedConfig,
}

#[napi]
impl SpeedNode {
  #[napi(constructor)]
  pub fn new(config: SpeedConfig) -> Self {
    Self { inner: config }
  }

  #[napi]
  pub fn get_base_url(&self) -> String {
    self.inner.base_url.clone().unwrap_or_default()
  }

  #[napi]
  pub fn get_api_key(&self) -> String {
    self.inner.api_key.clone()
  }

  #[napi]
  pub fn get_config(&self) -> SpeedConfig {
    self.inner.clone()
  }

  #[napi]
  pub async fn get_info(&self) -> napi::Result<lni::NodeInfo> {
    let info = lni::speed::api::get_info(&self.inner)
      .await
      .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(info)
  }

  #[napi]
  pub async fn create_invoice(
    &self,
    params: CreateInvoiceParams,
  ) -> napi::Result<lni::Transaction> {
    let txn = lni::speed::api::create_invoice(&self.inner, params)
      .await
      .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(txn)
  }

  #[napi]
  pub async fn pay_invoice(
    &self,
    params: PayInvoiceParams,
  ) -> napi::Result<lni::types::PayInvoiceResponse> {
    let invoice = lni::speed::api::pay_invoice(&self.inner, params)
      .await
      .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(invoice)
  }

  #[napi]
  pub async fn create_offer(&self, _params: CreateOfferParams) -> Result<lni::Offer> {
    Err(napi::Error::from_reason("Bolt12 not implemented for Speed".to_string()))
  }

  #[napi]
  pub async fn get_offer(&self, search: Option<String>) -> napi::Result<lni::Offer> {
    let offer = lni::speed::api::get_offer(&self.inner, search)
      .await
      .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(offer)
  }

  #[napi]
  pub async fn list_offers(&self, search: Option<String>) -> napi::Result<Vec<lni::Offer>> {
    let offers = lni::speed::api::list_offers(&self.inner, search)
      .await
      .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(offers)
  }

  #[napi]
  pub async fn lookup_invoice(
    &self,
    params: LookupInvoiceParams,
  ) -> napi::Result<lni::Transaction> {
    let txn =
      lni::speed::api::lookup_invoice(&self.inner, params.payment_hash, None, None, params.search)
        .await
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(txn)
  }

  #[napi]
  pub async fn pay_offer(
    &self,
    offer: String,
    amount_msats: i64,
    payer_note: Option<String>,
  ) -> napi::Result<lni::PayInvoiceResponse> {
    let offer = lni::speed::api::pay_offer(&self.inner, offer, amount_msats, payer_note)
      .await
      .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(offer)
  }

  #[napi]
  pub async fn list_transactions(
    &self,
    params: crate::ListTransactionsParams,
  ) -> napi::Result<Vec<lni::Transaction>> {
    let txns =
      lni::speed::api::list_transactions(&self.inner, params.from, params.limit, params.search)
        .await
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(txns)
  }

  #[napi]
  pub async fn decode(&self, str: String) -> napi::Result<String> {
    let decoded = lni::speed::api::decode(&self.inner, str)
      .await
      .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(decoded)
  }

  #[napi]
  pub fn on_invoice_events<T: Fn(String, Option<lni::Transaction>) -> Result<()>>(
    &self,
    params: lni::types::OnInvoiceEventParams,
    callback: T,
  ) -> Result<()> {
    let config = self.inner.clone();

    // Block on the async function in the current thread, similar to CLN's sync approach
    tokio::runtime::Runtime::new().unwrap().block_on(async {
      lni::speed::api::poll_invoice_events(&config, params, move |status, tx| {
        let _ = callback(status.clone(), tx.clone())
          .map_err(|err| napi::Error::from_reason(err.to_string()));
      })
      .await;
    });

    Ok(())
  }
}
