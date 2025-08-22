use lni::{
  strike::lib::StrikeConfig, CreateInvoiceParams, LookupInvoiceParams, PayInvoiceParams,
};
use napi::bindgen_prelude::*;
use napi_derive::napi;

#[napi]
pub struct StrikeNode {
  inner: StrikeConfig,
}

#[napi]
impl StrikeNode {
  #[napi(constructor)]
  pub fn new(config: StrikeConfig) -> Self {
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
  pub fn get_config(&self) -> StrikeConfig {
    self.inner.clone()
  }

  #[napi]
  pub fn get_info(&self) -> napi::Result<lni::NodeInfo> {
    let info = lni::strike::api::get_info(&self.inner)
      .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(info)
  }

  #[napi]
  pub fn create_invoice(&self, params: CreateInvoiceParams) -> napi::Result<lni::Transaction> {
    let txn = lni::strike::api::create_invoice(&self.inner, params)
      .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(txn)
  }

  #[napi]
  pub fn pay_invoice(&self, params: PayInvoiceParams) -> Result<lni::types::PayInvoiceResponse> {
    let invoice = lni::strike::api::pay_invoice(&self.inner, params)
      .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(invoice)
  }

  #[napi]
  pub fn get_offer(&self, search: Option<String>) -> Result<lni::PayCode> {
    let paycode = lni::strike::api::get_offer(&self.inner, search)
      .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(paycode)
  }

  #[napi]
  pub fn list_offers(&self, search: Option<String>) -> Result<Vec<lni::PayCode>> {
    let paycodes = lni::strike::api::list_offers(&self.inner, search)
      .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(paycodes)
  }

  #[napi]
  pub fn lookup_invoice(&self, params: LookupInvoiceParams) -> napi::Result<lni::Transaction> {
    let txn = lni::strike::api::lookup_invoice(
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
    let offer = lni::strike::api::pay_offer(&self.inner, offer, amount_msats, payer_note)
      .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(offer)
  }

  #[napi]
  pub fn list_transactions(
    &self,
    params: crate::ListTransactionsParams,
  ) -> napi::Result<Vec<lni::Transaction>> {
    let txns = lni::strike::api::list_transactions(&self.inner, params.from, params.limit, params.search)
      .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(txns)
  }

  #[napi]
  pub fn decode(&self, str: String) -> Result<String> {
    let decoded = lni::strike::api::decode(&self.inner, str)
      .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(decoded)
  }

  #[napi]
  pub fn on_invoice_events<T: Fn(String, Option<lni::Transaction>) -> Result<()>>(
    &self,
    params: lni::types::OnInvoiceEventParams,
    callback: T,
  ) -> Result<()> {
    lni::strike::api::poll_invoice_events(&self.inner, params, move |status, tx| {
      let _ = callback(status.clone(), tx.clone());
    });
    Ok(())
  }
}
