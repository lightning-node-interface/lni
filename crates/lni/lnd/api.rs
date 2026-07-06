use std::time::Duration;

use super::types::{
    BalancesResponse, Bolt11Resp, FetchInvoiceResponse, GetInfoResponse, LndPayInvoiceResponseWrapper,
    ListInvoiceResponse, ListInvoiceResponseWrapper,
};
use super::LndConfig;
use crate::types::NodeInfo;
use crate::{
    ApiError, CreateInvoiceParams, Offer, OnInvoiceEventCallback,
    OnInvoiceEventParams, OnchainFeePayer, OnchainFeePreference, OnchainFeePreferenceType,
    OnchainFeeSpeed, OnchainTransaction, PayInvoiceParams, PayInvoiceResponse,
    PayOnchainOptions, PayOnchainResponse, PrepareOnchainTransactionParams, Transaction,
    DEFAULT_INVOICE_EXPIRY,
};
use reqwest::header;
use serde::{Deserialize, Serialize};
use serde_json::json;

// Docs
// https://lightning.engineering/api-docs/api/lnd/rest-endpoints/

#[derive(Debug, Deserialize)]
struct EstimateFeeResponse {
    fee_sat: Option<String>,
    sat_per_vbyte: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SendCoinsResponse {
    txid: Option<String>,
}

#[derive(Clone)]
struct LndOnchainFeeRequest {
    target_conf: Option<i64>,
    sat_per_vbyte: Option<i64>,
}

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

fn assert_valid_onchain_amount(amount_sats: i64) -> Result<(), ApiError> {
    if amount_sats <= 0 {
        return Err(ApiError::InvalidInput(
            "pay_onchain requires a positive amount_sats".to_string(),
        ));
    }

    Ok(())
}

fn default_onchain_fee() -> OnchainFeePreference {
    OnchainFeePreference {
        preference_type: OnchainFeePreferenceType::TargetConf,
        speed: None,
        target_conf: Some(6),
        sats_per_vbyte: None,
        backend: None,
    }
}

fn resolve_lnd_fee_payer(fee_payer: Option<OnchainFeePayer>) -> Result<OnchainFeePayer, ApiError> {
    match fee_payer.unwrap_or(OnchainFeePayer::Sender) {
        OnchainFeePayer::Sender => Ok(OnchainFeePayer::Sender),
        OnchainFeePayer::Recipient => Err(ApiError::InvalidInput(
            "LND pay_onchain only supports sender-paid on-chain fees".to_string(),
        )),
    }
}

fn resolve_lnd_fee_request(fee: &OnchainFeePreference) -> Result<LndOnchainFeeRequest, ApiError> {
    match fee.preference_type {
        OnchainFeePreferenceType::Default => Ok(LndOnchainFeeRequest {
            target_conf: Some(6),
            sat_per_vbyte: None,
        }),
        OnchainFeePreferenceType::Speed => match fee.speed.clone().unwrap_or(OnchainFeeSpeed::Normal) {
            OnchainFeeSpeed::Fast => Ok(LndOnchainFeeRequest {
                target_conf: Some(1),
                sat_per_vbyte: None,
            }),
            OnchainFeeSpeed::Normal => Ok(LndOnchainFeeRequest {
                target_conf: Some(6),
                sat_per_vbyte: None,
            }),
            OnchainFeeSpeed::Slow => Ok(LndOnchainFeeRequest {
                target_conf: Some(12),
                sat_per_vbyte: None,
            }),
            OnchainFeeSpeed::Free => Err(ApiError::InvalidInput(
                "LND pay_onchain does not support free on-chain fee speed".to_string(),
            )),
        },
        OnchainFeePreferenceType::TargetConf => {
            let blocks = fee.target_conf.ok_or_else(|| {
                ApiError::InvalidInput(
                    "LND target_conf fee preference requires a block target".to_string(),
                )
            })?;
            if blocks <= 0 {
                return Err(ApiError::InvalidInput(
                    "LND target_conf fee preference requires a positive block target".to_string(),
                ));
            }
            Ok(LndOnchainFeeRequest {
                target_conf: Some(blocks),
                sat_per_vbyte: None,
            })
        }
        OnchainFeePreferenceType::SatsPerVbyte => {
            let sats_per_vbyte = fee.sats_per_vbyte.ok_or_else(|| {
                ApiError::InvalidInput(
                    "LND sats_per_vbyte fee preference requires a fee rate".to_string(),
                )
            })?;
            if !sats_per_vbyte.is_finite() || sats_per_vbyte <= 0.0 {
                return Err(ApiError::InvalidInput(
                    "LND sats_per_vbyte fee preference requires a positive fee rate".to_string(),
                ));
            }
            Ok(LndOnchainFeeRequest {
                target_conf: None,
                sat_per_vbyte: Some(sats_per_vbyte.ceil() as i64),
            })
        }
        OnchainFeePreferenceType::Backend => Err(ApiError::InvalidInput(
            "LND pay_onchain does not support backend fee preferences".to_string(),
        )),
    }
}

fn assert_valid_guardrail_limit(value: f64, name: &str) -> Result<(), ApiError> {
    if !value.is_finite() || value < 0.0 {
        return Err(ApiError::InvalidInput(format!(
            "{} must be a non-negative finite number",
            name
        )));
    }

    Ok(())
}

fn assert_onchain_fee_guardrail(
    transaction: &OnchainTransaction,
    options: PayOnchainOptions,
) -> Result<(), ApiError> {
    if options.dangerously_disable_fee_guardrail {
        return Ok(());
    }

    let default_guardrail = crate::types::OnchainFeeGuardrail::default();
    let guardrail = options.fee_guardrail.unwrap_or_default();
    let max_fee_sats = guardrail
        .max_fee_sats
        .or(default_guardrail.max_fee_sats)
        .unwrap_or(crate::types::DEFAULT_ONCHAIN_MAX_FEE_SATS);
    let max_fee_percent = guardrail
        .max_fee_percent
        .or(default_guardrail.max_fee_percent)
        .unwrap_or(crate::types::DEFAULT_ONCHAIN_MAX_FEE_PERCENT);

    if max_fee_sats < 0 {
        return Err(ApiError::InvalidInput(
            "fee_guardrail.max_fee_sats must be non-negative".to_string(),
        ));
    }
    assert_valid_guardrail_limit(max_fee_percent, "fee_guardrail.max_fee_percent")?;

    let fee_sats = transaction.fee_sats.ok_or_else(|| {
        ApiError::InvalidInput(
            "Cannot pay on-chain transaction because fee_sats is unknown. Re-prepare the transaction or pass dangerously_disable_fee_guardrail: true.".to_string(),
        )
    })?;

    if fee_sats < 0 {
        return Err(ApiError::InvalidInput(
            "Cannot pay on-chain transaction because fee_sats is invalid".to_string(),
        ));
    }

    if transaction.amount_sats <= 0 {
        return Err(ApiError::InvalidInput(
            "Cannot pay on-chain transaction because amount_sats is invalid".to_string(),
        ));
    }

    if fee_sats > max_fee_sats {
        return Err(ApiError::InvalidInput(format!(
            "On-chain fee {} sats exceeds guardrail max_fee_sats {}",
            fee_sats, max_fee_sats
        )));
    }

    let fee_percent = (fee_sats as f64 / transaction.amount_sats as f64) * 100.0;
    if fee_percent > max_fee_percent {
        return Err(ApiError::InvalidInput(format!(
            "On-chain fee {:.2}% exceeds guardrail max_fee_percent {}%",
            fee_percent, max_fee_percent
        )));
    }

    Ok(())
}

fn parse_optional_fee_sats(value: Option<String>) -> Option<i64> {
    value.and_then(|value| value.parse::<i64>().ok()).filter(|fee| *fee >= 0)
}

fn raw_label(transaction: &OnchainTransaction) -> Option<String> {
    let raw = transaction.raw.as_deref()?;
    let value: serde_json::Value = serde_json::from_str(raw).ok()?;
    value
        .get("label")
        .and_then(|label| label.as_str())
        .filter(|label| !label.is_empty())
        .map(|label| label.to_string())
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
#[cfg_attr(feature = "uniffi", uniffi::export(async_runtime = "tokio"))]
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

#[derive(Debug, Deserialize)]
struct LndListPermissionsResponse {
    method_permissions: Option<std::collections::HashMap<String, LndPermissionList>>,
}

#[derive(Debug, Deserialize, Serialize)]
struct LndPermissionList {
    permissions: Option<Vec<LndMacaroonPermission>>,
}

#[derive(Debug, Deserialize, Serialize)]
struct LndMacaroonPermission {
    entity: String,
    action: String,
}

#[derive(Debug, Serialize)]
struct LndCheckMacaroonPermissionsRequest {
    macaroon: String,
    permissions: Vec<LndMacaroonPermission>,
}

#[derive(Debug, Deserialize)]
struct LndCheckMacaroonPermissionsResponse {
    valid: Option<bool>,
}

pub async fn get_permissions(config: LndConfig) -> Result<crate::Permissions, ApiError> {
    let macaroon_bytes = hex::decode(config.macaroon.trim()).map_err(|e| ApiError::InvalidInput(
        format!("Invalid LND macaroon hex: {}", e),
    ))?;

    match get_lnd_remote_permissions(&config, &macaroon_bytes).await {
        Ok(permissions) => Ok(permissions),
        Err(error) => {
            let parsed = crate::permissions::parse_lnd_macaroon_permissions(&macaroon_bytes);
            if parsed.is_empty() {
                Err(error)
            } else {
                Ok(parsed)
            }
        }
    }
}

async fn get_lnd_remote_permissions(
    config: &LndConfig,
    macaroon_bytes: &[u8],
) -> Result<crate::Permissions, ApiError> {
    let client = async_client(config);
    let permissions_url = format!("{}/v1/macaroon/permissions", config.url);
    let permissions_response = client
        .get(&permissions_url)
        .send()
        .await
        .map_err(|e| ApiError::Http {
            reason: format!("Failed to list LND macaroon permissions: {}", e),
        })?;
    let permissions_text = permissions_response.text().await.map_err(|e| ApiError::Http {
        reason: format!("Failed to read LND permissions response: {}", e),
    })?;
    let permissions_payload: LndListPermissionsResponse = serde_json::from_str(&permissions_text)?;
    let macaroon = base64::encode(macaroon_bytes);
    let check_url = format!("{}/v1/macaroon/checkpermissions", config.url);
    let mut granted = Vec::new();

    for (method, permission_list) in permissions_payload.method_permissions.unwrap_or_default() {
        let check_response = client
            .post(&check_url)
            .json(&LndCheckMacaroonPermissionsRequest {
                macaroon: macaroon.clone(),
                permissions: permission_list.permissions.unwrap_or_default(),
            })
            .send()
            .await
            .map_err(|e| ApiError::Http {
                reason: format!("Failed to check LND macaroon permission for {method}: {e}"),
            })?;
        let check_text = check_response.text().await.map_err(|e| ApiError::Http {
            reason: format!("Failed to read LND permission check response for {method}: {e}"),
        })?;
        let check_payload: LndCheckMacaroonPermissionsResponse = serde_json::from_str(&check_text)?;
        if check_payload.valid.unwrap_or(false) {
            granted.push(method);
        }
    }

    Ok(crate::permissions::normalize_lnd_permissions(granted))
}

// get the one with the offer_id or label or get the first offer in the list or
pub async fn get_offer(config: &LndConfig, search: Option<String>) -> Result<Offer, ApiError> {
    return Err(ApiError::Json {
        reason: "Bolt12 not implemented".to_string(),
    });
}

pub async fn list_offers(config: &LndConfig, search: Option<String>) -> Result<Vec<Offer>, ApiError> {
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

pub async fn pay_offer(
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
#[cfg_attr(feature = "uniffi", uniffi::export(async_runtime = "tokio"))]
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

#[cfg_attr(feature = "uniffi", uniffi::export(async_runtime = "tokio"))]
pub async fn on_invoice_events(
    config: LndConfig,
    params: OnInvoiceEventParams,
    callback: std::sync::Arc<dyn OnInvoiceEventCallback>,
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
#[cfg_attr(feature = "uniffi", uniffi::export(async_runtime = "tokio"))]
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

#[cfg_attr(feature = "uniffi", uniffi::export(async_runtime = "tokio"))]
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

pub async fn prepare_onchain_transaction(
    config: LndConfig,
    params: PrepareOnchainTransactionParams,
) -> Result<OnchainTransaction, ApiError> {
    assert_valid_onchain_amount(params.amount_sats)?;

    let fee = params.fee.clone().unwrap_or_else(default_onchain_fee);
    let fee_payer = resolve_lnd_fee_payer(params.fee_payer.clone())?;
    let fee_request = resolve_lnd_fee_request(&fee)?;

    if fee_request.sat_per_vbyte.is_some() {
        return Ok(OnchainTransaction {
            id: None,
            address: params.address,
            amount_sats: params.amount_sats,
            fee_sats: None,
            total_amount_sats: None,
            recipient_amount_sats: Some(params.amount_sats),
            fee_payer,
            fee,
            expires_at: None,
            estimated_delivery_seconds: None,
            raw: Some(
                serde_json::json!({
                    "send_request": {
                        "sat_per_vbyte": fee_request
                            .sat_per_vbyte
                            .map(|sat_per_vbyte| sat_per_vbyte.to_string()),
                    },
                    "label": params.description,
                })
                .to_string(),
            ),
        });
    }

    let client = async_client(&config);
    let fee_url = format!("{}/v1/transactions/fee", config.url);
    let addr_amount_key = format!("AddrToAmount[{}]", params.address);
    let mut query = vec![(addr_amount_key, params.amount_sats.to_string())];
    if let Some(target_conf) = fee_request.target_conf {
        query.push(("target_conf".to_string(), target_conf.to_string()));
    }

    let response = client
        .get(&fee_url)
        .query(&query)
        .send()
        .await
        .map_err(|e| ApiError::Http {
            reason: format!("Failed to estimate LND on-chain fee: {}", e),
        })?;
    if !response.status().is_success() {
        let status = response.status();
        let error_text = response.text().await.unwrap_or_default();
        return Err(ApiError::Http {
            reason: format!("Failed to estimate LND on-chain fee: {} - {}", status, error_text),
        });
    }

    let response_text = response.text().await.unwrap_or_default();
    let estimate: EstimateFeeResponse = serde_json::from_str(&response_text)?;
    let fee_sats = parse_optional_fee_sats(estimate.fee_sat.clone());

    Ok(OnchainTransaction {
        id: None,
        address: params.address,
        amount_sats: params.amount_sats,
        fee_sats,
        total_amount_sats: fee_sats.map(|fee_sats| params.amount_sats + fee_sats),
        recipient_amount_sats: Some(params.amount_sats),
        fee_payer,
        fee,
        expires_at: None,
        estimated_delivery_seconds: fee_request.target_conf.map(|blocks| blocks * 10 * 60),
        raw: Some(
            serde_json::json!({
                "estimate": {
                    "fee_sat": estimate.fee_sat,
                    "sat_per_vbyte": estimate.sat_per_vbyte,
                },
                "label": params.description,
            })
            .to_string(),
        ),
    })
}

pub async fn pay_onchain(
    config: LndConfig,
    transaction: OnchainTransaction,
) -> Result<PayOnchainResponse, ApiError> {
    pay_onchain_with_options(config, transaction, PayOnchainOptions::default()).await
}

pub async fn pay_onchain_with_options(
    config: LndConfig,
    transaction: OnchainTransaction,
    options: PayOnchainOptions,
) -> Result<PayOnchainResponse, ApiError> {
    assert_valid_onchain_amount(transaction.amount_sats)?;
    let _fee_payer = resolve_lnd_fee_payer(Some(transaction.fee_payer.clone()))?;
    let fee_request = resolve_lnd_fee_request(&transaction.fee)?;
    assert_onchain_fee_guardrail(&transaction, options)?;

    let client = async_client(&config);
    let send_url = format!("{}/v1/transactions", config.url);
    let mut body = json!({
        "addr": transaction.address.clone(),
        "amount": transaction.amount_sats,
    });
    if let Some(target_conf) = fee_request.target_conf {
        body["target_conf"] = json!(target_conf);
    }
    if let Some(sat_per_vbyte) = fee_request.sat_per_vbyte {
        body["sat_per_vbyte"] = json!(sat_per_vbyte.to_string());
    }
    if let Some(label) = raw_label(&transaction) {
        body["label"] = json!(label);
    }

    let response = client
        .post(&send_url)
        .json(&body)
        .send()
        .await
        .map_err(|e| ApiError::Http {
            reason: format!("Failed to broadcast LND on-chain transaction: {}", e),
        })?;
    if !response.status().is_success() {
        let status = response.status();
        let error_text = response.text().await.unwrap_or_default();
        return Err(ApiError::Http {
            reason: format!("Failed to broadcast LND on-chain transaction: {} - {}", status, error_text),
        });
    }

    let response_text = response.text().await.unwrap_or_default();
    let send_response: SendCoinsResponse = serde_json::from_str(&response_text)?;

    Ok(PayOnchainResponse {
        payment_id: None,
        txid: send_response.txid.clone(),
        state: if send_response.txid.is_some() {
            "pending".to_string()
        } else {
            "failed".to_string()
        },
        address: transaction.address,
        amount_sats: transaction.amount_sats,
        fee_sats: transaction.fee_sats,
        total_amount_sats: transaction.total_amount_sats,
        recipient_amount_sats: transaction.recipient_amount_sats.or(Some(transaction.amount_sats)),
        created_at: None,
        raw: Some(response_text),
    })
}

#[cfg_attr(feature = "uniffi", uniffi::export(async_runtime = "tokio"))]
pub async fn decode(invoice_str: String) -> Result<String, ApiError> {
    crate::utils::decode_bolt11(invoice_str)
}

#[cfg_attr(feature = "uniffi", uniffi::export(async_runtime = "tokio"))]
pub async fn decode_offer(offer: String) -> Result<String, ApiError> {
    crate::utils::decode_offer(offer)
}

// Async version of list_transactions
#[cfg_attr(feature = "uniffi", uniffi::export(async_runtime = "tokio"))]
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

#[cfg(test)]
mod onchain_tests {
    use super::*;

    #[tokio::test]
    async fn prepare_manual_sat_per_vbyte_without_fee_estimate() {
        let transaction = prepare_onchain_transaction(
            LndConfig {
                url: "https://lnd.test".to_string(),
                macaroon: "00".to_string(),
                socks5_proxy: None,
                accept_invalid_certs: Some(true),
                http_timeout: Some(1),
            },
            PrepareOnchainTransactionParams {
                address: "bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh".to_string(),
                amount_sats: 10_000,
                fee: Some(OnchainFeePreference {
                    preference_type: OnchainFeePreferenceType::SatsPerVbyte,
                    speed: None,
                    target_conf: None,
                    sats_per_vbyte: Some(5.0),
                    backend: None,
                }),
                fee_payer: Some(OnchainFeePayer::Sender),
                description: Some("cold storage".to_string()),
                idempotency_key: None,
            },
        )
        .await
        .expect("manual sats/vbyte prepare should not call LND fee estimate");

        assert_eq!(transaction.amount_sats, 10_000);
        assert_eq!(transaction.recipient_amount_sats, Some(10_000));
        assert_eq!(transaction.fee_sats, None);
        assert_eq!(transaction.total_amount_sats, None);

        let raw: serde_json::Value =
            serde_json::from_str(transaction.raw.as_deref().unwrap_or_default()).unwrap();
        assert_eq!(raw["send_request"]["sat_per_vbyte"], "5");
        assert_eq!(raw["label"], "cold storage");
    }
}
