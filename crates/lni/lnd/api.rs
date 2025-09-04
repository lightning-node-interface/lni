use std::thread;
use std::time::Duration;

use super::types::{
    BalancesResponse, Bolt11Resp, FetchInvoiceResponse, GetInfoResponse, ListInvoiceResponse,
    ListInvoiceResponseWrapper, LndPayInvoiceResponseWrapper,
};
use super::LndConfig;
use crate::types::NodeInfo;
use crate::{
    calculate_fee_msats, ApiError, CreateInvoiceParams, InvoiceType, OnInvoiceEventCallback,
    OnInvoiceEventParams, PayCode, PayInvoiceParams, PayInvoiceResponse, Transaction,
};
use reqwest::header;

// Simple test function to check if basic uniffi binding works
#[uniffi::export]
pub fn lnd_test_sync(config: LndConfig) -> Result<String, ApiError> {
    Ok(format!("Sync test successful. URL: {}", config.url))
}

// Docs
// https://lightning.engineering/api-docs/api/lnd/rest-endpoints/

fn client(config: &LndConfig) -> reqwest::blocking::Client {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        "Grpc-Metadata-macaroon",
        header::HeaderValue::from_str(&config.macaroon).unwrap(),
    );
    let mut client = reqwest::blocking::ClientBuilder::new().default_headers(headers);
    let socks5 = config.socks5_proxy.clone().unwrap_or_default();
    if socks5 != "".to_string() {
        let proxy = reqwest::Proxy::all(&socks5).unwrap();
        client = client.proxy(proxy);
    }
    if config.accept_invalid_certs.is_some() {
        client = client.danger_accept_invalid_certs(true);
    }
    if config.http_timeout.is_some() {
        client = client.timeout(std::time::Duration::from_secs(
            config.http_timeout.unwrap_or_default() as u64,
        ));
    }
    client.build().unwrap()
}

fn async_client(config: &LndConfig) -> reqwest::Client {
    // Create HTTP client with optional SOCKS5 proxy following say_after_with_tokio pattern
    if let Some(proxy_url) = config.socks5_proxy.clone() {
        if !proxy_url.is_empty() {
            // Accept invalid certificates when using SOCKS5 proxy
            let client_builder = reqwest::Client::builder().danger_accept_invalid_certs(true);
            
            match reqwest::Proxy::all(&proxy_url) {
                Ok(proxy) => {
                    match client_builder.proxy(proxy).build() {
                        Ok(client) => return client,
                        Err(_) => {} // Fall through to default client creation
                    }
                }
                Err(_) => {} // Fall through to default client creation
            }
        }
    }
    
    // Default client creation
    let mut client_builder = reqwest::Client::builder();
    if config.accept_invalid_certs.unwrap_or(false) {
        client_builder = client_builder.danger_accept_invalid_certs(true);
    }
    if let Some(timeout) = config.http_timeout {
        client_builder = client_builder.timeout(std::time::Duration::from_secs(timeout as u64));
    }
    client_builder.build().unwrap_or_else(|_| reqwest::Client::new())
}

pub fn get_info(config: &LndConfig) -> Result<NodeInfo, ApiError> {
    let req_url = format!("{}/v1/getinfo", config.url);
    let client = client(config);
    let response = client.get(&req_url).send().unwrap();
    let response_text = response.text().unwrap();
    let response_text = response_text.as_str();
    let info: GetInfoResponse = serde_json::from_str(&response_text)?;

    // get balance
    // /v1/balance/channels
    // https://lightning.engineering/api-docs/api/lnd/lightning/channel-balance/
    // send_balance_msats, receive_balance_msats, pending_balance, inactive_balance
    let balance_url = format!("{}/v1/balance/channels", config.url);
    let balance_response = client.get(&balance_url).send().unwrap();
    let balance_response_text = balance_response.text().unwrap();
    let balance_response_text = balance_response_text.as_str();
    let balance: BalancesResponse = serde_json::from_str(&balance_response_text)?;

    let node_info = NodeInfo {
        alias: info.alias,
        color: info.color,
        pubkey: info.identity_pubkey,
        network: info.chains[0].network.clone(),
        block_height: info.block_height,
        block_hash: info.block_hash,
        send_balance_msat: balance
            .local_balance
            .msat
            .unwrap_or_default()
            .parse::<i64>()
            .unwrap_or_default(),
        receive_balance_msat: balance
            .remote_balance
            .msat
            .unwrap_or_default()
            .parse::<i64>()
            .unwrap_or_default(),
        unsettled_send_balance_msat: balance
            .unsettled_local_balance
            .msat
            .unwrap_or_default()
            .parse::<i64>()
            .unwrap_or_default(),
        unsettled_receive_balance_msat: balance
            .unsettled_remote_balance
            .msat
            .unwrap_or_default()
            .parse::<i64>()
            .unwrap_or_default(),
        pending_open_send_balance: balance
            .pending_open_local_balance
            .msat
            .unwrap_or_default()
            .parse::<i64>()
            .unwrap_or_default(),
        pending_open_receive_balance: balance
            .pending_open_remote_balance
            .msat
            .unwrap_or_default()
            .parse::<i64>()
            .unwrap_or_default(),
        ..Default::default()
    };
    Ok(node_info)
}

