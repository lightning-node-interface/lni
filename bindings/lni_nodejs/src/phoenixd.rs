use lni::{
  phoenixd::lib::PhoenixdConfig, CreateInvoiceParams, CreateOfferParams, LookupInvoiceParams, PayInvoiceParams,
};
use napi::bindgen_prelude::*;
use napi_derive::napi;

#[napi]
pub struct PhoenixdNode {
  inner: PhoenixdConfig,
}

#[napi]
impl PhoenixdNode {
  #[napi(constructor)]
  pub fn new(config: PhoenixdConfig) -> Self {
    Self { inner: config }
  }

  #[napi]
  pub fn get_url(&self) -> String {
    self.inner.url.clone()
  }

  #[napi]
  pub fn get_password(&self) -> String {
    self.inner.password.clone()
  }

  #[napi]
  pub fn get_config(&self) -> PhoenixdConfig {
    self.inner.clone()
  }

  #[napi]
  pub async fn get_info(&self) -> napi::Result<lni::NodeInfo> {
    let info = lni::phoenixd::api::get_info(self.inner.clone())
      .await.map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(info)
  }

  #[napi]
  pub async fn create_invoice(&self, params: CreateInvoiceParams) -> napi::Result<lni::Transaction> {
    let txn = lni::phoenixd::api::create_invoice(
      self.inner.clone(),
      params.invoice_type.unwrap_or(lni::InvoiceType::Bolt11),
      params.amount_msats,
      params.description,
      params.description_hash,
      params.expiry,
    )
    .await.map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(txn)
  }

  #[napi]
  pub async fn pay_invoice(&self, params: PayInvoiceParams) -> Result<lni::types::PayInvoiceResponse> {
    let invoice = lni::phoenixd::api::pay_invoice(self.inner.clone(), params)
      .await.map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(invoice)
  }

  #[napi]
  pub async fn create_offer(&self, params: CreateOfferParams) -> Result<lni::Offer> {
    let offer = lni::phoenixd::api::create_offer(self.inner.clone(), params)
      .await.map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(offer)
  }

  #[napi]
  pub async fn get_offer(&self) -> Result<lni::Offer> {
    let offer = lni::phoenixd::api::get_offer(self.inner.clone())
      .await.map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(offer)
  }

  #[napi]
  pub async fn lookup_invoice(&self, params: LookupInvoiceParams) -> napi::Result<lni::Transaction> {
    let txn = lni::phoenixd::api::lookup_invoice(
      self.inner.clone(),
      params.payment_hash,
      None,
      None,
      params.search,
    )
    .await.map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(txn)
  }

  #[napi]
  pub async fn pay_offer(
    &self,
    offer: String,
    amount_msats: i64,
    payer_note: Option<String>,
  ) -> napi::Result<lni::PayInvoiceResponse> {
    let offer = lni::phoenixd::api::pay_offer(self.inner.clone(), offer, amount_msats, payer_note)
      .await.map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(offer)
  }

  #[napi]
  pub async fn list_transactions(
    &self,
    params: crate::ListTransactionsParams,
  ) -> napi::Result<Vec<lni::Transaction>> {
    let txns = lni::phoenixd::api::list_transactions(self.inner.clone(), params)
      .await.map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(txns)
  }

  #[napi]
  pub fn on_invoice_events<T: Fn(String, Option<lni::Transaction>) -> Result<()>>(
    &self,
    params: lni::types::OnInvoiceEventParams,
    callback: T,
  ) -> Result<()> {
    let config = self.inner.clone();
    
    // Block on the async function in the current thread, similar to LND's sync approach
    tokio::runtime::Runtime::new().unwrap().block_on(async {
      lni::phoenixd::api::poll_invoice_events(config, params, move |status, tx| {
        let _ = callback(status.clone(), tx.clone()).map_err(|err| napi::Error::from_reason(err.to_string()));
      }).await;
    });
    
    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use dotenv::dotenv;
  use lazy_static::lazy_static;
  use std::env;
  // use tokio::test;

  lazy_static! {
    static ref URL: String = {
      dotenv().ok();
      env::var("PHOENIXD_URL").expect("PHOENIXD_URL must be set")
    };
    static ref PASSWORD: String = {
      dotenv().ok();
      env::var("PHOENIXD_PASSWORD").expect("PHOENIXD_PASSWORD must be set")
    };
    static ref NODE: PhoenixdNode = {
      PhoenixdNode::new(PhoenixdConfig {
        url: URL.clone(),
        password: PASSWORD.clone(),
        ..Default::default()
      })
    };
  }

  #[tokio::test]
  async fn test_get_info() {
    match NODE.get_info().await {
      Ok(info) => {
        println!("info: {:?}", info.pubkey);
        assert!(!info.pubkey.is_empty(), "Node pubkey should not be empty");
      }
      Err(e) => {
        panic!("Failed to get offer: {:?}", e);
      }
    }
  }
}
