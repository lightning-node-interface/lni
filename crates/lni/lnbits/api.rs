use std::str::FromStr;
use std::time::Duration;

use lightning_invoice::Bolt11Invoice;

use super::types::*;
use super::LnBitsConfig;
use crate::types::NodeInfo;
use crate::{
    ApiError, CreateInvoiceParams, InvoiceType, OnInvoiceEventCallback, OnInvoiceEventParams,
    PayCode, PayInvoiceParams, PayInvoiceResponse, Transaction,
};
use reqwest::header;

// Docs: https://github.com/lnbits/lnbits/blob/main/docs/guide/api.md
// API: https://demo.lnbits.com/docs#/Payments

fn client(config: &LnBitsConfig) -> reqwest::Client {
    let mut headers = reqwest::header::HeaderMap::new();
    
    // LNBits uses X-Api-Key header for authentication
    match header::HeaderValue::from_str(&config.api_key) {
        Ok(api_key_header) => headers.insert("X-Api-Key", api_key_header),
        Err(_) => {
            eprintln!("Failed to create API key header");
            return reqwest::ClientBuilder::new()
                .default_headers(headers)
                .build()
                .unwrap_or_else(|_| reqwest::Client::new());
        }
    };
    
    headers.insert(
        "Content-Type",
        header::HeaderValue::from_static("application/json"),
    );

    // Create HTTP client with optional SOCKS5 proxy following existing patterns
    if let Some(proxy_url) = config.socks5_proxy.clone() {
        if !proxy_url.is_empty() {
            // Accept invalid certificates when using SOCKS5 proxy
            let client_builder = reqwest::Client::builder()
                .default_headers(headers.clone())
                .danger_accept_invalid_certs(config.accept_invalid_certs.unwrap_or(true));
            
            match reqwest::Proxy::all(&proxy_url) {
                Ok(proxy) => {
                    let mut builder = client_builder.proxy(proxy);
                    if let Some(timeout) = config.http_timeout {
                        builder = builder.timeout(Duration::from_secs(timeout as u64));
                    }
                    match builder.build() {
                        Ok(client) => return client,
                        Err(_) => {} // Fall through to default client
                    }
                }
                Err(_) => {} // Fall through to default client
            }
        }
    }

    // Default client without proxy
    let mut builder = reqwest::Client::builder().default_headers(headers);
    if let Some(timeout) = config.http_timeout {
        builder = builder.timeout(Duration::from_secs(timeout as u64));
    }
    if config.accept_invalid_certs.unwrap_or(false) {
        builder = builder.danger_accept_invalid_certs(true);
    }
    
    builder.build().unwrap_or_else(|_| reqwest::Client::new())
}

fn get_base_url(config: &LnBitsConfig) -> String {
    config.base_url.as_ref()
        .unwrap_or(&"https://demo.lnbits.com".to_string())
        .clone()
}

pub async fn get_info(config: &LnBitsConfig) -> Result<NodeInfo, ApiError> {
    let client = client(config);
    
    // Try to get wallet details first
    let wallet_url = format!("{}/api/v1/wallet", get_base_url(config));
    let response = client
        .get(&wallet_url)
        .send()
        .await
        .map_err(|e| ApiError::Http {
            reason: e.to_string(),
        })?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response.text().await.unwrap_or_default();
        return Err(ApiError::Http {
            reason: format!("HTTP {} - {}", status, error_text),
        });
    }

    let wallet: WalletDetails = response.json().await.map_err(|e| ApiError::Json {
        reason: e.to_string(),
    })?;

    Ok(NodeInfo {
        alias: wallet.name.clone(),
        color: "".to_string(),
        pubkey: wallet.id.clone(),
        network: "".to_string(),
        block_height: 0,
        block_hash: "".to_string(),
        send_balance_msat: wallet.balance_msat,
        receive_balance_msat: wallet.balance_msat,
        fee_credit_balance_msat: 0,
        unsettled_send_balance_msat: 0,
        unsettled_receive_balance_msat: 0,
        pending_open_send_balance: 0,
        pending_open_receive_balance: 0,
    })
}

