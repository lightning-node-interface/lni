use lni::{
  lnbits::lib::LnBitsConfig, CreateInvoiceParams, LookupInvoiceParams, PayInvoiceParams,
};
use napi::bindgen_prelude::*;
use napi_derive::napi;

#[napi]
pub struct LnBitsNode {
  inner: LnBitsConfig,
}

#[napi]
impl LnBitsNode {
  #[napi(constructor)]
  pub fn new(config: LnBitsConfig) -> Self {
    Self { inner: config }
  }

  #[napi]
  pub fn get_base_url(&self) -> String {
    self.inner.base_url.as_ref().unwrap_or(&"https://demo.lnbits.com".to_string()).clone()
  }

  #[napi]
  pub fn get_api_key(&self) -> String {
    self.inner.api_key.clone()
  }

  #[napi]
  pub fn get_config(&self) -> LnBitsConfig {
    LnBitsConfig {
      base_url: self.inner.base_url.clone(),
      api_key: self.inner.api_key.clone(),
      socks5_proxy: self.inner.socks5_proxy.clone(),
      accept_invalid_certs: self.inner.accept_invalid_certs,
      http_timeout: self.inner.http_timeout,
    }
  }

  #[napi]
  pub async fn get_info(&self) -> Result<lni::types::NodeInfo> {
    let info = lni::lnbits::api::get_info(&self.inner)
      .await
      .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(info)
  }

  #[napi]
  pub async fn create_invoice(&self, params: CreateInvoiceParams) -> Result<lni::types::Transaction> {
    let txn = lni::lnbits::api::create_invoice(&self.inner, params)
      .await
      .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(txn)
  }

  #[napi]
  pub async fn pay_invoice(&self, params: PayInvoiceParams) -> Result<lni::types::PayInvoiceResponse> {
    let response = lni::lnbits::api::pay_invoice(&self.inner, params)
      .await
      .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(response)
  }

  #[napi]
  pub async fn lookup_invoice(&self, params: LookupInvoiceParams) -> Result<lni::types::Transaction> {
    let txn = lni::lnbits::api::lookup_invoice(
      &self.inner,
      params.payment_hash,
      None,
      None,
      params.search,
    )
    .await
    .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(txn)
  }

  #[napi]
  pub async fn list_transactions(&self, params: lni::types::ListTransactionsParams) -> Result<Vec<lni::types::Transaction>> {
    let txns = lni::lnbits::api::list_transactions(&self.inner, params.from, params.limit, params.search)
      .await
      .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(txns)
  }

  #[napi]
  pub async fn decode(&self, invoice_str: String) -> Result<String> {
    let decoded = lni::lnbits::api::decode(&self.inner, invoice_str)
      .await
      .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(decoded)
  }

  // These BOLT12 functions return not implemented errors
  #[napi]
  pub async fn get_offer(&self, search: Option<String>) -> Result<lni::types::PayCode> {
    let offer = lni::lnbits::api::get_offer(&self.inner, search)
      .await
      .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(offer)
  }

  #[napi]
  pub async fn list_offers(&self, search: Option<String>) -> Result<Vec<lni::types::PayCode>> {
    let offers = lni::lnbits::api::list_offers(&self.inner, search)
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
  ) -> Result<lni::types::PayInvoiceResponse> {
    let response = lni::lnbits::api::pay_offer(&self.inner, offer, amount_msats, payer_note)
      .await
      .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(response)
  }

  #[napi]
  pub fn on_invoice_events(
    &self,
    params: lni::types::OnInvoiceEventParams,
    callback: napi::JsFunction,
  ) -> Result<()> {
    // For simplicity, we'll just return an error indicating async callbacks are not yet implemented
    // This would need more complex implementation similar to other providers
    Err(napi::Error::from_reason("Invoice event callbacks not yet implemented for LNBits Node.js bindings".to_string()))
  }
}