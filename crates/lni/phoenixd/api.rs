use crate::phoenixd::lib::Bolt11Resp;
use crate::{ApiError, NodeInfo};

// https://phoenix.acinq.co/server/api
#[derive(Debug, Deserialize)]
pub struct InfoResponse {
    #[serde(rename = "nodeId")] // Handle JSON field `nodeId`
    pub node_id: String,
}

// make_invoice(bolt11, bolt12)
// lookup_invoice
// list_transactions
// list_channels
// get_balance
   /// Retrieves node information and prints the raw response for debugging.

// get_info
pub fn get_info(url: String, password: String) -> crate::Result<NodeInfo> {
    let url = format!("{}/getinfo", url);
    let client = Client::builder()
            .timeout(Duration::from_secs(5))
            .build().unwrap();
    let response = client
        .get(&url)
        .header("Authorization", format!("Basic {}", password))
        .send();

    // Print the raw response body for debugging
    let response_text = response.unwrap().text().unwrap();
    println!("Raw response: {}", response_text);

    // Deserialize the response into the InfoResponse struct
    let info: InfoResponse = serde_json::from_str(&response_text)?;

    let node_info = NodeInfo {
        alias: "Phoenixd".to_string(),
        color: "".to_string(),
        pubkey: info.node_id,
        network: "bitcoin".to_string(),
        block_height: 0,
        block_hash: "".to_string(),
    };
    Ok(node_info)
}

// pub async fn get_offer(url: String, password: String) -> crate::Result<String> {
//     let client: reqwest::blocking::Client = reqwest::blocking::Client::new();
//     let req_url = format!("{}/getoffer", url);
//     let response: reqwest::blocking::Response = client
//         .get(&req_url)
//         .basic_auth("", Some(password))
//         .send()
//         .unwrap();
//     let offer_str = response.text().unwrap();
//     Ok(offer_str)
// }

// pub async fn create_bolt_11_invoice(url: String, password: String) -> crate::Result<Bolt11Resp> {
//     let client = reqwest::blocking::Client::new();
//     let req_url = format!("{}/createinvoice", url);

//     let response: reqwest::blocking::Response = client
//         .get(&req_url)
//         .basic_auth("", Some(password))
//         .send()
//         .unwrap();

//     let invoice_str = response.text().unwrap();

//     // Parse JSON string into Bolt11Resp
//     let bolt11_resp: Bolt11Resp =
//         serde_json::from_str(&invoice_str).map_err(|e| crate::ApiError::Json {
//             reason: e.to_string(),
//         })?;

//     Ok(bolt11_resp)
// }

use reqwest::blocking::Client;
use serde::Deserialize;
use std::error::Error;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct PhoenixService {
    address: String,
    authorization: String,
    client: Client,
}



impl PhoenixService {
    /// Creates a new `PhoenixService` instance.
    pub fn new(address: String, authorization: String) -> crate::Result<Self> {
        let authorization_base64 = base64::encode(format!(":{}", authorization));
        let address = if address.starts_with("http") {
            address.to_string()
        } else {
            format!("http://{}", address)
        };

        let client = Client::builder()
            .timeout(Duration::from_secs(5))
            .build().unwrap();

        Ok(Self {
            address,
            authorization: authorization_base64,
            client,
        })
    }

 
}

#[cfg(test)]
mod tests {
    use super::*;
    use dotenv::dotenv;
    use std::env;

    #[test]
    fn test_get_info() {
        // Load environment variables
        dotenv().ok();

        let address = env::var("PHOENIXD_URL").expect("PHOENIXD_URL must be set");
        let authorization = env::var("PHOENIXD_PASSWORD").expect("PHOENIXD_PASSWORD must be set");

        // Create a new PhoenixService instance
        let service = PhoenixService::new(address, authorization)
            .expect("Failed to create PhoenixService");

        // Test get_info method
        match service.get_info() {
            Ok(info) => {
                if let node_id = info.node_id {
                    println!("Node ID: {}", node_id);
                    assert!(!node_id.is_empty(), "Node ID should not be empty");
                } else {
                    println!("Node ID is missing");
                    panic!("Node ID is missing in the response");
                }
            }
            Err(e) => {
                panic!("Failed to get node info: {:?}", e);
            }
        }
    }
}
