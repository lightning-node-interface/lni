use super::types::{
    Bolt11Resp, FetchInvoiceResponse, GetInfoResponse, ListInvoiceResponse,
    ListInvoiceResponseWrapper,
};
use crate::types::NodeInfo;
use crate::{ApiError, CreateInvoiceParams, InvoiceType, PayCode, PayInvoiceResponse, Transaction};
use reqwest::header;

// Docs
// https://lightning.engineering/api-docs/api/lnd/rest-endpoints/

fn client(macaroon: String) -> reqwest::blocking::Client {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        "Grpc-Metadata-macaroon",
        header::HeaderValue::from_str(&macaroon).unwrap(),
    );

    // TODO Tor proxy
    // let proxy = reqwest::Proxy::all("socks5h://127.0.0.1:9050").unwrap();

    reqwest::blocking::ClientBuilder::new()
        .danger_accept_invalid_certs(true)
        .default_headers(headers)
        //.proxy(proxy)
        .build()
        .unwrap()
}

pub fn get_info(url: String, macaroon: String) -> Result<NodeInfo, ApiError> {
    let req_url = format!("{}/v1/getinfo", url);
    let client = client(macaroon.clone());
    let response = client.get(&req_url).send().unwrap();
    let response_text = response.text().unwrap();
    let response_text = response_text.as_str();
    let info: GetInfoResponse = serde_json::from_str(&response_text)?;

    let node_info = NodeInfo {
        alias: info.alias,
        color: info.color,
        pubkey: info.identity_pubkey,
        network: info.chains[0].network.clone(),
        block_height: info.block_height,
        block_hash: info.block_hash,
    };
    Ok(node_info)
}