pub async fn create_invoice(
    config: &LnBitsConfig,
    invoice_params: CreateInvoiceParams,
) -> Result<Transaction, ApiError> {
    match invoice_params.invoice_type {
        InvoiceType::Bolt11 => {
            let client = client(config);
            
            let amount_sats = invoice_params.amount_msats.unwrap_or(0) / 1000;
            
            let create_request = CreateInvoiceRequest {
                out: false, // false for incoming invoices
                amount: amount_sats,
                memo: invoice_params.description.clone(),
                unit: "sat".to_string(),
                expiry: invoice_params.expiry,
                webhook: None,
                internal: Some(false),
            };

            let req_url = format!("{}/api/v1/payments", get_base_url(config));
            let response = client
                .post(&req_url)
                .json(&create_request)
                .send()
                .await
                .map_err(|e| ApiError::Http {
                    reason: e.to_string(),
                })?;

            if !response.status().is_success() {
                let status = response.status();
                let error_text = response.text().await.unwrap_or_default();
                return Err(ApiError::Http {
                    reason: format!("HTTP {} - {}", status, error_text),
                });
            }

            let invoice_response: CreateInvoiceResponse = response.json().await.map_err(|e| ApiError::Json {
                reason: e.to_string(),
            })?;

            // Parse the bolt11 invoice to get expiry information
            let expires_at = match Bolt11Invoice::from_str(&invoice_response.payment_request) {
                Ok(invoice) => {
                    let created_at = invoice.duration_since_epoch().as_secs() as i64;
                    let expiry_duration = invoice.expiry_time().as_secs();
                    created_at + expiry_duration as i64
                }
                Err(_) => {
                    // Fallback calculation
                    chrono::Utc::now().timestamp() + invoice_params.expiry.unwrap_or(3600)
                }
            };

            Ok(Transaction {
                type_: "incoming".to_string(),
                invoice: invoice_response.payment_request,
                preimage: "".to_string(),
                payment_hash: invoice_response.payment_hash,
                amount_msats: invoice_params.amount_msats.unwrap_or(0),
                fees_paid: 0,
                created_at: chrono::Utc::now().timestamp(),
                expires_at,
                settled_at: 0,
                description: invoice_params.description.unwrap_or_default(),
                description_hash: invoice_params.description_hash.unwrap_or_default(),
                payer_note: Some("".to_string()),
                external_id: Some(invoice_response.checking_id),
            })
        }
        InvoiceType::Bolt12 => Err(ApiError::Json {
            reason: "Bolt12 not implemented for LNBits".to_string(),
        }),
    }
}

pub async fn pay_invoice(
    config: &LnBitsConfig,
    invoice_params: PayInvoiceParams,
) -> Result<PayInvoiceResponse, ApiError> {
    let client = client(config);

    let pay_request = PayInvoiceRequest {
        out: true, // true for outgoing payments
        bolt11: invoice_params.invoice.clone(),
    };

    let req_url = format!("{}/api/v1/payments", get_base_url(config));
    let response = client
        .post(&req_url)
        .json(&pay_request)
        .send()
        .await
        .map_err(|e| ApiError::Http {
            reason: e.to_string(),
        })?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response.text().await.unwrap_or_default();
        return Err(ApiError::Http {
            reason: format!("HTTP {} - {}", status, error_text),
        });
    }

    let pay_response: LnBitsPayInvoiceResponse = response.json().await.map_err(|e| ApiError::Json {
        reason: e.to_string(),
    })?;

    Ok(crate::PayInvoiceResponse {
        payment_hash: pay_response.payment_hash,
        preimage: "".to_string(), // Will be available later when checking payment status
        fee_msats: 0, // LNBits doesn't return fees in the initial response
    })
}

pub async fn lookup_invoice(
    config: &LnBitsConfig,
    payment_hash: Option<String>,
    _r_hash: Option<String>,
    _r_hash_str: Option<String>,
    _search: Option<String>,
) -> Result<Transaction, ApiError> {
    if payment_hash.is_none() {
        return Err(ApiError::Api {
            reason: "payment_hash is required for LNBits lookup_invoice".to_string(),
        });
    }

    let client = client(config);
    let hash = payment_hash.unwrap();
    
    let req_url = format!("{}/api/v1/payments/{}", get_base_url(config), hash);
    let response = client
        .get(&req_url)
        .send()
        .await
        .map_err(|e| ApiError::Http {
            reason: e.to_string(),
        })?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response.text().await.unwrap_or_default();
        return Err(ApiError::Http {
            reason: format!("HTTP {} - {}", status, error_text),
        });
    }

    let payment: Payment = response.json().await.map_err(|e| ApiError::Json {
        reason: e.to_string(),
    })?;

    Ok(Transaction {
        type_: if payment.amount > 0 { "incoming".to_string() } else { "outgoing".to_string() },
        invoice: payment.payment_request,
        preimage: payment.preimage.unwrap_or_default(),
        payment_hash: payment.payment_hash,
        amount_msats: payment.amount * 1000, // Convert sats to msats
        fees_paid: payment.fee.unwrap_or(0) * 1000, // Convert sats to msats
        created_at: payment.time,
        expires_at: payment.expiry.unwrap_or(payment.time + 3600),
        settled_at: if payment.pending { 0 } else { payment.time },
        description: payment.memo.unwrap_or_default(),
        description_hash: "".to_string(),
        payer_note: Some("".to_string()),
        external_id: Some(payment.checking_id),
    })
}

