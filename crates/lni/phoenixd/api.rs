use super::types::{
    Bolt11Req, Bolt11Resp, InfoResponse, InvoiceResponse, OutgoingPaymentResponse, PayResponse,
    PhoenixPayInvoiceResp,
};
use crate::{
    phoenixd::types::GetBalanceResponse, ApiError, InvoiceType, NodeInfo, PayCode, PayInvoiceParams, PayInvoiceResponse, Transaction
};
use serde_urlencoded;

// TODO
// list_channels
// get_balance

// https://phoenix.acinq.co/server/api

pub fn get_info(url: String, password: String) -> Result<NodeInfo, ApiError> {
    let info_url = format!("{}/getinfo", url);
    let mut builder = reqwest::blocking::ClientBuilder::new();
    let proxy = reqwest::Proxy::all("socks5h://127.0.0.1:9050").unwrap();
    builder = builder.proxy(proxy);
    builder = builder.timeout(std::time::Duration::from_secs(120));
    builder = builder.danger_accept_invalid_certs(true);
    let client = builder.build().unwrap();

    let response: Result<reqwest::blocking::Response, reqwest::Error> = client.get(&info_url).basic_auth("", Some(password.clone())).send();
    let response_text = response.unwrap().text().unwrap();
    println!("get node info response: {}", response_text);
    let info: InfoResponse = serde_json::from_str(&response_text)?;

    // /getbalance
    let balance_url = format!("{}/getbalance", url);
    let balance_response: Result<reqwest::blocking::Response, reqwest::Error> = client.get(&balance_url).basic_auth("", Some(password)).send();
    let balance_response_text = balance_response.unwrap().text().unwrap();
    println!("balance_response: {}", balance_response_text);
    let balance: GetBalanceResponse = serde_json::from_str(&balance_response_text)?;

    let node_info = NodeInfo {
        alias: "Phoenixd".to_string(),
        color: "".to_string(),
        pubkey: info.node_id,
        network: "bitcoin".to_string(),
        block_height: 0,
        block_hash: "".to_string(),
        send_balance_msat: info.channels[0].balance_sat * 1000,
        receive_balance_msat: info.channels[0].inbound_liquidity_sat * 1000,
        fee_credit_balance_msat: balance.fee_credit_sat * 1000,
        ..Default::default()
    };
    Ok(node_info)
}

pub async fn create_invoice(
    url: String,
    password: String,
    invoice_type: InvoiceType,
    amount_msats: Option<i64>,
    description: Option<String>,
    description_hash: Option<String>,
    expiry: Option<i64>,
) -> Result<Transaction, ApiError> {
    let client = reqwest::blocking::Client::new();
    match invoice_type {
        InvoiceType::Bolt11 => {
            let req_url = format!("{}/createinvoice", url);

            let bolt11_req = Bolt11Req {
                description: description.clone(),
                amount_sat: amount_msats.unwrap_or_default() / 1000,
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
                amount_msats: amount_msats.unwrap_or(0),
                fees_paid: 0,
                created_at: 0,
                expires_at: expiry.unwrap_or(3600),
                settled_at: 0,
                description: description.unwrap_or_default(),
                description_hash: description_hash.unwrap_or_default(),
                payer_note: Some("".to_string()),
                external_id: Some("".to_string()),
            })
        }
        InvoiceType::Bolt12 => {
            return Err(ApiError::Json {
                reason: "phoenixd does not support bolt12 invoices".to_string(),
            });
        }
    }
}

pub async fn pay_invoice(
    url: String,
    password: String,
    invoice_params: PayInvoiceParams,
) -> Result<PayInvoiceResponse, ApiError> {
    let client = reqwest::blocking::Client::new();
    let req_url = format!("{}/payinvoice", url);
    let mut params = vec![];
    if invoice_params.amount_msats.is_some() {
        params.push((
            "amountSat",
            Some((invoice_params.amount_msats.unwrap_or_default() / 1000).to_string()),
        ));
    }
    params.push(("invoice", Some(invoice_params.invoice.to_string())));
    let response: reqwest::blocking::Response = client
        .post(&req_url)
        .basic_auth("", Some(password))
        .form(&params)
        .send()
        .unwrap();
    println!("Status: {}", response.status());
    let response_text = response.text().unwrap();
    let pay_invoice_resp: PhoenixPayInvoiceResp =
        serde_json::from_str(&response_text).map_err(|e| ApiError::Json {
            reason: format!("Failed to parse pay_invoice response: {}", e),
        })?;

    Ok(PayInvoiceResponse {
        payment_hash: pay_invoice_resp.payment_hash,
        preimage: pay_invoice_resp.preimage,
        fee_msats: pay_invoice_resp.routing_fee_sat * 1000,
    })
}

// TODO decode - bolt11 invoice (lnbc) bolt12 invoice (lni) or bolt12 offer (lno)
// Not supported by Phoenixd api so maybe we can use ldk to decode the bolt12 offer?
pub async fn decode(str: String) -> Result<String, ApiError> {
    Ok(str)
}

// TODO On Phoenixd there is not currenly a way to create a new BOLT 12 offer

