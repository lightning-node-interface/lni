use std::time::Duration;

use super::types::{
    AlbyBalancesResponse, AlbyCreateInvoiceRequest, AlbyCreateInvoiceResponse,
    AlbyInfoResponse, AlbyLookupInvoiceResponse, AlbyPayInvoiceRequest,
    AlbyPaymentResponse, AlbyTransactionsResponse,
};
use super::AlbyConfig;
use crate::types::NodeInfo;
use crate::{
    ApiError, CreateInvoiceParams, OnInvoiceEventCallback, OnInvoiceEventParams, PayCode,
    PayInvoiceParams, PayInvoiceResponse, Transaction,
};
use reqwest::header;

// Docs
// Based on Alby Hub API patterns from wails handlers

fn async_client(config: &AlbyConfig) -> reqwest::Client {
    let mut headers = reqwest::header::HeaderMap::new();
    
    // Add Authorization header
    let auth_header = format!("Bearer {}", config.api_key);
    headers.insert(
        "Authorization",
        header::HeaderValue::from_str(&auth_header).unwrap(),
    );
    
    // Add Alby-specific headers
    headers.insert(
        "AlbyHub-Name",
        header::HeaderValue::from_str(&config.alby_hub_name).unwrap(),
    );
    headers.insert(
        "AlbyHub-Region",
        header::HeaderValue::from_str(&config.alby_hub_region).unwrap(),
    );
    
    headers.insert(
        "Content-Type",
        header::HeaderValue::from_static("application/json"),
    );

    // Create HTTP client with optional SOCKS5 proxy following the same pattern as other implementations
    if let Some(proxy_url) = config.socks5_proxy.clone() {
        if !proxy_url.is_empty() {
            let client_builder = reqwest::Client::builder()
                .default_headers(headers.clone())
                .danger_accept_invalid_certs(true);
            
            match reqwest::Proxy::all(&proxy_url) {
                Ok(proxy) => {
                    let mut builder = client_builder.proxy(proxy);
                    if config.http_timeout.is_some() {
                        builder = builder.timeout(std::time::Duration::from_secs(
                            config.http_timeout.unwrap_or_default() as u64,
                        ));
                    }
                    match builder.build() {
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
    if config.http_timeout.is_some() {
        client_builder = client_builder.timeout(std::time::Duration::from_secs(
            config.http_timeout.unwrap_or_default() as u64,
        ));
    }
    client_builder.build().unwrap_or_else(|_| reqwest::Client::new())
}

fn get_base_url(config: &AlbyConfig) -> &str {
    config.base_url.as_deref().unwrap_or("https://my.albyhub.com/api")
}

pub async fn get_info(config: AlbyConfig) -> Result<NodeInfo, ApiError> {
    let client = async_client(&config);

    // Get node info from Alby Hub API
    let info_response = client
        .get(&format!("{}/info", get_base_url(&config)))
        .send()
        .await
        .map_err(|e| ApiError::Http {
            reason: e.to_string(),
        })?;

    if !info_response.status().is_success() {
        let status = info_response.status();
        let error_text = info_response.text().await.unwrap_or_default();
        return Err(ApiError::Http {
            reason: format!("HTTP {} - {}", status, error_text),
        });
    }

    let info: AlbyInfoResponse = info_response.json().await.map_err(|e| ApiError::Json {
        reason: e.to_string(),
    })?;

    // Get balance from Alby Hub API
    let balance_response = client
        .get(&format!("{}/balances", get_base_url(&config)))
        .send()
        .await
        .map_err(|e| ApiError::Http {
            reason: e.to_string(),
        })?;

    if !balance_response.status().is_success() {
        let status = balance_response.status();
        let error_text = balance_response.text().await.unwrap_or_default();
        return Err(ApiError::Http {
            reason: format!("HTTP {} - {}", status, error_text),
        });
    }

    let balances: AlbyBalancesResponse = balance_response.json().await.map_err(|e| ApiError::Json {
        reason: e.to_string(),
    })?;

    // Convert Alby balance to msat (assuming it's in sats)
    let balance_msat = balances.balances
        .iter()
        .find(|b| b.currency == "btc" || b.currency == "sats")
        .map(|b| b.balance * 1000)
        .unwrap_or(0);

    Ok(NodeInfo {
        alias: info.alias,
        color: info.color,
        pubkey: info.pubkey,
        network: info.network,
        block_height: info.block_height,
        block_hash: info.block_hash,
        send_balance_msat: balance_msat,
        receive_balance_msat: 0, // Alby Hub doesn't distinguish send/receive balance
        ..Default::default()
    })
}

pub async fn create_invoice(
    config: AlbyConfig,
    params: CreateInvoiceParams,
) -> Result<Transaction, ApiError> {
    let client = async_client(&config);
    
    let request = AlbyCreateInvoiceRequest {
        amount: params.amount_msats.unwrap_or(0) / 1000, // Convert msat to sat
        description: params.description.clone(),
        expiry: params.expiry,
    };

    let response = client
        .post(&format!("{}/invoices", get_base_url(&config)))
        .json(&request)
        .send()
        .await
        .map_err(|e| ApiError::Http {
            reason: format!("Failed to create invoice: {}", e),
        })?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response.text().await.unwrap_or_default();
        return Err(ApiError::Http {
            reason: format!("HTTP {} - {}", status, error_text),
        });
    }

    let create_response: AlbyCreateInvoiceResponse = response.json().await.map_err(|e| ApiError::Json {
        reason: format!("Failed to parse create invoice response: {}", e),
    })?;

    Ok(Transaction {
        type_: "incoming".to_string(),
        invoice: create_response.payment_request,
        preimage: "".to_string(),
        payment_hash: create_response.payment_hash,
        amount_msats: create_response.amount * 1000, // Convert sat to msat
        fees_paid: 0,
        created_at: parse_timestamp(&create_response.created_at),
        expires_at: parse_timestamp(&create_response.expires_at),
        settled_at: 0,
        description: create_response.description,
        description_hash: "".to_string(),
        payer_note: Some("".to_string()),
        external_id: Some("".to_string()),
    })
}

pub async fn pay_invoice(
    config: AlbyConfig,
    params: PayInvoiceParams,
) -> Result<PayInvoiceResponse, ApiError> {
    let client = async_client(&config);
    
    let request = AlbyPayInvoiceRequest {
        invoice: params.invoice.clone(),
        amount: params.amount_msats.map(|a| a / 1000), // Convert msat to sat if provided
    };

    let response = client
        .post(&format!("{}/payments/{}", get_base_url(&config), params.invoice))
        .json(&request)
        .send()
        .await
        .map_err(|e| ApiError::Http {
            reason: format!("Failed to pay invoice: {}", e),
        })?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response.text().await.unwrap_or_default();
        return Err(ApiError::Http {
            reason: format!("HTTP {} - {}", status, error_text),
        });
    }

    let payment_response: AlbyPaymentResponse = response.json().await.map_err(|e| ApiError::Json {
        reason: format!("Failed to parse pay invoice response: {}", e),
    })?;

    if payment_response.status != "settled" && payment_response.status != "succeeded" {
        return Err(ApiError::Api {
            reason: format!("Payment failed with status: {}", payment_response.status),
        });
    }

    Ok(PayInvoiceResponse {
        payment_hash: payment_response.payment_hash,
        preimage: payment_response.payment_preimage,
        fee_msats: payment_response.fee * 1000, // Convert sat to msat
    })
}

pub async fn lookup_invoice(
    config: AlbyConfig,
    payment_hash: Option<String>,
    _from: Option<i64>,
    _limit: Option<i64>,
    _search: Option<String>,
) -> Result<Transaction, ApiError> {
    let payment_hash_str = payment_hash.unwrap_or_default();
    let client = async_client(&config);
    
    let response = client
        .get(&format!("{}/transactions/{}", get_base_url(&config), payment_hash_str))
        .send()
        .await
        .map_err(|e| ApiError::Http {
            reason: format!("Failed to lookup invoice: {}", e),
        })?;

    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Err(ApiError::Json {
            reason: "Invoice not found".to_string(),
        });
    }

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response.text().await.unwrap_or_default();
        return Err(ApiError::Http {
            reason: format!("HTTP {} - {}", status, error_text),
        });
    }

    let invoice: AlbyLookupInvoiceResponse = response.json().await.map_err(|e| ApiError::Json {
        reason: format!("Failed to parse lookup invoice response: {}", e),
    })?;

    Ok(Transaction {
        type_: "incoming".to_string(),
        invoice: invoice.payment_request,
        preimage: invoice.payment_preimage.unwrap_or_default(),
        payment_hash: invoice.payment_hash,
        amount_msats: invoice.amount * 1000, // Convert sat to msat
        fees_paid: invoice.fee.unwrap_or(0) * 1000, // Convert sat to msat
        created_at: parse_timestamp(&invoice.created_at),
        expires_at: parse_timestamp(&invoice.expires_at),
        settled_at: invoice.settled_at.as_ref().map(|s| parse_timestamp(s)).unwrap_or(0),
        description: invoice.description.unwrap_or_default(),
        description_hash: "".to_string(),
        payer_note: Some("".to_string()),
        external_id: Some("".to_string()),
    })
}

pub async fn list_transactions(
    config: AlbyConfig,
    from: i64,
    limit: i64,
    _search: Option<String>,
) -> Result<Vec<Transaction>, ApiError> {
    let client = async_client(&config);
    
    let mut url = format!("{}/transactions", get_base_url(&config));
    let mut params = vec![];
    
    if limit > 0 {
        params.push(format!("limit={}", limit));
    }
    if from > 0 {
        params.push(format!("offset={}", from));
    }
    
    if !params.is_empty() {
        url.push('?');
        url.push_str(&params.join("&"));
    }

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| ApiError::Http {
            reason: format!("Failed to list transactions: {}", e),
        })?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response.text().await.unwrap_or_default();
        return Err(ApiError::Http {
            reason: format!("HTTP {} - {}", status, error_text),
        });
    }

    let txns: AlbyTransactionsResponse = response.json().await.map_err(|e| ApiError::Json {
        reason: format!("Failed to parse list transactions response: {}", e),
    })?;

    let mut transactions: Vec<Transaction> = txns
        .transactions
        .into_iter()
        .map(|txn| Transaction {
            type_: txn.type_,
            invoice: txn.payment_request.unwrap_or_default(),
            preimage: txn.payment_preimage.unwrap_or_default(),
            payment_hash: txn.payment_hash,
            amount_msats: txn.amount * 1000, // Convert sat to msat
            fees_paid: txn.fee.unwrap_or(0) * 1000, // Convert sat to msat
            created_at: parse_timestamp(&txn.created_at),
            expires_at: 0, // Not provided in list response
            settled_at: txn.settled_at.as_ref().map(|s| parse_timestamp(s)).unwrap_or(0),
            description: txn.description.unwrap_or_default(),
            description_hash: "".to_string(),
            payer_note: Some("".to_string()),
            external_id: Some("".to_string()),
        })
        .collect();

    transactions.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(transactions)
}

