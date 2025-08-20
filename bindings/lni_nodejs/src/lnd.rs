use lni::{lnd::lib::LndConfig, CreateInvoiceParams, LookupInvoiceParams, PayInvoiceParams};
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
      socks5_proxy: self.inner.socks5_proxy.clone(),
      accept_invalid_certs: self.inner.accept_invalid_certs,
      http_timeout: self.inner.http_timeout,
    }
  }

  #[napi]
  pub fn get_info(&self) -> napi::Result<lni::NodeInfo> {
    let info =
      lni::lnd::api::get_info(&self.inner).map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(info)
  }

  #[napi]
  pub fn create_invoice(&self, params: CreateInvoiceParams) -> napi::Result<lni::Transaction> {
    let txn = lni::lnd::api::create_invoice(&self.inner, params)
      .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(txn)
  }

  #[napi]
  pub fn pay_invoice(&self, params: PayInvoiceParams) -> Result<lni::types::PayInvoiceResponse> {
    let invoice = lni::lnd::api::pay_invoice(&self.inner, params)
      .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(invoice)
  }

  #[napi]
  pub fn get_offer(&self, search: Option<String>) -> Result<lni::types::PayCode> {
    let offer = lni::lnd::api::get_offer(&self.inner, search)
      .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(offer)
  }

  #[napi]
  pub fn list_offers(&self, search: Option<String>) -> Result<Vec<lni::types::PayCode>> {
    let offers = lni::lnd::api::list_offers(&self.inner, search)
      .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(offers)
  }

  #[napi]
  pub fn pay_offer(
    &self,
    offer: String,
    amount_msats: i64,
    payer_note: Option<String>,
  ) -> napi::Result<lni::PayInvoiceResponse> {
    let offer = lni::lnd::api::pay_offer(&self.inner, offer, amount_msats, payer_note)
      .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(offer)
  }

  #[napi]
  pub fn lookup_invoice(&self, params: LookupInvoiceParams) -> napi::Result<lni::Transaction> {
    let txn =
      lni::lnd::api::lookup_invoice(&self.inner, params.payment_hash, None, None, params.search)
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(txn)
  }

  #[napi]
  pub fn list_transactions(
    &self,
    params: lni::types::ListTransactionsParams,
  ) -> napi::Result<Vec<lni::Transaction>> {
    let txns =
      lni::lnd::api::list_transactions(&self.inner, params.from, params.limit, params.search)
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(txns)
  }

  #[napi]
  pub fn decode(&self, str: String) -> Result<String> {
    let decoded = lni::lnd::api::decode(&self.inner, str)
      .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(decoded)
  }

  #[napi]
  pub fn on_invoice_events<T: Fn(String, Option<lni::Transaction>) -> Result<()>>(
    &self,
    params: lni::types::OnInvoiceEventParams,
    callback: T,
  ) -> Result<()> {
    lni::lnd::api::poll_invoice_events(&self.inner, params, move |status, tx| {
      callback(status.clone(), tx.clone()).map_err(|err| napi::Error::from_reason(err.to_string()));
    });
    Ok(())
  }
}
