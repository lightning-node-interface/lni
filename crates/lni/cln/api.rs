use super::types::{FetchInvoiceResponse, InfoResponse, PayResponse};
use crate::types::NodeInfo;
use crate::{ApiError, PayInvoiceResponse};
use reqwest::header;

// https://docs.corelightning.org/reference/get_list_methods_resource

pub fn get_info(url: String, rune: String) -> Result<NodeInfo, ApiError> {
    let req_url = format!("{}/v1/getinfo", url);
    println!("Constructed URL: {} rune {}", req_url, rune);
    let client = clnrest_client(rune);
    let response = client.post(&req_url).send().unwrap();
    let response_text = response.text().unwrap();
    println!("Raw response: {}", response_text);
    let info: InfoResponse = serde_json::from_str(&response_text)?;

    let node_info = NodeInfo {
        alias: info.alias,
        color: info.color,
        pubkey: info.id,
        network: info.network,
        block_height: info.blockheight,
        block_hash: "".to_string(),
    };
    Ok(node_info)
}

pub async fn pay_offer(
    url: String,
    rune: String,
    offer: String,
    amount_msats: i64,
    payer_note: Option<String>,
) -> Result<PayInvoiceResponse, ApiError> {
    let fetch_invoice_url = format!("{}/v1/fetchinvoice", url);
    let client = clnrest_client(rune);
    let response: reqwest::blocking::Response = client
        .post(&fetch_invoice_url)
        .json(&serde_json::json!({
            "offer": offer,
            "amount_msat": amount_msats,
            "payer_note": payer_note,
        }))
        .send()
        .unwrap();
    let response_text = response.text().unwrap();
    let response_text = response_text.as_str();
    let fetch_invoice_resp: FetchInvoiceResponse = match serde_json::from_str(&response_text) {
        Ok(resp) => {
            println!("fetch_invoice_resp: {:?}", resp);
            resp
        }
        Err(e) => {
            return Err(ApiError::Json {
                reason: response_text.to_string(),
            })
        }
    };

    if (fetch_invoice_resp.invoice.is_empty()) {
        return Err(ApiError::Json {
            reason: "Missing BOLT 12 invoice".to_string(),
        });
    }

    // now pay the bolt 12 invoice lni
    let pay_url = format!("{}/v1/pay", url);
    let pay_response: reqwest::blocking::Response = client
        .post(&pay_url)
        .json(&serde_json::json!({"bolt11": fetch_invoice_resp.invoice}))
        .send()
        .unwrap();
    let pay_response_text = pay_response.text().unwrap();
    let pay_response_text = pay_response_text.as_str();
    let pay_resp: PayResponse = match serde_json::from_str(&pay_response_text) {
        Ok(resp) => resp,
        Err(e) => {
            return Err(ApiError::Json {
                reason: pay_response_text.to_string(),
            })
        }
    };

    Ok(PayInvoiceResponse {
        payment_hash: pay_resp.payment_hash,
        preimage: pay_resp.payment_preimage,
        fee: pay_resp.amount_sent_msat - pay_resp.amount_msat,
    })
}

fn clnrest_client(rune: String) -> reqwest::blocking::Client {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert("Rune", header::HeaderValue::from_str(&rune).unwrap());
    reqwest::blocking::ClientBuilder::new()
        .danger_accept_invalid_certs(true)
        .default_headers(headers)
        .build()
        .unwrap()
}