// Get latest BOLT12 offer
pub async fn get_offer(url: String, password: String) -> Result<PayCode, ApiError> {
    let req_url = format!("{}/getoffer", url);
    let client = reqwest::blocking::Client::new();
    let response: reqwest::blocking::Response = client
        .get(&req_url)
        .basic_auth("", Some(password))
        .send()
        .unwrap();
    let offer_str = response.text().unwrap();
    Ok(PayCode {
        offer_id: "".to_string(),
        bolt12: offer_str.to_string(),
        label: None,
        active: None,
        single_use: None,
        used: None,
    })
}

pub async fn pay_offer(
    url: String,
    password: String,
    offer: String,
    amount_msats: i64,
    payer_note: Option<String>,
) -> Result<PayInvoiceResponse, ApiError> {
    let client = reqwest::blocking::Client::new();
    let req_url = format!("{}/payoffer", url);
    let response: reqwest::blocking::Response = client
        .post(&req_url)
        .basic_auth("", Some(password))
        .form(&[
            ("amountSat", (amount_msats / 1000).to_string()),
            ("offer", offer),
            ("message", payer_note.unwrap_or_default()),
        ])
        .send()
        .unwrap();
    let response_text = response.text().unwrap();
    let response_text = response_text.as_str();
    let pay_resp: PayResponse = match serde_json::from_str(&response_text) {
        Ok(resp) => resp,
        Err(e) => {
            return Err(ApiError::Json {
                reason: response_text.to_string(),
            })
        }
    };
    Ok(PayInvoiceResponse {
        payment_hash: pay_resp.payment_hash,
        preimage: pay_resp.preimage,
        fee_msats: pay_resp.routing_fee_sat * 1000,
    })
}

// TODO implement list_offers, currently just one is returned by Phoenixd
pub async fn list_offers() {}

pub fn lookup_invoice(
    url: String,
    password: String,
    payment_hash: String,
) -> Result<Transaction, ApiError> {
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
        amount_msats: inv.received_sat * 1000,
        fees_paid: inv.fees * 1000,
        created_at: inv.created_at,
        expires_at: 0, // TODO
        settled_at: 0, // TODO
        description: inv.description.unwrap_or_default(),
        description_hash: "".to_string(), // TODO
        payer_note: Some(inv.payer_note.unwrap_or("".to_string())),
        external_id: Some(inv.external_id.unwrap_or("".to_string())),
    };
    Ok(txn)
}

pub fn list_transactions(
    url: String,
    password: String,
    from: i64,
    // until: i64,
    limit: i64,
    payment_hash: Option<String>,
    // offset: i64,
    // unpaid: bool,
    // invoice_type: Option<String>, // not currently used but included for parity
    // search_term: Option<String>,  // not currently used but included for parity
) -> Result<Vec<Transaction>, ApiError> {
    let client = reqwest::blocking::Client::new();

    // 1) Build query for incoming transactions
    let mut incoming_params = vec![];
    if from != 0 {
        incoming_params.push(("from", (from * 1000).to_string()));
    }
    if limit != 0 {
        incoming_params.push(("limit", limit.to_string()));
    }
    // if until != 0 {
    //     incoming_params.push(("to", (until * 1000).to_string()));
    // }
    // if offset != 0 {
    //     incoming_params.push(("offset", offset.to_string()));
    // }
    incoming_params.push(("all", "false".to_string()));

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
                amount_msats: inv.received_sat * 1000,
                fees_paid: inv.fees * 1000,
                created_at: (inv.created_at / 1000) as i64,
                expires_at: 0, // TODO
                settled_at: settled_at.unwrap_or(0),
                description: "".to_string(),
                description_hash: "".to_string(),
                payer_note: Some(inv.payer_note.unwrap_or("".to_string())),
                external_id: Some(inv.external_id.unwrap_or("".to_string())),
            }
        })
        .collect();

    // 2) Build query for outgoing transactions
    let mut outgoing_params = vec![];
    if from != 0 {
        outgoing_params.push(("from", (from * 1000).to_string()));
    }
    if limit != 0 {
        outgoing_params.push(("limit", limit.to_string()));
    }
    // if until != 0 {
    //     outgoing_params.push(("to", (until * 1000).to_string()));
    // }
    // if offset != 0 {
    //     outgoing_params.push(("offset", offset.to_string()));
    // }
    // outgoing_params.push(("all", unpaid.to_string()));

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
    let outgoing_payments: Vec<OutgoingPaymentResponse> =
        serde_json::from_str(&outgoing_text).unwrap();

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
            amount_msats: payment.sent * 1000,
            fees_paid: payment.fees * 1000,
            created_at: (payment.created_at / 1000) as i64,
            expires_at: 0, // TODO
            settled_at: settled_at.unwrap_or(0),
            description: "".to_string(),
            description_hash: "".to_string(),
            payer_note: Some(payment.payer_note.unwrap_or("".to_string())),
            external_id: Some(payment.external_id.unwrap_or("".to_string())),
        });
    }

    // Sort by created date descending
    transactions.sort_by(|a, b| b.created_at.cmp(&a.created_at));

    Ok(transactions)
}
