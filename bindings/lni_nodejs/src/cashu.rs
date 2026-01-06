use lni::{cashu::lib::CashuConfig, CreateInvoiceParams, CreateOfferParams, LookupInvoiceParams, PayInvoiceParams};
use napi::bindgen_prelude::*;
use napi_derive::napi;

#[napi]
pub struct CashuNode {
  inner: CashuConfig,
}

#[napi]
impl CashuNode {
  #[napi(constructor)]
  pub fn new(config: CashuConfig) -> Self {
    Self { inner: config }
  }

  #[napi]
  pub fn get_mint_url(&self) -> String {
    self.inner.mint_url.clone()
  }

  #[napi]
  pub fn get_config(&self) -> CashuConfig {
    self.inner.clone()
  }

  #[napi]
  pub async fn get_info(&self) -> napi::Result<lni::NodeInfo> {
    let info = lni::cashu::api::get_info(self.inner.clone())
      .await
      .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(info)
  }

  #[napi]
  pub async fn create_invoice(
    &self,
    params: CreateInvoiceParams,
  ) -> napi::Result<lni::Transaction> {
    let txn = lni::cashu::api::create_invoice(self.inner.clone(), params)
      .await
      .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(txn)
  }

  #[napi]
  pub async fn pay_invoice(
    &self,
    params: PayInvoiceParams,
  ) -> Result<lni::types::PayInvoiceResponse> {
    let invoice = lni::cashu::api::pay_invoice(self.inner.clone(), params)
      .await
      .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(invoice)
  }

  #[napi]
  pub async fn create_offer(&self, _params: CreateOfferParams) -> Result<lni::Offer> {
    Err(napi::Error::from_reason("Bolt12 not implemented for Cashu".to_string()))
  }

  #[napi]
  pub async fn get_offer(&self, search: Option<String>) -> Result<lni::Offer> {
    let offer = lni::cashu::api::get_offer(&self.inner, search)
      .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(offer)
  }

  #[napi]
  pub async fn list_offers(&self, search: Option<String>) -> Result<Vec<lni::Offer>> {
    let offers = lni::cashu::api::list_offers(&self.inner, search)
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
      lni::cashu::api::lookup_invoice(self.inner.clone(), params.payment_hash, None, None, params.search)
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
    let offer = lni::cashu::api::pay_offer(&self.inner, offer, amount_msats, payer_note)
      .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(offer)
  }

  #[napi]
  pub async fn list_transactions(
    &self,
    params: crate::ListTransactionsParams,
  ) -> napi::Result<Vec<lni::Transaction>> {
    let txns =
      lni::cashu::api::list_transactions(self.inner.clone(), params.from, params.limit, params.search)
        .await
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(txns)
  }

  #[napi]
  pub async fn decode(&self, str: String) -> Result<String> {
    let decoded = lni::cashu::api::decode(&self.inner, str)
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

    // Block on the async function in the current thread
    tokio::runtime::Runtime::new().unwrap().block_on(async {
      lni::cashu::api::poll_invoice_events(config, params, move |status, tx| {
        let _ = callback(status.clone(), tx.clone())
          .map_err(|err| napi::Error::from_reason(err.to_string()));
      })
      .await;
    });

    Ok(())
  }
}
