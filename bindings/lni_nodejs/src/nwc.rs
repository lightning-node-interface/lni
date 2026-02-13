use lni::{
  nwc::lib::NwcConfig, CreateInvoiceParams, CreateOfferParams, LookupInvoiceParams, PayInvoiceParams,
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
  pub async fn get_info(&self) -> napi::Result<lni::NodeInfo> {
    let info = lni::nwc::api::get_info(self.inner.clone())
      .await
      .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(info)
  }

  #[napi]
  pub async fn create_invoice(
    &self,
    params: CreateInvoiceParams,
  ) -> napi::Result<lni::Transaction> {
    let txn = lni::nwc::api::create_invoice(self.inner.clone(), params)
      .await
      .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(txn)
  }

  #[napi]
  pub async fn pay_invoice(
    &self,
    params: PayInvoiceParams,
  ) -> napi::Result<lni::types::PayInvoiceResponse> {
    let invoice = lni::nwc::api::pay_invoice(self.inner.clone(), params)
      .await
      .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(invoice)
  }

  #[napi]
  pub async fn create_offer(&self, _params: CreateOfferParams) -> Result<lni::Offer> {
    Err(napi::Error::from_reason("NWC does not support offers (BOLT12) yet".to_string()))
  }

  #[napi]
  pub async fn get_offer(&self, search: Option<String>) -> Result<lni::Offer> {
    let offer = lni::nwc::api::get_offer(&self.inner, search)
      .await
      .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(offer)
  }

  #[napi]
  pub async fn list_offers(&self, search: Option<String>) -> Result<Vec<lni::Offer>> {
    let offers = lni::nwc::api::list_offers(&self.inner, search)
      .await
      .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(offers)
  }

  #[napi]
  pub async fn lookup_invoice(
    &self,
    params: LookupInvoiceParams,
  ) -> napi::Result<lni::Transaction> {
    let txn = lni::nwc::api::lookup_invoice(self.inner.clone(), params.payment_hash, params.search)
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
    let offer = lni::nwc::api::pay_offer(&self.inner.clone(), offer, amount_msats, payer_note)
      .await
      .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(offer)
  }

  #[napi]
  pub async fn list_transactions(
    &self,
    params: crate::ListTransactionsParams,
  ) -> napi::Result<Vec<lni::Transaction>> {
    let nwc_params = lni::ListTransactionsParams {
      from: params.from,
      limit: params.limit,
      payment_hash: params.payment_hash,
      search: params.search,
      created_after: params.created_after,
      created_before: params.created_before,
    };
    let txns = lni::nwc::api::list_transactions(self.inner.clone(), nwc_params)
      .await
      .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(txns)
  }

  #[napi]
  pub async fn decode(&self, str: String) -> Result<String> {
    let decoded = lni::nwc::api::decode(self.inner.clone(), str)
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
      lni::nwc::api::poll_invoice_events(&config, params, move |status, tx| {
        let _ = callback(status.clone(), tx.clone())
          .map_err(|err| napi::Error::from_reason(err.to_string()));
      })
      .await;
    });

    Ok(())
  }
}
