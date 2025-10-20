use lni::{
  lnd::lib::LndConfig, CreateInvoiceParams, CreateOfferParams, LookupInvoiceParams, PayInvoiceParams,
};
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

  // These BOLT12 functions are still synchronous
  #[napi]
  pub fn create_offer(&self, _params: CreateOfferParams) -> Result<lni::types::Offer> {
    // LND doesn't support BOLT12 offers yet
    Err(napi::Error::from_reason("Bolt12 not implemented for LND".to_string()))
  }

  #[napi]
  pub async fn get_offer(&self, search: Option<String>) -> Result<lni::types::Offer> {
    let offer = lni::lnd::api::get_offer(&self.inner, search)
      .await
      .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(offer)
  }

  #[napi]
  pub async fn list_offers(&self, search: Option<String>) -> Result<Vec<lni::types::Offer>> {
    let offers = lni::lnd::api::list_offers(&self.inner, search)
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
    let offer = lni::lnd::api::pay_offer(&self.inner, offer, amount_msats, payer_note)
      .await
      .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(offer)
  }

  // Async methods - using the actual async API functions
  #[napi]
  pub async fn get_info(&self) -> napi::Result<lni::NodeInfo> {
    let info = lni::lnd::api::get_info(self.inner.clone())
      .await
      .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(info)
  }

  #[napi]
  pub async fn create_invoice(
    &self,
    params: CreateInvoiceParams,
  ) -> napi::Result<lni::Transaction> {
    let txn = lni::lnd::api::create_invoice(self.inner.clone(), params)
      .await
      .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(txn)
  }

  #[napi]
  pub async fn pay_invoice(
    &self,
    params: PayInvoiceParams,
  ) -> Result<lni::types::PayInvoiceResponse> {
    let invoice = lni::lnd::api::pay_invoice(self.inner.clone(), params)
      .await
      .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(invoice)
  }

  #[napi]
  pub async fn lookup_invoice(
    &self,
    params: LookupInvoiceParams,
  ) -> napi::Result<lni::Transaction> {
    let txn = lni::lnd::api::lookup_invoice(
      self.inner.clone(),
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
  pub async fn list_transactions(
    &self,
    params: lni::types::ListTransactionsParams,
  ) -> napi::Result<Vec<lni::Transaction>> {
    let txns = lni::lnd::api::list_transactions(
      self.inner.clone(),
      Some(params.from),
      Some(params.limit),
      params.search,
    )
    .await
    .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(txns)
  }

  #[napi]
  pub async fn decode(&self, invoice_str: String) -> Result<String> {
    let decoded = lni::lnd::api::decode(self.inner.clone(), invoice_str)
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
      lni::lnd::api::poll_invoice_events(&config, params, move |status, tx| {
        let _ = callback(status.clone(), tx.clone())
          .map_err(|err| napi::Error::from_reason(err.to_string()));
      })
      .await;
    });

    Ok(())
  }

  #[napi]
  pub async fn get_offer_async(&self, _search: Option<String>) -> Result<lni::types::Offer> {
    // Since BOLT12 is not implemented, we return the same error asynchronously
    Err(napi::Error::from_reason(
      "Bolt12 not implemented".to_string(),
    ))
  }

  #[napi]
  pub async fn list_offers_async(
    &self,
    _search: Option<String>,
  ) -> Result<Vec<lni::types::Offer>> {
    // Since BOLT12 is not implemented, we return the same error asynchronously
    Err(napi::Error::from_reason(
      "Bolt12 not implemented".to_string(),
    ))
  }

  #[napi]
  pub async fn pay_offer_async(
    &self,
    _offer: String,
    _amount_msats: i64,
    _payer_note: Option<String>,
  ) -> napi::Result<lni::PayInvoiceResponse> {
    // Since BOLT12 is not implemented, we return the same error asynchronously
    Err(napi::Error::from_reason(
      "Bolt12 not implemented".to_string(),
    ))
  }

  #[napi]
  pub async fn create_offer_async(
    &self,
    _amount_msats: Option<i64>,
    _description: Option<String>,
    _expiry: Option<i64>,
  ) -> napi::Result<lni::Transaction> {
    // Since BOLT12 is not implemented, we return the same error asynchronously
    Err(napi::Error::from_reason(
      "Bolt12 not implemented".to_string(),
    ))
  }

  #[napi]
  pub async fn fetch_invoice_from_offer_async(
    &self,
    _offer: String,
    _amount_msats: i64,
    _payer_note: Option<String>,
  ) -> Result<String> {
    // Since BOLT12 is not implemented, we return the same error asynchronously
    Err(napi::Error::from_reason(
      "Bolt12 not implemented".to_string(),
    ))
  }
}
