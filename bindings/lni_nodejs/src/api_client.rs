#[cfg(feature = "napi_rs")]
use napi::{JsObject, Result};
#[cfg(feature = "napi_rs")]
use napi_derive::napi;
use serde_json::json;
use std::sync::Arc;

#[cfg(feature = "napi_rs")]
#[napi]
pub struct Fetcher {}

// #[napi(object)]
// pub struct JsIp {
//   pub origin: String,
// }

#[cfg(feature = "napi_rs")]
#[napi]
impl Fetcher {
  #[napi(constructor)]
  pub fn new() -> Self {
    Self {}
  }

  #[napi]
  pub async fn get_ip_address(&self) -> Result<lni::Ip> {
    let ip = lni::get_ip_address()
      .await
      .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(ip)
  }
}

// #[cfg(test)]
// mod tests {
//     use super::*;
//     use tokio;

//     #[tokio::test]
//     async fn test_get_ip_address() {
//         let fetcher = Arc::new(Fetcher::new_rust());
//         let result = fetcher.get_ip_address().await;

//         match result {
//             Ok(ip) => {
//                 println!("IP Address: {:?}", ip.origin);
//                 assert!(!ip.origin.is_empty());
//             }
//             Err(e) => {
//                 panic!("Failed to get IP address: {:?}", e);
//             }
//         }
//     }
// }
