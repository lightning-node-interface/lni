use std::time::Duration;

use super::types::{
    BalancesResponse, Bolt11Resp, FetchInvoiceResponse, GetInfoResponse, LndPayInvoiceResponseWrapper,
    ListInvoiceResponse, ListInvoiceResponseWrapper,
};
use super::LndConfig;
use crate::types::NodeInfo;
use crate::{
    ApiError, CreateInvoiceParams, OnInvoiceEventCallback,
    OnInvoiceEventParams, PayCode, PayInvoiceParams, PayInvoiceResponse, Transaction,
    DEFAULT_INVOICE_EXPIRY,
};
use reqwest::header;
use serde_json::json;

// Docs
// https://lightning.engineering/api-docs/api/lnd/rest-endpoints/

fn async_client(config: &LndConfig) -> reqwest::Client {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        "Grpc-Metadata-macaroon",
        header::HeaderValue::from_str(&config.macaroon).unwrap(),
    );

    // Create HTTP client with optional SOCKS5 proxy following say_after_with_tokio pattern
    if let Some(proxy_url) = config.socks5_proxy.clone() {
        if !proxy_url.is_empty() {
            // Accept invalid certificates when using SOCKS5 proxy
            let client_builder = reqwest::Client::builder()
                .default_headers(headers.clone())
                .danger_accept_invalid_certs(true);
            
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
    let mut client_builder = reqwest::Client::builder().default_headers(headers);
    if config.accept_invalid_certs.unwrap_or(false) {
        client_builder = client_builder.danger_accept_invalid_certs(true);
    }
    if let Some(timeout) = config.http_timeout {
        client_builder = client_builder.timeout(std::time::Duration::from_secs(timeout as u64));
    }
    client_builder.build().unwrap_or_else(|_| reqwest::Client::new())
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

// Async version following the same pattern as say_after_with_tokio
#[uniffi::export(async_runtime = "tokio")]
pub async fn get_info(config: LndConfig) -> Result<NodeInfo, ApiError> {
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

// Async version of lookup_invoice following the same pattern as get_info_async
#[uniffi::export(async_runtime = "tokio")]
pub async fn lookup_invoice(
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
        preimage: parse_r_preimage(&inv.r_preimage.unwrap_or_default()),
        payment_hash: parse_r_hash(&inv.r_hash.unwrap_or_default()),
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

// Async version of polling logic
pub async fn poll_invoice_events<F>(
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

        let lookup_result = lookup_invoice(
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
pub async fn on_invoice_events(
    config: LndConfig,
    params: OnInvoiceEventParams,
    callback: Box<dyn OnInvoiceEventCallback>,
) {
    poll_invoice_events(&config, params, move |status, tx| match status.as_str() {
        "success" => callback.success(tx),
        "pending" => callback.pending(tx),
        "failure" => callback.failure(tx),
        _ => {}
    })
    .await;
}

// Async version of create_invoice
#[uniffi::export(async_runtime = "tokio")]
pub async fn create_invoice(
    config: LndConfig,
    params: CreateInvoiceParams,
) -> Result<Transaction, ApiError> {
    let client = async_client(&config);
    
    let mut body = json!({
        "value_msat": params.amount_msats.unwrap_or(0),
        "memo": params.description.clone().unwrap_or_default(),
        "expiry": params.expiry.unwrap_or(DEFAULT_INVOICE_EXPIRY),
        "private": params.is_private.unwrap_or(false),
    });

    if let Some(preimage) = params.r_preimage.clone() {
        body["r_preimage"] = json!(preimage);
    }

    if params.is_blinded.unwrap_or(false) {
        body["is_blinded"] = json!(true);
    }

    let req_url = format!("{}/v1/invoices", config.url);
    let response = client
        .post(&req_url)
        .header("Grpc-Metadata-macaroon", &config.macaroon)
        .json(&body)
        .send()
        .await
        .map_err(|e| ApiError::Http {
            reason: format!("Failed to create invoice: {}", e),
        })?;

    let response_text = response.text().await.map_err(|e| ApiError::Http {
        reason: format!("Failed to read create invoice response: {}", e),
    })?;

    let create_response: Bolt11Resp = serde_json::from_str(&response_text)?;

    Ok(Transaction {
        type_: "incoming".to_string(),
        invoice: create_response.payment_request,
        preimage: "".to_string(),
        payment_hash: parse_r_hash(&create_response.r_hash),
        amount_msats: params.amount_msats.unwrap_or(0),
        fees_paid: 0,
        created_at: 0,
        expires_at: params.expiry.unwrap_or(DEFAULT_INVOICE_EXPIRY),
        settled_at: 0,
        description: params.description.clone().unwrap_or_default(),
        description_hash: params.description_hash.clone().unwrap_or_default(),
        payer_note: Some("".to_string()),
        external_id: Some("".to_string()),
    })
}

// Async version of pay_invoice
#[uniffi::export(async_runtime = "tokio")]
pub async fn pay_invoice(
    config: LndConfig,
    params: PayInvoiceParams,
) -> Result<PayInvoiceResponse, ApiError> {
    let client = async_client(&config);
    
    let mut body = json!({
        "payment_request": params.invoice,
        "allow_self_payment": params.allow_self_payment.unwrap_or(false),
        "timeout_seconds": 60, // Default timeout of 60 seconds
    });

    if let Some(fee_limit_percentage) = params.fee_limit_percentage {
        if let Some(amt) = params.amount_msats {
            body["fee_limit"] = json!({
                "fixed_msat": Some(serde_json::Value::String(amt.to_string())),
                "percent": Some(serde_json::Value::Number(serde_json::Number::from_f64(fee_limit_percentage).unwrap()))
            });
        }
    }

    let req_url = format!("{}/v2/router/send", config.url);
    let response = client
        .post(&req_url)
        .header("Grpc-Metadata-macaroon", &config.macaroon)
        .json(&body)
        .send()
        .await
        .map_err(|e| ApiError::Http {
            reason: format!("Failed to pay invoice: {}", e),
        })?;

    let response_text = response.text().await.map_err(|e| ApiError::Http {
        reason: format!("Failed to read pay invoice response: {}", e),
    })?;

    // Try to parse as potential error response first
    if response_text.contains("error") && !response_text.contains("\"result\"") {
        return Err(ApiError::Json {
            reason: format!("Payment failed: {}", response_text),
        });
    }

    // LND sends streaming responses, we need to parse the last line which contains the final result
    let final_response = response_text
        .lines()
        .last()
        .unwrap_or(&response_text);

    // Parse as wrapped LND response
    let wrapped_response: LndPayInvoiceResponseWrapper = serde_json::from_str(final_response)
        .map_err(|e| ApiError::Json {
            reason: format!("Failed to parse LND wrapped response: {}. Raw response: {}", e, final_response),
        })?;
    
    // Check if payment failed
    if wrapped_response.result.status == "FAILED" {
        return Err(ApiError::Json {
            reason: format!("Payment failed: {}", wrapped_response.result.failure_reason),
        });
    }
    
    // Check if payment is still in flight (shouldn't happen with proper timeout, but just in case)
    if wrapped_response.result.status == "IN_FLIGHT" {
        return Err(ApiError::Json {
            reason: "Payment is still in flight - timeout may need to be increased".to_string(),
        });
    }
    
    // Payment should be SUCCEEDED at this point
    if wrapped_response.result.status != "SUCCEEDED" {
        return Err(ApiError::Json {
            reason: format!("Unknown payment status: {}", wrapped_response.result.status),
        });
    }
    
    // Convert to our standard PayInvoiceResponse format
    let pay_response = PayInvoiceResponse {
        payment_hash: wrapped_response.result.payment_hash,
        preimage: wrapped_response.result.payment_preimage,
        fee_msats: wrapped_response.result.fee_msat.parse::<i64>().unwrap_or(0),
    };
    
    Ok(pay_response)
}

// Async version of decode
#[uniffi::export(async_runtime = "tokio")]
pub async fn decode(config: LndConfig, invoice_str: String) -> Result<String, ApiError> {
    let client = async_client(&config);
    
    let req_url = format!("{}/v1/payreq/{}", config.url, invoice_str);
    let response = client
        .get(&req_url)
        .header("Grpc-Metadata-macaroon", &config.macaroon)
        .send()
        .await
        .map_err(|e| ApiError::Http {
            reason: format!("Failed to decode invoice: {}", e),
        })?;

    let response_text = response.text().await.map_err(|e| ApiError::Http {
        reason: format!("Failed to read decode response: {}", e),
    })?;

    Ok(response_text)
}

// Async version of list_transactions
#[uniffi::export(async_runtime = "tokio")]
pub async fn list_transactions(
    config: LndConfig,
    _from: Option<i64>,
    _limit: Option<i64>,
    _search: Option<String>,
) -> Result<Vec<Transaction>, ApiError> {
    let client = async_client(&config);
    
    let list_txns_url = format!("{}/v1/invoices", config.url);
    let response = client
        .get(&list_txns_url)
        .header("Grpc-Metadata-macaroon", &config.macaroon)
        .send()
        .await
        .map_err(|e| ApiError::Http {
            reason: format!("Failed to list transactions: {}", e),
        })?;

    let response_text = response.text().await.map_err(|e| ApiError::Http {
        reason: format!("Failed to read list transactions response: {}", e),
    })?;

    let txns: ListInvoiceResponseWrapper = serde_json::from_str(&response_text)?;

    let mut transactions: Vec<Transaction> = txns
        .invoices
        .into_iter()
        .map(|inv| Transaction {
            type_: "incoming".to_string(),
            invoice: inv.payment_request.unwrap_or_default(),
            preimage: parse_r_preimage(&inv.r_preimage.unwrap_or_default()),
            payment_hash: parse_r_hash(&inv.r_hash.unwrap_or_default()),
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

    transactions.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(transactions)
}

fn parse_r_hash(r_hash_str: &str) -> String {
    match base64::decode(r_hash_str) {
        Ok(decoded_bytes) => hex::encode(decoded_bytes),
        Err(_) => {
            // If base64 decoding fails, return the original string or empty string
            // This handles cases where r_hash might already be in hex format or is invalid
            r_hash_str.to_string()
        }
    }
}

fn parse_r_preimage(r_preimage_str: &str) -> String {
    match base64::decode(r_preimage_str) {
        Ok(decoded_bytes) => hex::encode(decoded_bytes),
        Err(_) => {
            // If base64 decoding fails, return the original string or empty string
            // This handles cases where r_preimage might already be in hex format or is invalid
            r_preimage_str.to_string()
        }
    }
}