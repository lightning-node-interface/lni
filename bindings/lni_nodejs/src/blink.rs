use lni::{
  blink::lib::BlinkConfig, CreateInvoiceParams, LookupInvoiceParams, PayInvoiceParams,
};
use napi::bindgen_prelude::*;
use napi_derive::napi;

#[napi]
pub struct BlinkNode {
  inner: BlinkConfig,
}

#[napi]
impl BlinkNode {
  #[napi(constructor)]
  pub fn new(config: BlinkConfig) -> Self {
    Self { inner: config }
  }

  #[napi]
  pub fn get_base_url(&self) -> String {
    self.inner.base_url.clone()
  }

  #[napi]
  pub fn get_api_key(&self) -> String {
    self.inner.api_key.clone()
  }

  #[napi]
  pub fn get_config(&self) -> BlinkConfig {
    self.inner.clone()
  }

  #[napi]
  pub fn get_info(&self) -> napi::Result<lni::NodeInfo> {
    let info = lni::blink::api::get_info(&self.inner)
      .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(info)
  }

  #[napi]
  pub fn create_invoice(&self, params: CreateInvoiceParams) -> napi::Result<lni::Transaction> {
    let txn = lni::blink::api::create_invoice(&self.inner, params)
      .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(txn)
  }

  #[napi]
  pub fn pay_invoice(&self, params: PayInvoiceParams) -> Result<lni::types::PayInvoiceResponse> {
    let invoice = lni::blink::api::pay_invoice(&self.inner, params)
      .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(invoice)
  }

  #[napi]
  pub fn get_offer(&self, search: Option<String>) -> Result<lni::PayCode> {
    let paycode = lni::blink::api::get_offer(&self.inner, search)
      .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(paycode)
  }

  #[napi]
  pub fn list_offers(&self, search: Option<String>) -> Result<Vec<lni::PayCode>> {
    let paycodes = lni::blink::api::list_offers(&self.inner, search)
      .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(paycodes)
  }

  #[napi]
  pub fn lookup_invoice(&self, params: LookupInvoiceParams) -> napi::Result<lni::Transaction> {
    let txn = lni::blink::api::lookup_invoice(
      &self.inner,
      params.payment_hash,
      None,
      None,
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
    let offer = lni::blink::api::pay_offer(&self.inner, offer, amount_msats, payer_note)
      .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(offer)
  }

  #[napi]
  pub fn list_transactions(
    &self,
    params: crate::ListTransactionsParams,
  ) -> napi::Result<Vec<lni::Transaction>> {
    let txns = lni::blink::api::list_transactions(&self.inner, params.from, params.limit, params.search)
      .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(txns)
  }

  #[napi]
  pub fn decode(&self, str: String) -> Result<String> {
    let decoded = lni::blink::api::decode(&self.inner, str)
      .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(decoded)
  }

  #[napi]
  pub fn on_invoice_events(
    &self,
    _params: lni::types::OnInvoiceEventParams,
  ) -> Result<()> {
    // For now, we'll implement a simple polling mechanism
    // TODO: Implement proper callback support for Node.js bindings
    Err(napi::Error::from_reason("on_invoice_events not yet implemented for Node.js bindings".to_string()))
  }
}
