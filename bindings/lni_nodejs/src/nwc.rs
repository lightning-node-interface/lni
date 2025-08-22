use lni::{
  nwc::lib::NwcConfig, CreateInvoiceParams, LookupInvoiceParams, PayInvoiceParams,
};
use napi::bindgen_prelude::*;
use napi_derive::napi;

#[napi]
pub struct NwcNode {
  inner: NwcConfig,
}

#[napi]
impl NwcNode {
  #[napi(constructor)]
  pub fn new(config: NwcConfig) -> Self {
    Self { inner: config }
  }

  #[napi]
  pub fn get_nwc_uri(&self) -> String {
    self.inner.nwc_uri.clone()
  }

  #[napi]
  pub fn get_socks5_proxy(&self) -> Option<String> {
    self.inner.socks5_proxy.clone()
  }

  #[napi]
  pub fn get_config(&self) -> NwcConfig {
    self.inner.clone()
  }

  #[napi]
  pub fn get_info(&self) -> napi::Result<lni::NodeInfo> {
    let info = lni::nwc::api::get_info(&self.inner)
      .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(info)
  }

  #[napi]
  pub fn create_invoice(&self, params: CreateInvoiceParams) -> napi::Result<lni::Transaction> {
    let txn = lni::nwc::api::create_invoice(&self.inner, params)
      .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(txn)
  }

  #[napi]
  pub fn pay_invoice(&self, params: PayInvoiceParams) -> Result<lni::types::PayInvoiceResponse> {
    let invoice = lni::nwc::api::pay_invoice(&self.inner, params)
      .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(invoice)
  }

  #[napi]
  pub fn get_offer(&self, search: Option<String>) -> Result<lni::PayCode> {
    let paycode = lni::nwc::api::get_offer(&self.inner, search)
      .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(paycode)
  }

  #[napi]
  pub fn list_offers(&self, search: Option<String>) -> Result<Vec<lni::PayCode>> {
    let paycodes = lni::nwc::api::list_offers(&self.inner, search)
      .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(paycodes)
  }

  #[napi]
  pub fn lookup_invoice(&self, params: LookupInvoiceParams) -> napi::Result<lni::Transaction> {
    let txn = lni::nwc::api::lookup_invoice(
      &self.inner,
      params.payment_hash,
      params.search,
    )
    .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(txn)
  }

  #[napi]
  pub fn pay_offer(
    &self,
    offer: String,
    amount_msats: i64,
    payer_note: Option<String>,
  ) -> napi::Result<lni::PayInvoiceResponse> {
    let offer = lni::nwc::api::pay_offer(&self.inner, offer, amount_msats, payer_note)
      .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(offer)
  }

  #[napi]
  pub fn list_transactions(
    &self,
    params: crate::ListTransactionsParams,
  ) -> napi::Result<Vec<lni::Transaction>> {
    let nwc_params = lni::ListTransactionsParams {
      from: params.from,
      limit: params.limit,
      payment_hash: params.payment_hash,
      search: params.search,
    };
    let txns = lni::nwc::api::list_transactions(&self.inner, nwc_params)
      .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(txns)
  }

  #[napi]
  pub fn decode(&self, str: String) -> Result<String> {
    let decoded = lni::nwc::api::decode(&self.inner, str)
      .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(decoded)
  }

  #[napi]
  pub fn on_invoice_events<T: Fn(String, Option<lni::Transaction>) -> Result<()>>(
    &self,
    params: lni::types::OnInvoiceEventParams,
    callback: T,
  ) -> Result<()> {
    lni::nwc::api::poll_invoice_events(&self.inner, params, move |status, tx| {
      let _ = callback(status.clone(), tx.clone());
    });
    Ok(())
  }
}