// Core shared logic for processing LND node info and balance responses
fn process_node_info_responses(
    info: GetInfoResponse,
    balance: BalancesResponse,
) -> NodeInfo {
    NodeInfo {
        alias: info.alias,
        color: info.color,
        pubkey: info.identity_pubkey,
        network: info.chains[0].network.clone(),
        block_height: info.block_height,
        block_hash: info.block_hash,
        send_balance_msat: balance
            .local_balance
            .msat
            .unwrap_or_default()
            .parse::<i64>()
            .unwrap_or_default(),
        receive_balance_msat: balance
            .remote_balance
            .msat
            .unwrap_or_default()
            .parse::<i64>()
            .unwrap_or_default(),
        unsettled_send_balance_msat: balance
            .unsettled_local_balance
            .msat
            .unwrap_or_default()
            .parse::<i64>()
            .unwrap_or_default(),
        unsettled_receive_balance_msat: balance
            .unsettled_remote_balance
            .msat
            .unwrap_or_default()
            .parse::<i64>()
            .unwrap_or_default(),
        pending_open_send_balance: balance
            .pending_open_local_balance
            .msat
            .unwrap_or_default()
            .parse::<i64>()
            .unwrap_or_default(),
        pending_open_receive_balance: balance
            .pending_open_remote_balance
            .msat
            .unwrap_or_default()
            .parse::<i64>()
            .unwrap_or_default(),
        ..Default::default()
    }
}

// Synchronous version that works reliably with uniffi-bindgen-react-native
#[cfg_attr(feature = "uniffi", uniffi::export)]
pub fn lnd_get_info_sync(config: LndConfig) -> Result<NodeInfo, ApiError> {
    // Try using a different approach - spawn a separate thread for the HTTP call
    std::thread::spawn(move || {
        // Add artificial delay to test UI blocking (remove in production)
        std::thread::sleep(std::time::Duration::from_secs(15));

        let req_url = format!("{}/v1/getinfo", config.url);
        let client = client(&config);

        let response = client.get(&req_url).send().unwrap();
        let response_text = response.text().unwrap();
        let response_text = response_text.as_str();
        let info: GetInfoResponse = serde_json::from_str(&response_text)?;

        // get balance
        let balance_url = format!("{}/v1/balance/channels", config.url);
        let balance_response = client.get(&balance_url).send().unwrap();
        let balance_response_text = balance_response.text().unwrap();
        let balance_response_text = balance_response_text.as_str();
        let balance: BalancesResponse = serde_json::from_str(&balance_response_text)?;

        // Use shared logic to create NodeInfo
        let node_info = process_node_info_responses(info, balance);
        Ok(node_info)
    }).join().unwrap()
}

// Async version following the same pattern as say_after_with_tokio
#[uniffi::export(async_runtime = "tokio")]
pub async fn lnd_get_info_async(config: LndConfig) -> Result<NodeInfo, ApiError> {
    // Create HTTP client using the helper function
    let client = async_client(&config);
    
    // Get node info
    let req_url = format!("{}/v1/getinfo", config.url);
    let mut info_request = client.get(&req_url);
    info_request = info_request.header("Grpc-Metadata-macaroon", &config.macaroon);
    
    let info_response = info_request.send().await.map_err(|e| ApiError::Http {
        reason: format!("Failed to get node info: {}", e)
    })?;
    
    let info_text = info_response.text().await.map_err(|e| ApiError::Http {
        reason: format!("Failed to read node info response: {}", e)
    })?;
    
    let info: GetInfoResponse = serde_json::from_str(&info_text)?;
    
    // Get balance info
    let balance_url = format!("{}/v1/balance/channels", config.url);
    let mut balance_request = client.get(&balance_url);
    balance_request = balance_request.header("Grpc-Metadata-macaroon", &config.macaroon);
    
    let balance_response = balance_request.send().await.map_err(|e| ApiError::Http {
        reason: format!("Failed to get balance info: {}", e)
    })?;
    
    let balance_text = balance_response.text().await.map_err(|e| ApiError::Http {
        reason: format!("Failed to read balance response: {}", e)
    })?;
    
    let balance: BalancesResponse = serde_json::from_str(&balance_text)?;

    // Use shared logic to create NodeInfo
    let node_info = process_node_info_responses(info, balance);
    Ok(node_info)
}

