use lni::{phoenixd::lib::PhoenixdConfig, CreateInvoiceParams, PayInvoiceParams};
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
  pub fn get_info(&self) -> napi::Result<lni::NodeInfo> {
    let info = lni::phoenixd::api::get_info(&self.inner)
      .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(info)
  }

  #[napi]
  pub fn create_invoice(&self, params: CreateInvoiceParams) -> napi::Result<lni::Transaction> {
    let txn = lni::phoenixd::api::create_invoice(
      &self.inner,
      params.invoice_type,
      params.amount_msats,
      params.description,
      params.description_hash,
      params.expiry,
    )
    .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(txn)
  }

  #[napi]
  pub fn pay_invoice(&self, params: PayInvoiceParams) -> Result<lni::types::PayInvoiceResponse> {
    let invoice = lni::phoenixd::api::pay_invoice(&self.inner, params)
      .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(invoice)
  }

  #[napi]
  pub fn get_offer(&self) -> Result<lni::PayCode> {
    let paycode = lni::phoenixd::api::get_offer(&self.inner)
      .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(paycode)
  }

  #[napi]
  pub fn lookup_invoice(&self, payment_hash: String) -> napi::Result<lni::Transaction> {
    let txn = lni::phoenixd::api::lookup_invoice(&self.inner, payment_hash)
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
    let offer = lni::phoenixd::api::pay_offer(&self.inner, offer, amount_msats, payer_note)
      .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(offer)
  }

  #[napi]
  pub fn list_transactions(
    &self,
    params: crate::ListTransactionsParams,
  ) -> napi::Result<Vec<lni::Transaction>> {
    let txns = lni::phoenixd::api::list_transactions(&self.inner, params.from, params.limit, None)
      .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(txns)
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

  #[test]
  fn test_get_info() {
    match NODE.get_info() {
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
