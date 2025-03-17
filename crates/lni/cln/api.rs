use super::types::{
    Bolt11Resp, Bolt12Resp, FetchInvoiceResponse, InfoResponse, InvoicesResponse, PayResponse,
};
use crate::types::NodeInfo;
use crate::{ApiError, InvoiceType, PayInvoiceResponse, Transaction};
use reqwest::header;

// https://docs.corelightning.org/reference/get_list_methods_resource

pub fn get_info(url: String, rune: String) -> Result<NodeInfo, ApiError> {
    let req_url = format!("{}/v1/getinfo", url);
    println!("Constructed URL: {} rune {}", req_url, rune);
    let client = clnrest_client(rune);
    let response = client
        .post(&req_url)
        .header("Content-Type", "application/json")
        .send()
        .unwrap();
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

// invoice - amount_msat label description expiry fallbacks preimage exposeprivatechannels cltv

pub async fn create_invoice(
    url: String,
    rune: String,
    invoice_type: InvoiceType,
    amount_msats: Option<i64>,
    description: Option<String>,
    description_hash: Option<String>,
    expiry: Option<i64>,
) -> Result<Transaction, ApiError> {
    let client = clnrest_client(rune);
    match invoice_type {
        InvoiceType::Bolt11 => {
            let description_clone = description.clone();
            let req_url = format!("{}/v1/invoice", url);
            let response: reqwest::blocking::Response = client
                .post(&req_url)
                .header("Content-Type", "application/json")
                .json(&serde_json::json!({
                    "description": description,
                    "amount_msat": amount_msats,
                    "expiry": expiry,
                    "label": format!("lni.{}", rand::random::<u32>()),
                }))
                .send()
                .unwrap();

            println!("Status: {}", response.status());

            let invoice_str = response.text().unwrap();
            let invoice_str = invoice_str.as_str();
            println!("Bolt11 {}", &invoice_str.to_string());

            let bolt11_resp: Bolt11Resp =
                serde_json::from_str(&invoice_str).map_err(|e| crate::ApiError::Json {
                    reason: e.to_string(),
                })?;

            Ok(Transaction {
                type_: "incoming".to_string(),
                invoice: bolt11_resp.bolt11,
                preimage: "".to_string(),
                payment_hash: bolt11_resp.payment_hash,
                amount_msats: amount_msats.unwrap_or(0),
                fees_paid: 0,
                created_at: 0,
                expires_at: expiry.unwrap_or(3600),
                settled_at: 0,
                description: description_clone.unwrap_or_default(),
                description_hash: description_hash.unwrap_or_default(),
                payer_note: Some("".to_string()),
                external_id: Some("".to_string()),
            })
        }
        InvoiceType::Bolt12 => {
            let req_url = format!("{}/v1/offer", url);
            let mut params: Vec<(&str, Option<String>)> = vec![];
            if let Some(amount_msats) = amount_msats {
                params.push(("amount", Some(format!("{}msat", amount_msats))))
            } else {
                params.push(("amount", Some("any".to_string())))
            }
            let description_clone = description.clone();
            if let Some(description) = description_clone {
                params.push(("description", Some(description)))
            }
            let response: reqwest::blocking::Response = client
                .post(&req_url)
                .header("Content-Type", "application/json")
                .json(&serde_json::json!(params
                    .into_iter()
                    .filter_map(|(k, v)| v.map(|v| (k, v)))
                    .collect::<serde_json::Value>()))
                .send()
                .unwrap();
            let offer_str = response.text().unwrap();
            let offer_str = offer_str.as_str();
            let bolt12resp: Bolt12Resp =
                serde_json::from_str(&offer_str).map_err(|e| crate::ApiError::Json {
                    reason: e.to_string(),
                })?;
            Ok(Transaction {
                type_: "incoming".to_string(),
                invoice: bolt12resp.bolt12,
                preimage: "".to_string(),
                payment_hash: "".to_string(),
                amount_msats: amount_msats.unwrap_or(0),
                fees_paid: 0,
                created_at: 0,
                expires_at: expiry.unwrap_or_default(),
                settled_at: 0,
                description: description.unwrap_or_default(),
                description_hash: description_hash.unwrap_or_default(),
                payer_note: Some("".to_string()),
                external_id: Some(bolt12resp.offer_id.unwrap_or_default()),
            })
        }
    }
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
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({
            "offer": offer,
            "amount_msat": amount_msats,
            "payer_note": payer_note,
            "timeout": 60,
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
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({
            "bolt11": fetch_invoice_resp.invoice.to_string(),
            "maxfeepercent": 1, // TODO read from config
            "retry_for": 60,
        }))
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

// label, invstring, payment_hash, offer_id, index, start, limit
pub fn lookup_invoice(
    url: String,
    rune: String,
    payment_hash: Option<String>,
    from: Option<i64>,
    limit: Option<i64>,
) -> Result<Vec<Transaction>, ApiError> {
    let list_invoices_url = format!("{}/v1/listinvoices", url);
    let client = clnrest_client(rune);

    // 1) Build query for incoming transactions
    let mut params: Vec<(&str, Option<String>)> = vec![];
    if let Some(from_value) = from {
        params.push(("start", Some(from_value.to_string())));
        params.push(("index", Some("created".to_string())));
    }
    if let Some(limit_value) = limit {
        params.push(("limit", Some(limit_value.to_string())));
    }
    if let Some(payment_hash_value) = payment_hash {
        params.push(("payment_hash", Some(payment_hash_value)));
    }

    // Fetch incoming transactions
    let response: reqwest::blocking::Response = client
        .post(&list_invoices_url)
        .header("Content-Type", "application/json")
        //.json(&serde_json::json!(params))
        .json(&serde_json::json!(params
            .into_iter()
            .filter_map(|(k, v)| v.map(|v| (k, v)))
            .collect::<serde_json::Value>()))
        .send()
        .unwrap();
    let response_text = response.text().unwrap();
    let response_text = response_text.as_str();
    let incoming_payments: InvoicesResponse = serde_json::from_str(&response_text).unwrap();

    // Convert incoming payments into "incoming" Transaction
    let mut transactions: Vec<Transaction> = incoming_payments
        .invoices
        .into_iter()
        .map(|inv| {
            Transaction {
                type_: "incoming".to_string(),
                invoice: inv.bolt11.unwrap_or_else(|| inv.bolt12.unwrap_or_default()),
                preimage: inv.payment_preimage.unwrap_or("".to_string()),
                payment_hash: inv.payment_hash,
                amount_msats: inv.amount_received_msat.unwrap_or(0),
                fees_paid: 0,
                created_at: 0, // TODO
                expires_at: inv.expires_at,
                settled_at: inv.paid_at.unwrap_or(0),
                description: inv.description.unwrap_or("".to_string()),
                description_hash: "".to_string(),
                payer_note: Some(inv.invreq_payer_note.unwrap_or("".to_string())),
                external_id: Some(inv.label),
            }
        })
        .collect();

    // Sort by created date descending
    transactions.sort_by(|a, b| b.created_at.cmp(&a.created_at));

    Ok(transactions)
}

pub fn list_transactions(
    url: String,
    rune: String,
    from: i64,
    limit: i64,
) -> Result<Vec<Transaction>, ApiError> {
    match lookup_invoice(url, rune, None, Some(from), Some(limit)) {
        Ok(transactions) => Ok(transactions),
        Err(e) => Err(e),
    }
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