pub fn create_invoice(
    config: &LndConfig,
    invoice_params: CreateInvoiceParams,
) -> Result<Transaction, ApiError> {
    let client = client(config);
    let amount_msat_str: String = invoice_params
        .amount_msats
        .map_or("any".to_string(), |amt| amt.to_string());
    match invoice_params.invoice_type {
        InvoiceType::Bolt11 => {
            let req_url = format!("{}/v1/invoices", config.url);
            let response = client
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

pub fn pay_invoice(
    config: &LndConfig,
    invoice_params: PayInvoiceParams,
) -> Result<PayInvoiceResponse, ApiError> {
    let client = client(config);
    let mut params: Vec<(&str, Option<serde_json::Value>)> = vec![];
    params.push((
        "payment_request",
        Some(serde_json::Value::String(
            (invoice_params.invoice.to_string()),
        )),
    ));
    invoice_params.amount_msats.map(|amt| {
        params.push((
            "amt_msat",
            Some(serde_json::Value::String((amt.to_string()))),
        ))
    });
    invoice_params.allow_self_payment.map(|allow| {
        params.push(("allow_self_payment", Some(serde_json::Value::Bool(allow))));
    });

    // calculate fee limit
    if invoice_params.fee_limit_msat.is_some() && invoice_params.fee_limit_percentage.is_some() {
        return Err(ApiError::Json {
            reason: "Cannot set both fee_limit_msat and fee_limit_percentage".to_string(),
        });
    }
    invoice_params.fee_limit_msat.map(|amt| {
        params.push((
            "fee_limit_msat",
            Some(serde_json::Value::String(amt.to_string())),
        ));
    });
    invoice_params.fee_limit_percentage.map(|fee_percentage| {
        let fee_msats = calculate_fee_msats(
            invoice_params.invoice.as_str(),
            fee_percentage,
            invoice_params.amount_msats.map(|v| v as u64),
        )
        .unwrap();
        params.push((
            "fee_limit_msat",
            Some(serde_json::Value::String(fee_msats.to_string())),
        ));
    });

    invoice_params.first_hop_pubkey.map(|pubkey| {
        params.push((
            "first_hop_pubkey",
            Some(serde_json::Value::String(pubkey.to_string())),
        ))
    });
    invoice_params
        .is_amp
        .map(|is_amp| params.push(("is_amp", Some(serde_json::Value::Bool(is_amp)))));
    invoice_params.last_hop_pubkey.map(|pubkey| {
        params.push((
            "last_hop_pubkey",
            Some(serde_json::Value::String(pubkey.to_string())),
        ))
    });
    invoice_params.max_parts.map(|parts| {
        params.push((
            "max_parts",
            Some(serde_json::Value::String(parts.to_string())),
        ))
    });
    invoice_params.timeout_seconds.map(|timeout| {
        params.push((
            "timeout_seconds",
            Some(serde_json::Value::String(timeout.to_string())),
        ))
    });

    let params_json: serde_json::Value = params
        .into_iter()
        .filter_map(|(k, v)| v.map(|v| (k.to_string(), v)))
        .collect::<serde_json::Map<String, _>>()
        .into();

    println!("PayInvoice params: {:?}", &params_json);

    let req_url = format!("{}/v2/router/send", config.url);
    let response = client.post(&req_url).json(&params_json).send().unwrap();

    println!("Status: {}", response.status());
    let invoice_str = response.text().unwrap();

    // * LND returns a stream of JSON objects, one per line, so we need to parse each line and grab the JSON string and then parse
    let invoice_lines: Vec<&str> = invoice_str.split('\n').collect();
    let pay_invoice_resp: LndPayInvoiceResponseWrapper = invoice_lines
        .iter()
        .filter_map(|line| {
            let resp: Result<LndPayInvoiceResponseWrapper, _> = serde_json::from_str(line);
            match resp {
                Ok(r) if r.result.status == "SUCCEEDED" => Some(r),
                _ => None,
            }
        })
        .next()
        .ok_or_else(|| crate::ApiError::Json {
            reason: "No successful payment found".to_string(),
        })?;

    println!("PayInvoice response final: {:?}", &pay_invoice_resp);

    Ok(PayInvoiceResponse {
        payment_hash: pay_invoice_resp.result.payment_hash,
        preimage: pay_invoice_resp.result.payment_preimage,
        fee_msats: pay_invoice_resp
            .result
            .fee_msat
            .parse::<i64>()
            .unwrap_or_default(),
    })
}

// decode - bolt11 invoice (lnbc) TODO decode: bolt12 invoice (lni) or bolt12 offer (lno)
pub fn decode(config: &LndConfig, str: String) -> Result<String, ApiError> {
    let client = client(config);
    let req_url = format!("{}/v1/payreq/{}", config.url, str);
    let response = client.get(&req_url).send().unwrap();
    // TODO parse JSON response
    let decoded = response.text().unwrap();
    let decoded = decoded.as_str();
    Ok(decoded.to_string())
}

// get the one with the offer_id or label or get the first offer in the list or
pub fn get_offer(config: &LndConfig, search: Option<String>) -> Result<PayCode, ApiError> {
    return Err(ApiError::Json {
        reason: "Bolt12 not implemented".to_string(),
    });
}

pub fn list_offers(config: &LndConfig, search: Option<String>) -> Result<Vec<PayCode>, ApiError> {
    return Err(ApiError::Json {
        reason: "Bolt12 not implemented".to_string(),
    });
}

pub fn create_offer(
    config: &LndConfig,
    amount_msats: Option<i64>,
    description: Option<String>,
    expiry: Option<i64>,
) -> Result<Transaction, ApiError> {
    return Err(ApiError::Json {
        reason: "Bolt12 not implemented".to_string(),
    });
}

pub fn fetch_invoice_from_offer(
    config: &LndConfig,
    offer: String,
    amount_msats: i64, // TODO make optional if the lno already has amount in it
    payer_note: Option<String>,
) -> Result<FetchInvoiceResponse, ApiError> {
    return Err(ApiError::Json {
        reason: "Bolt12 not implemented".to_string(),
    });
}

pub fn pay_offer(
    config: &LndConfig,
    offer: String,
    amount_msats: i64,
    payer_note: Option<String>,
) -> Result<PayInvoiceResponse, ApiError> {
    return Err(ApiError::Json {
        reason: "Bolt12 not implemented".to_string(),
    });
}

pub fn lookup_invoice(
    config: &LndConfig,
    payment_hash: Option<String>,
    from: Option<i64>,
    limit: Option<i64>,
    search: Option<String>,
) -> Result<Transaction, ApiError> {
    let payment_hash_str = payment_hash.unwrap_or_default();
    let list_invoices_url = format!("{}/v1/invoice/{}", config.url, payment_hash_str);
    println!("list_invoices_url {}", &list_invoices_url);
    let client = client(config);
    // Fetch incoming transactions
    let response = client.get(&list_invoices_url).send().unwrap();
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

// Async version of lookup_invoice following the same pattern as lnd_get_info_async
#[uniffi::export(async_runtime = "tokio")]
pub async fn lnd_lookup_invoice_async(
    config: LndConfig,
    payment_hash: Option<String>,
    _from: Option<i64>,
    _limit: Option<i64>,
    _search: Option<String>,
) -> Result<Transaction, ApiError> {
    let payment_hash_str = payment_hash.unwrap_or_default();
    let list_invoices_url = format!("{}/v1/invoice/{}", config.url, payment_hash_str);
    println!("list_invoices_url {}", &list_invoices_url);
    
    // Create HTTP client using the helper function
    let client = async_client(&config);
    
    // Fetch incoming transactions
    let mut request = client.get(&list_invoices_url);
    request = request.header("Grpc-Metadata-macaroon", &config.macaroon);
    
    let response = request.send().await.map_err(|e| ApiError::Http {
        reason: format!("Failed to lookup invoice: {}", e)
    })?;
    
    let status = response.status();
    if status == reqwest::StatusCode::NOT_FOUND {
        return Err(ApiError::Json {
            reason: "Invoice not found".to_string(),
        });
    }
    
    println!("Status: {}", status);
    let response_text = response.text().await.map_err(|e| ApiError::Http {
        reason: format!("Failed to read invoice response: {}", e)
    })?;
    
    let inv: ListInvoiceResponse = serde_json::from_str(&response_text)?;
    
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
    config: &LndConfig,
    from: i64,
    limit: i64,
    search: Option<String>,
) -> Result<Vec<Transaction>, ApiError> {
    let list_txns_url = format!(
        "{}/v1/invoices?index_offest={}&num_max_invoices={}",
        config.url, from, limit
    );
    let client = client(config);

    // Fetch incoming transactions
    let response = client.get(&list_txns_url).send().unwrap();
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

// Core shared logic for invoice polling - processes lookup result and determines status
fn process_invoice_lookup_result(transaction_result: Result<Transaction, ApiError>) -> (String, Option<Transaction>) {
    match transaction_result {
        Ok(transaction) => {
            if transaction.settled_at > 0 {
                ("settled".to_string(), Some(transaction))
            } else {
                ("pending".to_string(), Some(transaction))
            }
        }
        Err(_) => ("error".to_string(), None),
    }
}

// Core shared logic for handling poll status - determines if we should continue polling
fn handle_poll_status<F>(status: &str, transaction: Option<Transaction>, mut callback: F) -> bool
where
    F: FnMut(String, Option<Transaction>),
{
    match status {
        "settled" => {
            callback("success".to_string(), transaction);
            false // Stop polling
        }
        "error" => {
            callback("failure".to_string(), transaction);
            true // Continue polling on error
        }
        _ => {
            callback("pending".to_string(), transaction);
            true // Continue polling
        }
    }
}

// Sync version of polling logic
pub fn poll_invoice_events<F>(config: &LndConfig, params: OnInvoiceEventParams, mut callback: F)
where
    F: FnMut(String, Option<Transaction>),
{
    let start_time = std::time::Instant::now();
    loop {
        if start_time.elapsed() > Duration::from_secs(params.max_polling_sec as u64) {
            // timeout
            callback("failure".to_string(), None);
            break;
        }

        let lookup_result = lookup_invoice(
            config,
            params.payment_hash.clone(),
            None,
            None,
            params.search.clone(),
        );
        
        let (status, transaction) = process_invoice_lookup_result(lookup_result);
        let should_continue = handle_poll_status(&status, transaction, &mut callback);
        
        if !should_continue {
            break;
        }

        thread::sleep(Duration::from_secs(params.polling_delay_sec as u64));
    }
}

pub fn on_invoice_events(
    config: LndConfig,
    params: OnInvoiceEventParams,
    callback: Box<dyn OnInvoiceEventCallback>,
) {
    poll_invoice_events(&config, params, move |status, tx| match status.as_str() {
        "success" => callback.success(tx),
        "pending" => callback.pending(tx),
        "failure" => callback.failure(tx),
        _ => {}
    });
}

// Async version of polling logic
pub async fn poll_invoice_events_async<F>(
    config: &LndConfig,
    params: OnInvoiceEventParams,
    mut callback: F,
) where
    F: FnMut(String, Option<Transaction>),
{
    let start_time = std::time::Instant::now();
    loop {
        if start_time.elapsed() > Duration::from_secs(params.max_polling_sec as u64) {
            // timeout
            callback("failure".to_string(), None);
            break;
        }

        let lookup_result = lnd_lookup_invoice_async(
            config.clone(),
            params.payment_hash.clone(),
            None,
            None,
            params.search.clone(),
        )
        .await;
        
        let (status, transaction) = process_invoice_lookup_result(lookup_result);
        let should_continue = handle_poll_status(&status, transaction, &mut callback);
        
        if !should_continue {
            break;
        }

        tokio::time::sleep(Duration::from_secs(params.polling_delay_sec as u64)).await;
    }
}

#[uniffi::export(async_runtime = "tokio")]
pub async fn lnd_on_invoice_events_async(
    config: LndConfig,
    params: OnInvoiceEventParams,
    callback: Box<dyn OnInvoiceEventCallback>,
) {
    poll_invoice_events_async(&config, params, move |status, tx| match status.as_str() {
        "success" => callback.success(tx),
        "pending" => callback.pending(tx),
        "failure" => callback.failure(tx),
        _ => {}
    })
    .await;
}