pub async fn list_transactions(
    config: &LnBitsConfig,
    _from: i64,
    _limit: i64,
    _search: Option<String>,
) -> Result<Vec<Transaction>, ApiError> {
    let client = client(config);
    
    let req_url = format!("{}/api/v1/payments", get_base_url(config));
    let response = client
        .get(&req_url)
        .send()
        .await
        .map_err(|e| ApiError::Http {
            reason: e.to_string(),
        })?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response.text().await.unwrap_or_default();
        return Err(ApiError::Http {
            reason: format!("HTTP {} - {}", status, error_text),
        });
    }

    let payments: Vec<Payment> = response.json().await.map_err(|e| ApiError::Json {
        reason: e.to_string(),
    })?;

    let mut transactions = Vec::new();
    for payment in payments {
        transactions.push(Transaction {
            type_: if payment.amount > 0 { "incoming".to_string() } else { "outgoing".to_string() },
            invoice: payment.payment_request,
            preimage: payment.preimage.unwrap_or_default(),
            payment_hash: payment.payment_hash,
            amount_msats: payment.amount * 1000, // Convert sats to msats
            fees_paid: payment.fee.unwrap_or(0) * 1000, // Convert sats to msats
            created_at: payment.time,
            expires_at: payment.expiry.unwrap_or(payment.time + 3600),
            settled_at: if payment.pending { 0 } else { payment.time },
            description: payment.memo.unwrap_or_default(),
            description_hash: "".to_string(),
            payer_note: Some("".to_string()),
            external_id: Some(payment.checking_id),
        });
    }

    Ok(transactions)
}

pub async fn decode(_config: &LnBitsConfig, str: String) -> Result<String, ApiError> {
    // For now, just return the original string
    // LNBits doesn't have a specific decode endpoint in the basic API
    match Bolt11Invoice::from_str(&str) {
        Ok(invoice) => Ok(format!("{:?}", invoice)),
        Err(e) => Err(ApiError::Api {
            reason: format!("Failed to decode invoice: {}", e),
        }),
    }
}

pub async fn get_offer(_config: &LnBitsConfig, _search: Option<String>) -> Result<PayCode, ApiError> {
    Err(ApiError::Api {
        reason: "BOLT12 offers not implemented for LNBits".to_string(),
    })
}

pub async fn list_offers(
    _config: &LnBitsConfig,
    _search: Option<String>,
) -> Result<Vec<PayCode>, ApiError> {
    Err(ApiError::Api {
        reason: "BOLT12 offers not implemented for LNBits".to_string(),
    })
}

pub async fn pay_offer(
    _config: &LnBitsConfig,
    _offer: String,
    _amount_msats: i64,
    _payer_note: Option<String>,
) -> Result<PayInvoiceResponse, ApiError> {
    Err(ApiError::Api {
        reason: "BOLT12 offers not implemented for LNBits".to_string(),
    })
}

pub async fn on_invoice_events(
    config: LnBitsConfig,
    params: OnInvoiceEventParams,
    callback: Box<dyn OnInvoiceEventCallback>,
) {
    let payment_hash = match params.payment_hash {
        Some(hash) => hash,
        None => {
            callback.failure(None);
            return;
        }
    };

    let polling_delay = Duration::from_secs(params.polling_delay_sec as u64);
    let max_duration = Duration::from_secs(params.max_polling_sec as u64);
    let start_time = std::time::Instant::now();

    loop {
        if start_time.elapsed() > max_duration {
            callback.failure(None);
            break;
        }

        match lookup_invoice(&config, Some(payment_hash.clone()), None, None, None).await {
            Ok(transaction) => {
                if transaction.settled_at > 0 {
                    callback.success(Some(transaction));
                    break;
                } else {
                    callback.pending(Some(transaction));
                }
            }
            Err(_) => {
                callback.pending(None);
            }
        }

        tokio::time::sleep(polling_delay).await;
    }
}