pub fn decode(_config: &AlbyConfig, _invoice_str: String) -> Result<String, ApiError> {
    // Alby Hub doesn't have a specific decode endpoint
    // For now, we'll return a simple error indicating this isn't supported
    Err(ApiError::Api {
        reason: "Invoice decode not implemented for Alby Hub".to_string(),
    })
}

pub async fn on_invoice_events(
    config: AlbyConfig,
    params: OnInvoiceEventParams,
    callback: Box<dyn OnInvoiceEventCallback>,
) {
    let start_time = std::time::Instant::now();
    
    loop {
        if start_time.elapsed() > Duration::from_secs(params.max_polling_sec as u64) {
            callback.failure(None);
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
        
        match lookup_result {
            Ok(transaction) => {
                if transaction.settled_at > 0 {
                    callback.success(Some(transaction));
                    break;
                } else {
                    callback.pending(Some(transaction));
                }
            }
            Err(_) => {
                callback.failure(None);
                // Continue polling on error
            }
        }

        tokio::time::sleep(Duration::from_secs(params.polling_delay_sec as u64)).await;
    }
}

pub fn get_offer(_config: &AlbyConfig, _search: Option<String>) -> Result<PayCode, ApiError> {
    Err(ApiError::Api {
        reason: "Bolt12 offers not implemented for Alby Hub".to_string(),
    })
}

pub fn list_offers(_config: &AlbyConfig, _search: Option<String>) -> Result<Vec<PayCode>, ApiError> {
    Err(ApiError::Api {
        reason: "Bolt12 offers not implemented for Alby Hub".to_string(),
    })
}

pub fn pay_offer(
    _config: &AlbyConfig,
    _offer: String,
    _amount_msats: i64,
    _payer_note: Option<String>,
) -> Result<PayInvoiceResponse, ApiError> {
    Err(ApiError::Api {
        reason: "Bolt12 offers not implemented for Alby Hub".to_string(),
    })
}

fn parse_timestamp(timestamp_str: &str) -> i64 {
    // Try to parse ISO 8601 timestamp
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(timestamp_str) {
        return dt.timestamp();
    }
    
    // Try to parse as Unix timestamp
    if let Ok(timestamp) = timestamp_str.parse::<i64>() {
        return timestamp;
    }
    
    // Default to 0 if parsing fails
    0
}