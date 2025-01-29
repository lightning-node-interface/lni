use lni::{phoenixd::lib::PhoenixdConfig, phoenixd::lib::PhoenixdMakeInvoiceParams};
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
    PhoenixdConfig {
      url: self.inner.url.clone(),
      password: self.inner.password.clone(),
    }
  }

  #[napi]
  pub async fn get_info(&self) -> napi::Result<lni::NodeInfo> {
    let info = lni::phoenixd::api::get_info(self.inner.url.clone(), self.inner.password.clone())
      .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(info)
  }

  #[napi]
  pub async fn make_invoice(
    &self,
    params: PhoenixdMakeInvoiceParams,
  ) -> napi::Result<lni::Transaction> {
    let txn = lni::phoenixd::api::make_invoice(
      self.inner.url.clone(),
      self.inner.password.clone(),
      params.invoice_type,
      params.amount,
      params.description,
      params.description_hash,
      params.expiry,
    )
    .await
    .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(txn)
  }

  #[napi]
  pub async fn lookup_invoice(&self, payment_hash: String) -> napi::Result<lni::Transaction> {
    let txn = lni::phoenixd::api::lookup_invoice(
      self.inner.url.clone(),
      self.inner.password.clone(),
      payment_hash,
    )
    .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(txn)
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use dotenv::dotenv;
  use lazy_static::lazy_static;
  use std::env;
  use tokio::test;

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
      })
    };
  }

  #[test]
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