pub async fn create_invoice(
    url: String,
    macaroon: String,
    invoice_params: CreateInvoiceParams,
) -> Result<Transaction, ApiError> {
    let client = client(macaroon.clone());
    let amount_msat_str: String = invoice_params
        .amount_msats
        .map_or("any".to_string(), |amt| amt.to_string());
    match invoice_params.invoice_type {
        InvoiceType::Bolt11 => {
            let req_url = format!("{}/v1/invoices", url);
            let response: reqwest::blocking::Response = client
                .post(&req_url)
                .json(&serde_json::json!({
                    "memo": invoice_params.description,
                    "value_msat": amount_msat_str,
                    "expiry": invoice_params.expiry,
                    "is_blinded": invoice_params.is_blinded,
                    "is_keysend": invoice_params.is_keysend,
                    "r_preimage": invoice_params.r_preimage,
                    "is_amp": invoice_params.is_amp,
                    "private": invoice_params.is_private,
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
                invoice: bolt11_resp.payment_request,
                preimage: "".to_string(),
                payment_hash: bolt11_resp.r_hash,
                amount_msats: invoice_params.amount_msats.unwrap_or(0),
                fees_paid: 0,
                created_at: 0,
                expires_at: invoice_params.expiry.unwrap_or(3600),
                settled_at: 0,
                description: invoice_params.description.clone().unwrap_or_default(),
                description_hash: invoice_params.description_hash.unwrap_or_default(),
                payer_note: Some("".to_string()),
                external_id: Some("".to_string()),
            })
        }
        InvoiceType::Bolt12 => {
            return Err(ApiError::Json {
                reason: "Bolt12 not implemented".to_string(),
            });
        }
    }
}

// decode - bolt11 invoice (lnbc) TODO decode: bolt12 invoice (lni) or bolt12 offer (lno)
pub async fn decode(url: String, macaroon: String, str: String) -> Result<String, ApiError> {
    let client = client(macaroon);
    let req_url = format!("{}/v1/payreq/{}", url, str);
    let response: reqwest::blocking::Response = client.get(&req_url).send().unwrap();
    // TODO parse JSON response
    let decoded = response.text().unwrap();
    let decoded = decoded.as_str();
    Ok(decoded.to_string())
}

// get the one with the offer_id or label or get the first offer in the list or
pub async fn get_offer(
    url: String,
    macaroon: String,
    search: Option<String>,
) -> Result<PayCode, ApiError> {
    return Err(ApiError::Json {
        reason: "Bolt12 not implemented".to_string(),
    });
}

pub async fn list_offers(
    url: String,
    macaroon: String,
    search: Option<String>,
) -> Result<Vec<PayCode>, ApiError> {
    return Err(ApiError::Json {
        reason: "Bolt12 not implemented".to_string(),
    });
}

pub async fn create_offer(
    url: String,
    macaroon: String,
    amount_msats: Option<i64>,
    description: Option<String>,
    expiry: Option<i64>,
) -> Result<Transaction, ApiError> {
    return Err(ApiError::Json {
        reason: "Bolt12 not implemented".to_string(),
    });
}

pub async fn fetch_invoice_from_offer(
    url: String,
    macaroon: String,
    offer: String,
    amount_msats: i64, // TODO make optional if the lno already has amount in it
    payer_note: Option<String>,
) -> Result<FetchInvoiceResponse, ApiError> {
    return Err(ApiError::Json {
        reason: "Bolt12 not implemented".to_string(),
    });
}

pub async fn pay_offer(
    url: String,
    macaroon: String,
    offer: String,
    amount_msats: i64,
    payer_note: Option<String>,
) -> Result<PayInvoiceResponse, ApiError> {
    return Err(ApiError::Json {
        reason: "Bolt12 not implemented".to_string(),
    });
}

pub fn lookup_invoice(
    url: String,
    macaroon: String,
    payment_hash: Option<String>,
) -> Result<Transaction, ApiError> {
    let payment_hash_str = payment_hash.unwrap_or_default();
    let list_invoices_url = format!("{}/v1/invoice/{}", url, payment_hash_str);
    println!("list_invoices_url {}", &list_invoices_url);
    let client = client(macaroon);
    // Fetch incoming transactions
    let response: reqwest::blocking::Response = client.get(&list_invoices_url).send().unwrap();
    let status = response.status();
    if status == reqwest::StatusCode::NOT_FOUND {
        return Err(ApiError::Json {
            reason: "Invoice not found".to_string(),
        });
    }
    println!("Status: {}", status);
    let response_text = response.text().unwrap();
    let response_text = response_text.as_str();
    let inv: ListInvoiceResponse = serde_json::from_str(&response_text).unwrap();
    Ok(Transaction {
        type_: "incoming".to_string(),
        invoice: inv.payment_request.unwrap_or_default(),
        preimage: hex::encode(base64::decode(inv.r_preimage.unwrap_or_default()).unwrap()),
        payment_hash: hex::encode(base64::decode(inv.r_hash.unwrap_or_default()).unwrap()),
        amount_msats: inv
            .amt_paid_msat
            .unwrap_or_default()
            .parse::<i64>()
            .unwrap_or_default(),
        fees_paid: inv
            .value_msat
            .unwrap_or_default()
            .parse::<i64>()
            .unwrap_or_default(),
        created_at: inv
            .creation_date
            .unwrap_or_default()
            .parse::<i64>()
            .unwrap_or_default(),
        expires_at: inv
            .expiry
            .unwrap_or_default()
            .parse::<i64>()
            .unwrap_or_default(),
        settled_at: inv
            .settle_date
            .unwrap_or_default()
            .parse::<i64>()
            .unwrap_or_default(),
        description: inv.memo.unwrap_or_default(),
        description_hash: inv.description_hash.unwrap_or_default(), // TODO: what format should hash be in? hex or base64? does anyone care?
        payer_note: Some("".to_string()),
        external_id: Some("".to_string()),
    })
}

pub fn list_transactions(
    url: String,
    macaroon: String,
    from: i64,
    limit: i64,
) -> Result<Vec<Transaction>, ApiError> {
    let list_txns_url = format!(
        "{}/v1/invoices?index_offest={}&num_max_invoices={}",
        url, from, limit
    );
    let client = client(macaroon);

    // Fetch incoming transactions
    let response: reqwest::blocking::Response = client.get(&list_txns_url).send().unwrap();
    let response_text = response.text().unwrap();
    let response_text = response_text.as_str();
    let txns: ListInvoiceResponseWrapper = serde_json::from_str(&response_text).unwrap();

    // Convert incoming payments into "incoming" Transaction
    let mut transactions: Vec<Transaction> = txns
        .invoices
        .into_iter()
        .map(|inv| Transaction {
            type_: "incoming".to_string(),
            invoice: inv.payment_request.unwrap_or_default(),
            preimage: hex::encode(base64::decode(inv.r_preimage.unwrap_or_default()).unwrap()),
            payment_hash: hex::encode(base64::decode(inv.r_hash.unwrap_or_default()).unwrap()),
            amount_msats: inv
                .amt_paid_msat
                .unwrap_or_default()
                .parse::<i64>()
                .unwrap_or_default(),
            fees_paid: inv
                .value_msat
                .unwrap_or_default()
                .parse::<i64>()
                .unwrap_or_default(),
            created_at: inv
                .creation_date
                .unwrap_or_default()
                .parse::<i64>()
                .unwrap_or_default(),
            expires_at: inv
                .expiry
                .unwrap_or_default()
                .parse::<i64>()
                .unwrap_or_default(),
            settled_at: inv
                .settle_date
                .unwrap_or_default()
                .parse::<i64>()
                .unwrap_or_default(),
            description: inv.memo.unwrap_or_default(),
            description_hash: inv.description_hash.unwrap_or_default(),
            payer_note: Some("".to_string()),
            external_id: Some("".to_string()),
        })
        .collect();

    // Sort by created date descending
    transactions.sort_by(|a, b| b.created_at.cmp(&a.created_at));

    Ok(transactions)
}
