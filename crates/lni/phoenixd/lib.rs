#[cfg(feature = "napi_rs")]
use napi_derive::napi;

use crate::phoenixd::api::*;
use serde::Deserialize;

#[cfg_attr(feature = "napi_rs", napi(object))]
pub struct PhoenixdConfig {
    pub url: String,
    pub password: String,
}

#[cfg_attr(feature = "napi_rs", napi(object))]
pub struct PhoenixdNode {
    pub url: String,
    pub password: String,
}

#[cfg_attr(feature = "napi_rs", napi(object))]
#[derive(Debug, Deserialize)]
pub struct Bolt11Resp {
    pub amountSat: i64,
    pub paymentHash: String,
    pub serialized: String,
}

impl PhoenixdNode {
    pub fn new(config: PhoenixdConfig) -> Self {
        Self {
            url: config.url,
            password: config.password,
        }
    }

    // pub async fn get_offer(&self) -> crate::Result<String> {
    //     get_offer(self.url.clone(), self.password.clone()).await
    // }

    // pub async fn create_bolt_11_invoice(&self) -> crate::Result<Bolt11Resp> {
    //     create_bolt_11_invoice(self.url.clone(), self.password.clone()).await
    // }
}

// #[cfg(test)]
// mod tests {
//     use super::*;
//     use dotenv::dotenv;
//     use lazy_static::lazy_static;
//     use std::env;
//     use tokio::test;

//     lazy_static! {
//         static ref URL: String = {
//             dotenv().ok();
//             env::var("PHOENIXD_URL").expect("PHOENIXD_URL must be set")
//         };
//         static ref PASSWORD: String = {
//             dotenv().ok();
//             env::var("PHOENIXD_PASSWORD").expect("PHOENIXD_PASSWORD must be set")
//         };
//         static ref NODE2: PhoenixdNode = {
//             PhoenixdNode::new(PhoenixdConfig {
//                 url: URL.clone(),
//                 password: PASSWORD.clone(),
//             })
//         };
//     }

//     // #[test]
//     // async fn test_get_offer() {
//     //     match NODE.get_offer().await {
//     //         Ok(offer) => {
//     //             println!("offer: {:?}", offer);
//     //             assert!(!offer.is_empty(), "Offer should not be empty");
//     //         }
//     //         Err(e) => {
//     //             panic!("Failed to get offer: {:?}", e);
//     //         }
//     //     }
//     // }

//     // #[tokio::test]
//     // async fn test_get_bolt11() {
//     //     match NODE.create_bolt_11_invoice().await {
//     //         Ok(offer) => {
//     //             println!("offer: {:?}", offer.serialized);
//     //             assert!(!offer.serialized.is_empty(), "Offer should not be empty");
//     //         }
//     //         Err(e) => {
//     //             panic!("Failed to get offer: {:?}", e);
//     //         }
//     //     }
//     // }
//     #[tokio::test]
//     async fn test_make_invoice() {
//         let amount = 1000;
//         let description = Some("Test invoice");
//         let description_hash = None;
//         let expiry = Some(3600);

//         match NODE
//             .make_invoice(amount, description, description_hash, expiry)
//             .await
//         {
//             Ok(invoice) => {
//                 println!("invoice: {:?}", invoice);
//                 assert!(!invoice.is_empty(), "Invoice should not be empty");
//             }
//             Err(e) => {
//                 panic!("Failed to make invoice: {:?}", e);
//             }
//         }
//     }
// }
