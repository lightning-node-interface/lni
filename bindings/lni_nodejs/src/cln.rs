use lni::{cln::lib::ClnConfig, CreateInvoiceParams, LookupInvoiceParams, PayInvoiceParams};
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
    self.inner.clone()
  }

  #[napi]
  pub fn get_info(&self) -> napi::Result<lni::NodeInfo> {
    let info =
      lni::cln::api::get_info(&self.inner).map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(info)
  }

  #[napi]
  pub fn create_invoice(&self, params: CreateInvoiceParams) -> napi::Result<lni::Transaction> {
    let txn = lni::cln::api::create_invoice(
      &self.inner,
      params.invoice_type,
      params.amount_msats,
      params.offer,
      params.description,
      params.description_hash,
      params.expiry,
    )
    .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(txn)
  }

  #[napi]
  pub fn pay_invoice(&self, params: PayInvoiceParams) -> Result<lni::types::PayInvoiceResponse> {
    let invoice = lni::cln::api::pay_invoice(&self.inner, params)
      .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(invoice)
  }

  #[napi]
  pub fn get_offer(&self, search: Option<String>) -> Result<lni::types::PayCode> {
    let offer = lni::cln::api::get_offer(&self.inner, search)
      .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(offer)
  }

  #[napi]
  pub fn list_offers(&self, search: Option<String>) -> Result<Vec<lni::types::PayCode>> {
    let offers = lni::cln::api::list_offers(&self.inner, search)
      .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(offers)
  }

  #[napi]
  pub fn pay_offer(
    &self,
    offer: String,
    amount_msats: i64,
    payer_note: Option<String>,
  ) -> napi::Result<lni::PayInvoiceResponse> {
    let offer = lni::cln::api::pay_offer(&self.inner, offer, amount_msats, payer_note)
      .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(offer)
  }

  #[napi]
  pub fn lookup_invoice(&self, params: LookupInvoiceParams) -> napi::Result<lni::Transaction> {
    let txn =
      lni::cln::api::lookup_invoice(&self.inner, params.payment_hash, None, None, params.search)
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(txn)
  }

  #[napi]
  pub fn list_transactions(
    &self,
    params: lni::types::ListTransactionsParams,
  ) -> napi::Result<Vec<lni::Transaction>> {
    let txns =
      lni::cln::api::list_transactions(&self.inner, params.from, params.limit, params.search)
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(txns)
  }

  #[napi]
  pub fn decode(&self, str: String) -> Result<String> {
    let decoded = lni::cln::api::decode(&self.inner, str)
      .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(decoded)
  }

  #[napi]
  pub fn on_invoice_events<T: Fn(String, Option<lni::Transaction>) -> Result<()>>(
    &self,
    params: lni::types::OnInvoiceEventParams,
    callback: T,
  ) -> Result<()> {
    lni::cln::api::poll_invoice_events(&self.inner, params, move |status, tx| {
      callback(status.clone(), tx.clone()).map_err(|err| napi::Error::from_reason(err.to_string()));
    });
    Ok(())
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
//    fn test_get_info() {
//     match NODE.get_info() {
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
