use std::str::FromStr;
use std::time::Duration;

use base64;
use lightning_invoice::Bolt11Invoice;
use reqwest::header;

use super::types::*;
use super::SpeedConfig;
use crate::types::NodeInfo;
use crate::{
    ApiError, CreateInvoiceParams, InvoiceType, Offer, OnInvoiceEventCallback, OnInvoiceEventParams,
    PayInvoiceParams, PayInvoiceResponse, Transaction,
};

// Docs: https://apidocs.tryspeed.com/

fn client(config: &SpeedConfig) -> reqwest::Client {
    let mut headers = reqwest::header::HeaderMap::new();

    // Speed uses HTTP Basic Auth with API key as username, no password (hence the colon)
    let api_key_with_colon = config.api_key.clone() + ":";
    let auth_value = base64::encode(&api_key_with_colon);

    let auth_header = format!("Basic {}", auth_value);

    match header::HeaderValue::from_str(&auth_header) {
        Ok(auth_header_value) => headers.insert(header::AUTHORIZATION, auth_header_value),
        Err(_) => {
            eprintln!("Failed to create authorization header");
            return reqwest::ClientBuilder::new()
                .default_headers(headers)
                .build()
                .unwrap_or_else(|_| reqwest::Client::new());
        }
    };
    
    headers.insert(
        header::CONTENT_TYPE,
        header::HeaderValue::from_static("application/json"),
    );

    // Create HTTP client with optional SOCKS5 proxy following Strike pattern
    if let Some(proxy_url) = config.socks5_proxy.clone() {
        if !proxy_url.is_empty() {
            // Accept invalid certificates when using SOCKS5 proxy
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

fn get_base_url(config: &SpeedConfig) -> &str {
    config.base_url.as_deref().unwrap_or("https://api.tryspeed.com")
}

pub async fn get_info(config: &SpeedConfig) -> Result<NodeInfo, ApiError> {
    let client = client(config);

    // Get balance from Speed API
    let response = client
        .get(&format!("{}/balances", get_base_url(config)))
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

    let balance_response: SpeedBalanceResponse = response.json().await.map_err(|e| ApiError::Json {
        reason: e.to_string(),
    })?;

    // Extract SATS balance and convert to millisats
    let send_balance_msat = balance_response
        .available
        .iter()
        .find(|balance| balance.target_currency == "SATS")
        .map(|balance| (balance.amount * 1000.0) as i64)
        .unwrap_or(0);

    Ok(NodeInfo {
        alias: "Speed Node".to_string(),
        color: "".to_string(),
        pubkey: "".to_string(),
        network: "mainnet".to_string(),
        block_height: 0,
        block_hash: "".to_string(),
        send_balance_msat,
        receive_balance_msat: 0,
        fee_credit_balance_msat: 0,
        unsettled_send_balance_msat: 0,
        unsettled_receive_balance_msat: 0,
        pending_open_send_balance: 0,
        pending_open_receive_balance: 0,
    })
}

pub async fn create_invoice(
    config: &SpeedConfig,
    invoice_params: CreateInvoiceParams,
) -> Result<Transaction, ApiError> {
    match invoice_params.invoice_type {
        InvoiceType::Bolt11 => {
            let client = client(config);

            let request = SpeedCreatePaymentRequest {
                amount: (invoice_params.amount_msats.unwrap_or(0) as f64) / 1000.0, // Convert msats to sats
                currency: "SATS".to_string(),
                memo: invoice_params.description.clone(),
                external_id: None,
            };

            let response = client
                .post(&format!("{}/payments", get_base_url(config)))
                .json(&request)
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

            let payment: SpeedCreatePaymentResponse =
                response.json().await.map_err(|e| ApiError::Json {
                    reason: e.to_string(),
                })?;

            // Extract Lightning payment request from payment_method_options
            let lightning_invoice = payment
                .payment
                .payment_method_options
                .as_ref()
                .and_then(|options| options.get("lightning"))
                .and_then(|lightning| lightning.as_object())
                .and_then(|lightning_obj| lightning_obj.get("payment_request"))
                .and_then(|pr| pr.as_str())
                .unwrap_or_default();

            // Parse the BOLT11 invoice to get payment hash and expiry
            let (payment_hash, expires_at) = if !lightning_invoice.is_empty() {
                match Bolt11Invoice::from_str(lightning_invoice) {
                    Ok(bolt11) => {
                        let hash = format!("{:x}", bolt11.payment_hash());
                        let expiry = bolt11
                            .expires_at()
                            .map(|duration| duration.as_secs() as i64)
                            .unwrap_or(0);
                        (hash, expiry)
                    }
                    Err(_) => (String::new(), 0),
                }
            } else {
                (String::new(), 0)
            };

            Ok(Transaction {
                type_: "incoming".to_string(),
                invoice: lightning_invoice.to_string(),
                preimage: "".to_string(), // Not available in Speed API
                payment_hash,
                amount_msats: (payment.payment.amount * 1000.0) as i64,
                fees_paid: payment
                    .payment
                    .speed_fee
                    .as_ref()
                    .and_then(|f| f.amount)
                    .map(|a| (a * 1000.0) as i64)
                    .unwrap_or(0),
                created_at: payment.payment.created,
                expires_at,
                settled_at: payment.payment.target_amount_paid_at.unwrap_or(0),
                description: payment.payment.statement_descriptor.unwrap_or_default(),
                description_hash: invoice_params.description_hash.unwrap_or_default(),
                payer_note: Some("".to_string()),
                external_id: Some(payment.payment.id),
            })
        }
        InvoiceType::Bolt12 => Err(ApiError::Json {
            reason: "Bolt12 not implemented for Speed".to_string(),
        }),
    }
}

pub async fn pay_invoice(
    config: &SpeedConfig,
    invoice_params: PayInvoiceParams,
) -> Result<PayInvoiceResponse, ApiError> {
    let client = client(config);

    // Extract amount from invoice or use provided amount
    let amount = if let Some(amount_msats) = invoice_params.amount_msats {
        amount_msats as f64 / 1000.0 // Convert msats to sats
    } else {
        // Try to extract amount from BOLT11 invoice
        match Bolt11Invoice::from_str(&invoice_params.invoice) {
            Ok(bolt11) => {
                if let Some(amount_msat) = bolt11.amount_milli_satoshis() {
                    amount_msat as f64 / 1000.0 // Convert to sats
                } else {
                    return Err(ApiError::Json {
                        reason: "Zero amount invoice requires amount_msats parameter".to_string(),
                    });
                }
            }
            Err(e) => {
                return Err(ApiError::Json {
                    reason: format!("Failed to parse BOLT11 invoice: {}", e),
                });
            }
        }
    };

    let request = SpeedPayInvoiceRequest {
        amount,
        currency: "SATS".to_string(),
        target_currency: "SATS".to_string(),
        withdraw_method: "lightning".to_string(),
        withdraw_request: invoice_params.invoice.clone(),
        note: Some("LNI payment".to_string()),
        external_id: None,
    };

    let response = client
        .post(&format!("{}/send", get_base_url(config)))
        .json(&request)
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

    let send_response: SpeedSendResponse = response.json().await.map_err(|e| ApiError::Json {
        reason: format!("error decoding response body: {}", e),
    })?;

    // Extract payment hash from the BOLT11 invoice since it's not in the response
    let payment_hash = match Bolt11Invoice::from_str(&invoice_params.invoice) {
        Ok(invoice) => format!("{:x}", invoice.payment_hash()),
        Err(_) => "".to_string(),
    };

    Ok(PayInvoiceResponse {
        payment_hash,
        preimage: "".to_string(), // Not available in Speed send response
        fee_msats: (send_response.speed_fee.amount * 1000) as i64,
    })
}

pub async fn decode(_config: &SpeedConfig, str: String) -> Result<String, ApiError> {
    // Speed doesn't have a decode endpoint, return raw string
    Ok(str)
}

pub async fn get_offer(_config: &SpeedConfig, _search: Option<String>) -> Result<Offer, ApiError> {
    Err(ApiError::Json {
        reason: "Bolt12 not implemented for Speed".to_string(),
    })
}

pub async fn list_offers(
    _config: &SpeedConfig,
    _search: Option<String>,
) -> Result<Vec<Offer>, ApiError> {
    Err(ApiError::Json {
        reason: "Bolt12 not implemented for Speed".to_string(),
    })
}

pub async fn create_offer(
    _config: &SpeedConfig,
    _amount_msats: Option<i64>,
    _description: Option<String>,
    _expiry: Option<i64>,
) -> Result<Transaction, ApiError> {
    Err(ApiError::Json {
        reason: "Bolt12 not implemented for Speed".to_string(),
    })
}

pub async fn fetch_invoice_from_offer(
    _config: &SpeedConfig,
    _offer: String,
    _amount_msats: i64,
    _payer_note: Option<String>,
) -> Result<crate::cln::types::FetchInvoiceResponse, ApiError> {
    Err(ApiError::Json {
        reason: "Bolt12 not implemented for Speed".to_string(),
    })
}

pub async fn pay_offer(
    _config: &SpeedConfig,
    _offer: String,
    _amount_msats: i64,
    _payer_note: Option<String>,
) -> Result<PayInvoiceResponse, ApiError> {
    Err(ApiError::Json {
        reason: "Bolt12 not implemented for Speed".to_string(),
    })
}

// Helper function to fetch send transactions using the /send/filter endpoint
async fn fetch_send_transactions(
    config: &SpeedConfig,
    status_filter: Option<Vec<String>>,
    withdraw_request_filter: Option<String>,
) -> Result<Vec<SpeedSendResponse>, ApiError> {
    let client = client(config);

    let request = SpeedSendFilterRequest {
        status: status_filter,
        withdraw_request: withdraw_request_filter,
    };

    let response = client
        .post(&format!("{}/send/filter", get_base_url(config)))
        .json(&request)
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

    let filter_response: SpeedSendFilterResponse = response.json().await.map_err(|e| ApiError::Json {
        reason: format!("error decoding response body: {}", e),
    })?;

    Ok(filter_response.data)
}

// Helper function to convert SpeedSendResponse to Transaction
fn convert_send_to_transaction(send_tx: SpeedSendResponse) -> Transaction {
    // Extract payment hash from the withdraw_request (BOLT11 invoice)
    let payment_hash = if !send_tx.withdraw_request.is_empty() {
        match Bolt11Invoice::from_str(&send_tx.withdraw_request) {
            Ok(invoice) => format!("{:x}", invoice.payment_hash()),
            Err(_) => "".to_string(),
        }
    } else {
        "".to_string()
    };

    // Determine transaction type based on withdraw_method
    let type_ = if send_tx.withdraw_method == "lightning" {
        "outgoing" // Send transactions are outgoing payments
    } else {
        "outgoing"
    };

    // Convert amounts to millisats
    let amount_msats = (send_tx.target_amount * 1000.0) as i64;
    let fees_paid = (send_tx.speed_fee.amount as i64) * 1000; // Convert to millisats

    // Determine settled_at based on status
    let settled_at = if send_tx.status == "paid" {
        send_tx.modified.unwrap_or(send_tx.created)
    } else {
        0
    };

    Transaction {
        type_: type_.to_string(),
        invoice: send_tx.withdraw_request.clone(),
        preimage: "".to_string(), // Not available in send response
        payment_hash,
        amount_msats,
        fees_paid,
        created_at: send_tx.created,
        expires_at: 0, // Not applicable for send transactions
        settled_at,
        description: send_tx.note.clone().unwrap_or_default(),
        description_hash: "".to_string(),
        payer_note: send_tx.note,
        external_id: Some(send_tx.id),
    }
}

pub async fn lookup_invoice(
    config: &SpeedConfig,
    payment_hash: Option<String>,
    _from: Option<i64>,
    _limit: Option<i64>,
    search: Option<String>,
) -> Result<Transaction, ApiError> {
    // For lookup_invoice, we need to find a specific transaction
    // If we have a payment_hash, we need to extract the invoice from it first
    // If we have a search term, use it directly as the withdraw_request filter

    let withdraw_request_filter = if let Some(search_term) = search {
        Some(search_term)
    } else if payment_hash.is_some() {
        // We can't reverse a payment hash back to an invoice, so we'll search all paid transactions
        // and filter client-side by payment hash
        None
    } else {
        return Err(ApiError::Json {
            reason: "Either payment_hash or search parameter is required".to_string(),
        });
    };

    // Fetch transactions from all statuses to ensure we find the transaction
    let statuses = vec![
        "paid".to_string(),
        "unpaid".to_string(),
        "failed".to_string(),
    ];
    let send_transactions =
        fetch_send_transactions(config, Some(statuses), withdraw_request_filter).await?;

    // Convert to Transaction and find the matching one
    let mut transactions = Vec::new();
    for send_tx in send_transactions {
        let transaction = convert_send_to_transaction(send_tx);
        transactions.push(transaction);
    }

    // If we have a payment hash, filter by it
    if let Some(target_hash) = payment_hash {
        let transaction = transactions
            .into_iter()
            .find(|t| t.payment_hash == target_hash)
            .ok_or_else(|| ApiError::Json {
                reason: format!("Transaction not found for payment hash: {}", target_hash),
            })?;
        Ok(transaction)
    } else if transactions.len() == 1 {
        Ok(transactions.into_iter().next().unwrap())
    } else if transactions.is_empty() {
        Err(ApiError::Json {
            reason: "No transactions found matching search criteria".to_string(),
        })
    } else {
        // Multiple matches, return the first one
        Ok(transactions.into_iter().next().unwrap())
    }
}

pub async fn list_transactions(
    config: &SpeedConfig,
    _from: i64,
    limit: i64,
    search: Option<String>,
) -> Result<Vec<Transaction>, ApiError> {
    // Use the new /send/filter endpoint to get all transactions
    let withdraw_request_filter = search.clone();

    // If search is not set, use default status filter for unpaid, paid, and failed
    let status_filter = if search.is_none() {
        Some(vec!["unpaid".to_string(), "paid".to_string(), "failed".to_string()])
    } else {
        None
    };

    let send_transactions = fetch_send_transactions(config, status_filter, withdraw_request_filter).await?;

    dbg!(&send_transactions);

    // Convert to Transaction objects
    let mut transactions: Vec<Transaction> = send_transactions
        .into_iter()
        .map(convert_send_to_transaction)
        .collect();

    // Sort by created_at descending (newest first)
    transactions.sort_by(|a, b| b.created_at.cmp(&a.created_at));

    // Apply limit
    if limit > 0 {
        transactions.truncate(limit as usize);
    }

    Ok(transactions)
}

// Core logic shared by both implementations
pub async fn poll_invoice_events<F>(config: &SpeedConfig, params: OnInvoiceEventParams, mut callback: F)
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

        let (status, transaction) = match lookup_invoice(
            config,
            params.payment_hash.clone(),
            None,
            None,
            params.search.clone(),
        ).await {
            Ok(transaction) => {
                if transaction.settled_at > 0 {
                    ("success".to_string(), Some(transaction))
                } else {
                    ("pending".to_string(), Some(transaction))
                }
            }
            Err(_) => ("error".to_string(), None),
        };

        callback(status.clone(), transaction.clone());

        if status == "success" || status == "failure" {
            break;
        }

        tokio::time::sleep(Duration::from_secs(params.polling_delay_sec as u64)).await;
    }
}

pub async fn on_invoice_events(
    config: SpeedConfig,
    params: OnInvoiceEventParams,
    callback: std::sync::Arc<dyn OnInvoiceEventCallback>,
) {
    poll_invoice_events(&config, params, move |status, tx| match status.as_str() {
        "success" => callback.success(tx),
        "pending" => callback.pending(tx),
        "failure" | _ => callback.failure(tx),
    }).await;
}
