use crate::{InvoiceType, NodeInfo, Transaction};
use serde::{Deserialize, Serialize};
use serde_urlencoded;
use super::lib::Bolt11Resp;

/// https://phoenix.acinq.co/server/api

// TODO
// list_channels
// get_balance

#[derive(Debug, Deserialize)]
pub struct InfoResponse {
    #[serde(rename = "nodeId")] // Handle JSON field `nodeId`
    pub node_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Bolt11Req {
    #[serde(rename = "amountSat")]
    pub amount_sat: i64,
    #[serde(rename = "expirySeconds")]
    pub expiry_seconds: i64,
    #[serde(rename = "externalId")]
    pub external_id: Option<String>,
    #[serde(rename = "description")]
    pub description: Option<String>,
    #[serde(rename = "webhookUrl")]
    pub webhook_url: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InvoiceResponse {
    #[serde(rename = "preimage")]
    pub preimage: String,
    #[serde(rename = "paymentHash")]
    pub payment_hash: String,
    #[serde(rename = "receivedSat")]
    pub received_sat: i64,
    #[serde(rename = "fees")]
    pub fees: i64,
    #[serde(rename = "completedAt")]
    pub completed_at: i64,
    #[serde(rename = "createdAt")]
    pub created_at: i64,
    #[serde(rename = "isPaid")]
    pub is_paid: bool,
    #[serde(rename = "payerNote")]
    pub payer_note: Option<String>,
    #[serde(rename = "payerKey")]
    pub payer_key: Option<String>,
    #[serde(rename = "invoice")]
    pub invoice: Option<String>,
    #[serde(rename = "description")]
    pub description: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OutgoingPaymentResponse {
    #[serde(rename = "paymentId")]
    pub payment_id: String,
    #[serde(rename = "preimage")]
    pub preimage: String,
    #[serde(rename = "paymentHash")]
    pub payment_hash: String,
    #[serde(rename = "sent")]
    pub sent: i64,
    #[serde(rename = "fees")]
    pub fees: i64,
    #[serde(rename = "createdAt")]
    pub created_at: i64,
    #[serde(rename = "completedAt")]
    pub completed_at: i64,
    #[serde(rename = "isPaid")]
    pub is_paid: bool,
}


pub fn get_info(url: String, password: String) -> crate::Result<NodeInfo> {
    let url = format!("{}/getinfo", url);
    let client: reqwest::blocking::Client = reqwest::blocking::Client::new();
    let response = client.get(&url).basic_auth("", Some(password)).send();

    let response_text = response.unwrap().text().unwrap();
    println!("Raw response: {}", response_text);
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
    description: Option<String>,
    description_hash: Option<String>,
    expiry: Option<i64>,
) -> crate::Result<Transaction> {
    let client = reqwest::blocking::Client::new();
    match invoice_type {
        InvoiceType::Bolt11 => {
            let req_url = format!("{}/createinvoice", url);

            let bolt11_req = Bolt11Req {
                description: description.clone(),
                amount_sat: amount,
                expiry_seconds: expiry.unwrap_or(3600),
                external_id: None, // TODO 
                webhook_url: None, // TODO 
            };

            let response: reqwest::blocking::Response = client
                .post(&req_url)
                .basic_auth("", Some(password))
                .form(&bolt11_req)
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
                invoice: bolt11_resp.serialized,
                preimage: "".to_string(),
                payment_hash: bolt11_resp.payment_hash,
                amount: amount,
                fees_paid: 0,
                created_at: 0,
                expires_at: expiry.unwrap_or(3600),
                settled_at: 0,
                description: description.unwrap_or_default(),
                description_hash: description_hash.unwrap_or_default(),
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
                expires_at: expiry.unwrap_or_default(),
                settled_at: 0,
                description: description.unwrap_or_default(),
                description_hash: description_hash.unwrap_or_default(),
            })
        }
    }
}

pub fn lookup_invoice(
    url: String,
    password: String,
    payment_hash: String,
) -> crate::Result<Transaction> {
    let url = format!("{}/payments/incoming/{}", url, payment_hash);
    let client: reqwest::blocking::Client = reqwest::blocking::Client::new();
    let response = client.get(&url).basic_auth("", Some(password)).send();
    let response_text = response.unwrap().text().unwrap();
    let response_text = response_text.as_str();
    let inv: InvoiceResponse = serde_json::from_str(&response_text)?;

    let txn = Transaction {
        type_: "incoming".to_string(),
        invoice: inv.invoice.unwrap_or_default(),
        preimage: inv.preimage,
        payment_hash: inv.payment_hash,
        amount: inv.received_sat * 1000,
        fees_paid: inv.fees * 1000,
        created_at: inv.created_at,
        expires_at: 0, // TODO
        settled_at: 0, // TODO
        description: inv.description.unwrap_or_default(),
        description_hash: "".to_string(), // TODO
    };
    Ok(txn)
}


pub fn list_transactions(
    url: String,
    password: String,
    from: i64,
    until: i64,
    limit: i64,
    offset: i64,
    unpaid: bool,
    invoice_type: String, // not currently used but included for parity
) -> crate::Result<Vec<Transaction>> {
    let client = reqwest::blocking::Client::new();

    // 1) Build query for incoming transactions
    let mut incoming_params = vec![];
    if from != 0 {
        incoming_params.push(("from", (from * 1000).to_string()));
    }
    if until != 0 {
        incoming_params.push(("to", (until * 1000).to_string()));
    }
    if limit != 0 {
        incoming_params.push(("limit", limit.to_string()));
    }
    if offset != 0 {
        incoming_params.push(("offset", offset.to_string()));
    }
    incoming_params.push(("all", unpaid.to_string()));

    // Build the final incoming URL with query
    let incoming_query = serde_urlencoded::to_string(&incoming_params).unwrap();
    let incoming_url = format!("{}/payments/incoming?{}", url, incoming_query);

    // Fetch incoming transactions
    let incoming_resp = client
        .get(&incoming_url)
        .basic_auth("", Some(password.clone()))
        .send();
    let incoming_text = incoming_resp.unwrap().text().unwrap();
    let incoming_text = incoming_text.as_str();
    let incoming_payments: Vec<InvoiceResponse> = serde_json::from_str(&incoming_text).unwrap();

    // Convert incoming payments into "incoming" Transaction
    let mut transactions: Vec<Transaction> = incoming_payments
        .into_iter()
        .map(|inv| {
            // Convert completedAt to an optional settled_at
            let settled_at = if inv.completed_at != 0 {
                Some((inv.completed_at / 1000) as i64)
            } else {
                None
            };
            Transaction {
                type_: "incoming".to_string(),
                invoice: "".to_string(),
                preimage: inv.preimage,
                payment_hash: inv.payment_hash,
                amount: inv.received_sat,
                fees_paid: inv.fees * 1000,
                created_at: (inv.created_at / 1000) as i64,
                expires_at: 0, // TODO 
                settled_at: settled_at.unwrap_or(0),
                description: inv.payer_note.unwrap_or_default(), // TODO description or payer_note?
                description_hash: "".to_string(), // or parse if needed
            }
        })
        .collect();

    // 2) Build query for outgoing transactions
    let mut outgoing_params = vec![];
    if from != 0 {
        outgoing_params.push(("from", (from * 1000).to_string()));
    }
    if until != 0 {
        outgoing_params.push(("to", (until * 1000).to_string()));
    }
    if limit != 0 {
        outgoing_params.push(("limit", limit.to_string()));
    }
    if offset != 0 {
        outgoing_params.push(("offset", offset.to_string()));
    }
    outgoing_params.push(("all", unpaid.to_string()));

    // Build the final outgoing URL with query
    let outgoing_query = serde_urlencoded::to_string(&outgoing_params).unwrap();
    let outgoing_url = format!("{}/payments/outgoing?{}", url, outgoing_query);

    // Fetch outgoing transactions
    let outgoing_resp = client
        .get(&outgoing_url)
        .basic_auth("", Some(password))
        .send();
    let outgoing_text = outgoing_resp.unwrap().text().unwrap();
    let outgoing_text = outgoing_text.as_str();
    let outgoing_payments: Vec<OutgoingPaymentResponse> = serde_json::from_str(&outgoing_text).unwrap();

    // Convert outgoing payments into "outgoing" Transaction
    for payment in outgoing_payments {
        let settled_at = if payment.completed_at != 0 {
            Some((payment.completed_at / 1000) as i64)
        } else {
            None
        };
        transactions.push(Transaction {
            type_: "outgoing".to_string(),
            invoice: "".to_string(), // TODO 
            preimage: payment.preimage,
            payment_hash: payment.payment_hash,
            amount: payment.sent * 1000,
            fees_paid: payment.fees * 1000,
            created_at: (payment.created_at / 1000) as i64,
            expires_at: 0, // TODO
            settled_at: settled_at.unwrap_or(0),
            description: "".to_string(),  // not in OutgoingPaymentResponse data
            description_hash: "".to_string(),
        });
    }

    // Sort by created date descending
    transactions.sort_by(|a, b| b.created_at.cmp(&a.created_at));

    Ok(transactions)
}

