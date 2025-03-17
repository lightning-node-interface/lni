use lni::{cln::lib::ClnConfig, CreateInvoiceParams};
use napi::bindgen_prelude::*;
use napi_derive::napi;

#[napi]
pub struct ClnNode {
  inner: ClnConfig,
}

#[napi]
impl ClnNode {
  #[napi(constructor)]
  pub fn new(config: ClnConfig) -> Self {
    Self { inner: config }
  }

  #[napi]
  pub fn get_url(&self) -> String {
    self.inner.url.clone()
  }

  #[napi]
  pub fn get_rune(&self) -> String {
    self.inner.rune.clone()
  }

  #[napi]
  pub fn get_config(&self) -> ClnConfig {
    ClnConfig {
      url: self.inner.url.clone(),
      rune: self.inner.rune.clone(),
    }
  }

  #[napi]
  pub async fn get_info(&self) -> napi::Result<lni::NodeInfo> {
    let info = lni::cln::api::get_info(self.inner.url.clone(), self.inner.rune.clone())
      .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(info)
  }

  #[napi]
  pub async fn create_invoice(
    &self,
    params: CreateInvoiceParams,
  ) -> napi::Result<lni::Transaction> {
    let txn = lni::cln::api::create_invoice(
      self.inner.url.clone(),
      self.inner.rune.clone(),
      params.invoice_type,
      params.amount_msats,
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
    let txn = lni::cln::api::lookup_invoice(
      self.inner.url.clone(),
      self.inner.rune.clone(),
      Some(payment_hash),
      None,
      None,
    )
    .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(txn.into_iter().next().ok_or_else(|| napi::Error::from_reason("No transaction found"))?)
  }

  #[napi]
  pub async fn pay_offer(
    &self,
    offer: String,
    amount_msats: i64,
    payer_note: Option<String>,
  ) -> napi::Result<lni::PayInvoiceResponse> {
    let offer = lni::cln::api::pay_offer(
      self.inner.url.clone(),
      self.inner.rune.clone(),
      offer,
      amount_msats,
      payer_note,
    ).await
    .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(offer)
  }

  #[napi]
  pub async fn list_transactions(
    &self,
    params: lni::types::ListTransactionsParams,
  ) -> napi::Result<Vec<lni::Transaction>> {
    let txns = lni::cln::api::list_transactions(
      self.inner.url.clone(),
      self.inner.rune.clone(),
      params.from,
      params.limit,
    )
    .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(txns)
  }
}

// #[cfg(test)]
// mod tests {
//   use super::*;
//   use dotenv::dotenv;
//   use lazy_static::lazy_static;
//   use std::env;
//   use tokio::test;

//   lazy_static! {
//     static ref URL: String = {
//       dotenv().ok();
//       env::var("PHOENIXD_URL").expect("PHOENIXD_URL must be set")
//     };
//     static ref PASSWORD: String = {
//       dotenv().ok();
//       env::var("PHOENIXD_PASSWORD").expect("PHOENIXD_PASSWORD must be set")
//     };
//     static ref NODE: PhoenixdNode = {
//       PhoenixdNode::new(PhoenixdConfig {
//         url: URL.clone(),
//         password: PASSWORD.clone(),
//       })
//     };
//   }

//   #[test]
//   async fn test_get_info() {
//     match NODE.get_info().await {
//       Ok(info) => {
//         println!("info: {:?}", info.pubkey);
//         assert!(!info.pubkey.is_empty(), "Node pubkey should not be empty");
//       }
//       Err(e) => {
//         panic!("Failed to get offer: {:?}", e);
//       }
//     }
//   }
// }
