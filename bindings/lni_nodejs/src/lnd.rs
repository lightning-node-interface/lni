use lni::{lnd::lib::LndConfig, CreateInvoiceParams, PayInvoiceParams};
use napi::bindgen_prelude::*;
use napi_derive::napi;
#[napi]
pub struct LndNode {
  inner: LndConfig,
}

#[napi]
impl LndNode {
  #[napi(constructor)]
  pub fn new(config: LndConfig) -> Self {
    Self { inner: config }
  }

  #[napi]
  pub fn get_url(&self) -> String {
    self.inner.url.clone()
  }

  #[napi]
  pub fn get_macaroon(&self) -> String {
    self.inner.macaroon.clone()
  }

  #[napi]
  pub fn get_config(&self) -> LndConfig {
    LndConfig {
      url: self.inner.url.clone(),
      macaroon: self.inner.macaroon.clone(),
    }
  }

  #[napi]
  pub async fn get_info(&self) -> napi::Result<lni::NodeInfo> {
    let info = lni::lnd::api::get_info(self.inner.url.clone(), self.inner.macaroon.clone())
      .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(info)
  }

  #[napi]
  pub async fn create_invoice(
    &self,
    params: CreateInvoiceParams,
  ) -> napi::Result<lni::Transaction> {
    let txn =
      lni::lnd::api::create_invoice(self.inner.url.clone(), self.inner.macaroon.clone(), params)
        .await
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(txn)
  }

  #[napi]
  pub async fn pay_invoice(
    &self,
    params: PayInvoiceParams,
  ) -> Result<lni::types::PayInvoiceResponse> {
    let invoice =
      lni::lnd::api::pay_invoice(self.inner.url.clone(), self.inner.macaroon.clone(), params)
        .await
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(invoice)
  }

  #[napi]
  pub async fn get_offer(&self, search: Option<String>) -> Result<lni::types::PayCode> {
    let offer =
      lni::lnd::api::get_offer(self.inner.url.clone(), self.inner.macaroon.clone(), search)
        .await
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(offer)
  }

  #[napi]
  pub async fn list_offers(&self, search: Option<String>) -> Result<Vec<lni::types::PayCode>> {
    let offers =
      lni::lnd::api::list_offers(self.inner.url.clone(), self.inner.macaroon.clone(), search)
        .await
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(offers)
  }

  #[napi]
  pub async fn pay_offer(
    &self,
    offer: String,
    amount_msats: i64,
    payer_note: Option<String>,
  ) -> napi::Result<lni::PayInvoiceResponse> {
    let offer = lni::lnd::api::pay_offer(
      self.inner.url.clone(),
      self.inner.macaroon.clone(),
      offer,
      amount_msats,
      payer_note,
    )
    .await
    .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(offer)
  }

  #[napi]
  pub async fn lookup_invoice(&self, payment_hash: String) -> napi::Result<lni::Transaction> {
    let txn = lni::lnd::api::lookup_invoice(
      self.inner.url.clone(),
      self.inner.macaroon.clone(),
      Some(payment_hash),
    )
    .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(txn)
  }

  #[napi]
  pub async fn list_transactions(
    &self,
    params: lni::types::ListTransactionsParams,
  ) -> napi::Result<Vec<lni::Transaction>> {
    let txns = lni::lnd::api::list_transactions(
      self.inner.url.clone(),
      self.inner.macaroon.clone(),
      params.from,
      params.limit,
    )
    .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(txns)
  }

  #[napi]
  pub async fn decode(&self, str: String) -> Result<String> {
    let decoded = lni::lnd::api::decode(self.inner.url.clone(), self.inner.macaroon.clone(), str)
      .await
      .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(decoded)
  }
}
