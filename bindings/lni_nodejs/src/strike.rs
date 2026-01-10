use lni::{
  strike::lib::StrikeConfig,
  CreateInvoiceParams,
  CreateOfferParams,
  LookupInvoiceParams,
  PayInvoiceParams,
};
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
    self.inner.base_url.clone().unwrap_or_default()
  }

  #[napi]
  pub fn get_api_key(&self) -> String {
    self.inner.api_key.clone()
  }

  #[napi]
  pub fn get_config(&self) -> StrikeConfig {
    self.inner.clone()
  }

  // Async methods using tokio runtime
  #[napi]
  pub async fn get_info(&self) -> napi::Result<lni::NodeInfo> {
    let info = lni::strike::api::get_info(self.inner.clone()).await
      .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(info)
  }

  #[napi]
  pub async fn create_invoice(&self, params: CreateInvoiceParams) -> napi::Result<lni::Transaction> {
    let txn = lni::strike::api::create_invoice(self.inner.clone(), params).await
      .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(txn)
  }

  #[napi]
  pub async fn pay_invoice(&self, params: PayInvoiceParams) -> napi::Result<lni::types::PayInvoiceResponse> {
    let invoice = lni::strike::api::pay_invoice(self.inner.clone(), params).await
      .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(invoice)
  }

  #[napi]
  pub fn create_offer(&self, _params: CreateOfferParams) -> napi::Result<lni::Offer> {
    Err(napi::Error::from_reason("Bolt12 not implemented for Strike".to_string()))
  }

  #[napi]
  pub async fn lookup_invoice(&self, params: LookupInvoiceParams) -> napi::Result<lni::Transaction> {
    let txn = lni::strike::api::lookup_invoice(
      self.inner.clone(),
      params.payment_hash,
      None,
      None,
      params.search,
    ).await
    .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(txn)
  }

  #[napi]
  pub async fn list_transactions(
    &self,
    params: crate::ListTransactionsParams,
  ) -> napi::Result<Vec<lni::Transaction>> {
    let txns = lni::strike::api::list_transactions(self.inner.clone(), params.from, params.limit, params.search).await
      .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(txns)
  }

  #[napi]
  pub fn get_offer(&self, search: Option<String>) -> napi::Result<lni::Offer> {
    let offer = lni::strike::api::get_offer(&self.inner, search)
      .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(offer)
  }

  #[napi]
  pub async fn list_offers(&self, search: Option<String>) -> napi::Result<Vec<lni::Offer>> {
    let offers = lni::strike::api::list_offers(&self.inner, search)
      .await.map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(offers)
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
  pub fn decode(&self, str: String) -> napi::Result<String> {
    let decoded = lni::strike::api::decode(&self.inner, str)
      .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(decoded)
  }

  #[napi]
  pub fn on_invoice_events<T: Fn(String, Option<lni::Transaction>) -> napi::Result<()>>(
    &self,
    params: lni::types::OnInvoiceEventParams,
    callback: T,
  ) -> napi::Result<()> {
    let config = self.inner.clone();
    let rt = tokio::runtime::Runtime::new().unwrap();
    
    rt.block_on(async move {
      lni::strike::api::poll_invoice_events(config, params, move |status, transaction| {
        let _ = callback(status, transaction);
      }).await;
    });
    
    Ok(())
  }
}
