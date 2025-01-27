use crate::{InvoiceType, NodeInfo, Transaction};
use serde::Deserialize;
use serde_json::json;

use super::lib::Bolt11Resp;

/// https://phoenix.acinq.co/server/api

#[derive(Debug, Deserialize)]
pub struct InfoResponse {
    #[serde(rename = "nodeId")] // Handle JSON field `nodeId`
    pub node_id: String,
}

// lookup_invoice
// list_transactions
// list_channels
// get_balance

// get_info
pub fn get_info(url: String, password: String) -> crate::Result<NodeInfo> {
    let url = format!("{}/getinfo", url);
    let client: reqwest::blocking::Client = reqwest::blocking::Client::new();
    let response = client.get(&url).basic_auth("", Some(password)).send();

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

pub async fn make_invoice(
    url: String,
    password: String,
    invoice_type: InvoiceType,
    amount: i64,
    description: String,
    description_hash: String,
    expiry: i64,
) -> crate::Result<Transaction> {
    let client = reqwest::blocking::Client::new();
    match invoice_type {
        InvoiceType::Bolt11 => {
            let req_url = format!("{}/createinvoice", url);

            let response: reqwest::blocking::Response = client
                .post(&req_url)
                .basic_auth("", Some(password))
                .json(&json!({
                    "amountSat": amount,
                    "description": description,
                    "expiry": expiry,
                }))
                .send()
                .unwrap();

            println!("Status: {}", response.status());

            let invoice_str = response.text().unwrap();
            println!("Bolt11 {}", &invoice_str.to_string());

            // Parse JSON string into Bolt11Resp
            let bolt11_resp: Bolt11Resp =
                serde_json::from_str(&invoice_str).map_err(|e| crate::ApiError::Json {
                    reason: e.to_string(),
                })?;

            Ok(Transaction {
                type_: "incoming".to_string(),
                invoice: bolt11_resp.serialized,
                preimage: "".to_string(),
                payment_hash: bolt11_resp.paymentHash,
                amount: amount,
                fees_paid: 0,
                created_at: 0,
                expires_at: expiry,
                settled_at: 0,
                description: description,
                description_hash: description_hash,
            })
        }
        InvoiceType::Bolt12 => {
            let req_url = format!("{}/getoffer", url);
            let response: reqwest::blocking::Response = client
                .get(&req_url)
                .basic_auth("", Some(password))
                .send()
                .unwrap();
            let offer_str = response.text().unwrap();
            Ok(Transaction {
                type_: "incoming".to_string(),
                invoice: offer_str,
                preimage: "".to_string(),
                payment_hash: "".to_string(),
                amount: amount,
                fees_paid: 0,
                created_at: 0,
                expires_at: expiry,
                settled_at: 0,
                description: description,
                description_hash: description_hash,
            })
        }
    }
}
