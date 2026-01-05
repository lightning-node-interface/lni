use std::time::Duration;
use std::str::FromStr;

use lightning_invoice::Bolt11Invoice;

use super::types::{
    Amount, CreateReceiveRequestRequest, PaymentExecutionResponse, PaymentQuoteRequest,
    PaymentQuoteResponse, PaymentsResponse, ReceiveRequestBolt11,
    StrikeReceiveRequestResponse, StrikeReceivesWithCountResponse,
};
use super::StrikeConfig;
use crate::types::NodeInfo;
use crate::{
    ApiError, CreateInvoiceParams, InvoiceType, Offer, OnInvoiceEventCallback, OnInvoiceEventParams,
    PayInvoiceParams, PayInvoiceResponse, Transaction,
};
use reqwest::header;

// Docs
// https://docs.strike.me/api/

fn async_client(config: &StrikeConfig) -> reqwest::Client {
    let mut headers = reqwest::header::HeaderMap::new();
    let auth_header = format!("Bearer {}", config.api_key);
    headers.insert(
        "Authorization",
        header::HeaderValue::from_str(&auth_header).unwrap(),
    );
    headers.insert(
        "Content-Type",
        header::HeaderValue::from_static("application/json"),
    );

    // Create HTTP client with optional SOCKS5 proxy following LND pattern
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

fn get_base_url(config: &StrikeConfig) -> &str {
    config.base_url.as_deref().unwrap_or("https://api.strike.me/v1")
}

pub async fn get_info(config: StrikeConfig) -> Result<NodeInfo, ApiError> {
    let client = async_client(&config);

    // Get balance from Strike API
    let response = client
        .get(&format!("{}/balances", get_base_url(&config)))
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

    let balances: Vec<super::types::StrikeBalance> = response.json().await.map_err(|e| ApiError::Json {
        reason: e.to_string(),
    })?;

    // Extract BTC balance and convert to millisats
    let send_balance_msat = balances
        .iter()
        .find(|balance| balance.currency == "BTC")
        .map(|balance| {
            let btc_amount = balance.current.parse::<f64>().unwrap_or(0.0);
            (btc_amount * 100_000_000_000.0) as i64
        })
        .unwrap_or(0);

    Ok(NodeInfo {
        alias: "Strike Node".to_string(),
        color: "".to_string(),
        pubkey: "".to_string(),
        network: "mainnet".to_string(),
        block_height: 0,
        block_hash: "".to_string(),
        send_balance_msat,
        receive_balance_msat: 0,
        fee_credit_balance_msat: 0,        // No fee credit for Strike
        unsettled_send_balance_msat: 0,    // No unsettled balance
        unsettled_receive_balance_msat: 0, // No unsettled balance
        pending_open_send_balance: 0,      // No pending opens
        pending_open_receive_balance: 0,   // No pending opens
    })
}

pub async fn create_invoice(
    config: StrikeConfig,
    invoice_params: CreateInvoiceParams,
) -> Result<Transaction, ApiError> {
    let client = async_client(&config);

    match invoice_params.get_invoice_type() {
        InvoiceType::Bolt11 => {
            // Create a receive request with bolt11 configuration
            let req_url = format!("{}/receive-requests", get_base_url(&config));

            let amount = invoice_params.amount_msats.map(|amt| {
                // Convert msats to BTC (Strike expects BTC amounts)
                let btc_amount = amt as f64 / 100_000_000_000.0;
                Amount {
                    amount: format!("{:.8}", btc_amount),
                    currency: "BTC".to_string(),
                }
            });

            let create_request = CreateReceiveRequestRequest {
                bolt11: Some(ReceiveRequestBolt11 {
                    amount,
                    description: invoice_params.description.clone(),
                    description_hash: invoice_params.description_hash.clone(),
                    expiry_in_seconds: invoice_params.expiry,
                }),
                onchain: None,
                target_currency: Some("BTC".to_string()),
            };

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
                    reason: format!(
                        "Failed to create receive request: {} - {}",
                        status, error_text
                    ),
                });
            }

            let response_text = response.text().await.unwrap();

            // Try to parse as Strike's actual receive request response format
            let receive_request_resp: StrikeReceiveRequestResponse =
                serde_json::from_str(&response_text).map_err(|e| ApiError::Json {
                    reason: format!(
                        "Failed to parse receive request response: {} - Response: {}",
                        e, response_text
                    ),
                })?;

            // Extract bolt11 info from the receive request
            let bolt11_info = receive_request_resp.bolt11.ok_or_else(|| ApiError::Json {
                reason: "No bolt11 information in receive request response".to_string(),
            })?;

            Ok(Transaction {
                type_: "incoming".to_string(),
                invoice: bolt11_info.invoice,
                preimage: "".to_string(),
                payment_hash: bolt11_info.payment_hash,
                amount_msats: invoice_params.amount_msats.unwrap_or(0),
                fees_paid: 0,
                created_at: chrono::DateTime::parse_from_rfc3339(&receive_request_resp.created)
                    .map(|dt| dt.timestamp())
                    .unwrap_or(0),
                expires_at: chrono::DateTime::parse_from_rfc3339(&bolt11_info.expires)
                    .map(|dt| dt.timestamp())
                    .unwrap_or(0),
                settled_at: 0,
                description: bolt11_info.description.unwrap_or_default(),
                description_hash: invoice_params.description_hash.unwrap_or_default(),
                payer_note: Some("".to_string()),
                external_id: Some(receive_request_resp.receive_request_id),
            })
        }
        InvoiceType::Bolt12 => Err(ApiError::Json {
            reason: "Bolt12 not implemented for Strike".to_string(),
        }),
    }
}

pub async fn pay_invoice(
    config: StrikeConfig,
    invoice_params: PayInvoiceParams,
) -> Result<PayInvoiceResponse, ApiError> {
    let client = async_client(&config);

    // Create payment quote first
    let quote_url = format!("{}/payment-quotes/lightning", get_base_url(&config));
    let quote_request = PaymentQuoteRequest {
        ln_invoice: invoice_params.invoice.clone(),
        source_currency: "BTC".to_string(),
        amount: invoice_params
            .amount_msats
            .map(|amt| super::types::PaymentQuoteAmount {
                amount: format!("{:.8}", amt as f64 / 100_000_000_000.0),
                currency: "BTC".to_string(),
            }),
    };

    let quote_response = client
        .post(&quote_url)
        .json(&quote_request)
        .send()
        .await
        .map_err(|e| ApiError::Http {
            reason: e.to_string(),
        })?;

    if !quote_response.status().is_success() {
        let status = quote_response.status();
        let error_text = quote_response.text().await.unwrap_or_default();
        return Err(ApiError::Http {
            reason: format!(
                "Failed to create payment quote: {} - {}",
                status, error_text
            ),
        });
    }

    let quote_text = quote_response.text().await.unwrap();
    let quote_resp: PaymentQuoteResponse = serde_json::from_str(&quote_text)?;

    // Execute the payment quote
    let execute_url = format!(
        "{}/payment-quotes/{}/execute",
        get_base_url(&config), quote_resp.payment_quote_id
    );
    let execute_response = client
        .patch(&execute_url)
        .send()
        .await
        .map_err(|e| ApiError::Http {
            reason: e.to_string(),
        })?;

    if !execute_response.status().is_success() {
        let status = execute_response.status();
        let error_text = execute_response.text().await.unwrap_or_default();
        return Err(ApiError::Http {
            reason: format!("Failed to execute payment: {} - {}", status, error_text),
        });
    }

    let execute_text = execute_response.text().await.unwrap();
    let execute_resp: PaymentExecutionResponse = serde_json::from_str(&execute_text)?;

    // Get payment details
    let payment_id = &execute_resp.payment_id;
    
    let payment_url = format!(
        "{}/payments/{}",
        get_base_url(&config), payment_id
    );
    let payment_response = client
        .get(&payment_url)
        .send()
        .await
        .map_err(|e| ApiError::Http {
            reason: e.to_string(),
        })?;

    if !payment_response.status().is_success() {
        let status = payment_response.status();
        let error_text = payment_response.text().await.unwrap_or_default();
        return Err(ApiError::Http {
            reason: format!("Failed to get payment details: {} - {}", status, error_text),
        });
    }

    let payment_text = payment_response.text().await.unwrap();
    let payment_resp: PaymentExecutionResponse = serde_json::from_str(&payment_text)?;

    let fee_msats = if let Some(lightning) = &payment_resp.lightning {
        let fee_amount = lightning.network_fee.amount.parse::<f64>().unwrap_or(0.0);
        if lightning.network_fee.currency == "BTC" {
            (fee_amount * 100_000_000_000.0) as i64
        } else {
            0
        }
    } else {
        0
    };

    // Extract payment hash from the original BOLT11 invoice
    let payment_hash = match Bolt11Invoice::from_str(&invoice_params.invoice) {
        Ok(invoice) => {
            format!("{:x}", invoice.payment_hash())
        }
        Err(_) => "".to_string(), // If parsing fails, return empty string
    };

    Ok(PayInvoiceResponse {
        payment_hash, // Extract from BOLT11 invoice
        preimage: "".to_string(), // Strike doesn't expose preimage
        fee_msats,
    })
}

pub fn decode(_config: &StrikeConfig, str: String) -> Result<String, ApiError> {
    // Strike doesn't have a decode endpoint, return raw string
    Ok(str)
}

pub fn get_offer(_config: &StrikeConfig, _search: Option<String>) -> Result<Offer, ApiError> {
    Err(ApiError::Json {
        reason: "Bolt12 not implemented for Strike".to_string(),
    })
}

pub fn list_offers(
    _config: &StrikeConfig,
    _search: Option<String>,
) -> Result<Vec<Offer>, ApiError> {
    Err(ApiError::Json {
        reason: "Bolt12 not implemented for Strike".to_string(),
    })
}

pub fn create_offer(
    _config: &StrikeConfig,
    _amount_msats: Option<i64>,
    _description: Option<String>,
    _expiry: Option<i64>,
) -> Result<Transaction, ApiError> {
    Err(ApiError::Json {
        reason: "Bolt12 not implemented for Strike".to_string(),
    })
}

pub fn fetch_invoice_from_offer(
    _config: &StrikeConfig,
    _offer: String,
    _amount_msats: i64,
    _payer_note: Option<String>,
) -> Result<crate::cln::types::FetchInvoiceResponse, ApiError> {
    Err(ApiError::Json {
        reason: "Bolt12 not implemented for Strike".to_string(),
    })
}

pub fn pay_offer(
    _config: &StrikeConfig,
    _offer: String,
    _amount_msats: i64,
    _payer_note: Option<String>,
) -> Result<PayInvoiceResponse, ApiError> {
    Err(ApiError::Json {
        reason: "Bolt12 not implemented for Strike".to_string(),
    })
}

pub async fn lookup_invoice(
    config: StrikeConfig,
    payment_hash: Option<String>,
    _from: Option<i64>,
    _limit: Option<i64>,
    _search: Option<String>,
) -> Result<Transaction, ApiError> {
    let client = async_client(&config);

    let target_payment_hash = payment_hash.unwrap_or_default();
    
    // Use the receive-requests/receives endpoint with payment hash query parameter
    let receives_url = format!(
        "{}/receive-requests/receives?$paymentHash={}",
        get_base_url(&config), target_payment_hash
    );
    let response = client
        .get(&receives_url)
        .send()
        .await
        .map_err(|e| ApiError::Http {
            reason: e.to_string(),
        })?;

    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Err(ApiError::Json {
            reason: "Receive not found".to_string(),
        });
    }

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response.text().await.unwrap_or_default();
        return Err(ApiError::Http {
            reason: format!("Failed to get receives: {} - {}", status, error_text),
        });
    }

    let response_text = response.text().await.unwrap();
    
    // Try to parse as Strike's receives response format with count
    let receives_resp: StrikeReceivesWithCountResponse =
        serde_json::from_str(&response_text).map_err(|e| ApiError::Json {
            reason: format!("Failed to parse receives response: {} - Response: {}", e, response_text),
        })?;

    // Get the first item from the response
    let receive = receives_resp.items.into_iter().next()
        .ok_or_else(|| ApiError::Json {
            reason: format!("No receive found for payment hash: {}", target_payment_hash),
        })?;

    let lightning_info = receive.lightning.ok_or_else(|| ApiError::Json {
        reason: "No lightning information in receive".to_string(),
    })?;

    // Convert amount to millisatoshis
    let amount_msats = if receive.amount_received.currency == "BTC" {
        let btc_amount = receive.amount_received.amount.parse::<f64>().unwrap_or(0.0);
        (btc_amount * 100_000_000_000.0) as i64
    } else {
        0
    };

    Ok(Transaction {
        type_: "incoming".to_string(),
        invoice: lightning_info.invoice,
        preimage: lightning_info.preimage,
        payment_hash: lightning_info.payment_hash,
        amount_msats,
        fees_paid: 0,
        created_at: chrono::DateTime::parse_from_rfc3339(&receive.created)
            .map(|dt| dt.timestamp())
            .unwrap_or(0),
        expires_at: 0, // Not available in receives response
        settled_at: if receive.state == "COMPLETED" {
            receive.completed
                .as_ref()
                .and_then(|dt| chrono::DateTime::parse_from_rfc3339(dt).ok())
                .map(|dt| dt.timestamp())
                .unwrap_or(0)
        } else {
            0
        },
        description: lightning_info.description.unwrap_or_else(|| {
            // If no description, use description_hash if available
            lightning_info.description_hash.clone().unwrap_or_default()
        }),
        description_hash: lightning_info.description_hash.clone().unwrap_or_default(),
        payer_note: Some("".to_string()),
        external_id: Some(receive.receive_request_id),
    })
}

pub async fn list_transactions(
    config: StrikeConfig,
    from: i64,
    limit: i64,
    _search: Option<String>,
) -> Result<Vec<Transaction>, ApiError> {
    let client = async_client(&config);

    // Get receives (incoming) using the receives endpoint similar to lookup_invoice
    let receives_url = format!(
        "{}/receive-requests/receives?$skip={}&$top={}",
        get_base_url(&config), from, limit
    );
    let receives_response =
        client
            .get(&receives_url)
            .send()
            .await
            .map_err(|e| ApiError::Http {
                reason: e.to_string(),
            })?;

    let mut transactions: Vec<Transaction> = Vec::new();

    if receives_response.status().is_success() {
        let receives_text = receives_response.text().await.unwrap();
        let receives_resp: StrikeReceivesWithCountResponse =
            serde_json::from_str(&receives_text).map_err(|e| ApiError::Json {
                reason: format!("Failed to parse receives response: {} - Response: {}", e, receives_text),
            })?;

        for receive in receives_resp.items {
            if let Some(lightning_info) = receive.lightning {
                // Convert amount to millisatoshis
                let amount_msats = if receive.amount_received.currency == "BTC" {
                    let btc_amount = receive.amount_received.amount.parse::<f64>().unwrap_or(0.0);
                    (btc_amount * 100_000_000_000.0) as i64
                } else {
                    0
                };

                transactions.push(Transaction {
                    type_: "incoming".to_string(),
                    invoice: lightning_info.invoice,
                    preimage: lightning_info.preimage,
                    payment_hash: lightning_info.payment_hash,
                    amount_msats,
                    fees_paid: 0,
                    created_at: chrono::DateTime::parse_from_rfc3339(&receive.created)
                        .map(|dt| dt.timestamp())
                        .unwrap_or(0),
                    expires_at: 0, // Not available in receives response
                    settled_at: if receive.state == "COMPLETED" {
                        receive.completed
                            .as_ref()
                            .and_then(|dt| chrono::DateTime::parse_from_rfc3339(dt).ok())
                            .map(|dt| dt.timestamp())
                            .unwrap_or(0)
                    } else {
                        0
                    },
                    description: lightning_info.description.unwrap_or_else(|| {
                        // If no description, use description_hash if available
                        lightning_info.description_hash.clone().unwrap_or_default()
                    }),
                    description_hash: lightning_info.description_hash.clone().unwrap_or_default(),
                    payer_note: Some("".to_string()),
                    external_id: Some(receive.receive_request_id),
                });
            }
        }
    }

    // Get payments (outgoing)
    let payments_url = format!("{}/payments?skip={}&top={}", get_base_url(&config), from, limit);
    let payments_response = client
        .get(&payments_url)
        .send()
        .await
        .map_err(|e| ApiError::Http {
            reason: e.to_string(),
        })?;

    if payments_response.status().is_success() {
        let payments_text = payments_response.text().await.unwrap();
        let payments_resp: PaymentsResponse = serde_json::from_str(&payments_text)?;

        for payment in payments_resp.data {
            let amount_msats = if payment.amount.currency == "BTC" {
                let btc_amount = payment.amount.amount.parse::<f64>().unwrap_or(0.0);
                (btc_amount * 100_000_000_000.0) as i64
            } else {
                0
            };

            let fee_msats = if let Some(lightning) = &payment.lightning {
                if let Some(network_fee) = &lightning.network_fee {
                    let fee_amount = network_fee.amount.parse::<f64>().unwrap_or(0.0);
                    if network_fee.currency == "BTC" {
                        (fee_amount * 100_000_000_000.0) as i64
                    } else {
                        0
                    }
                } else {
                    0
                }
            } else {
                0
            };

            transactions.push(Transaction {
                type_: "outgoing".to_string(),
                invoice: payment
                    .lightning
                    .as_ref()
                    .and_then(|l| l.payment_request.clone())
                    .unwrap_or_default(),
                preimage: "".to_string(),
                payment_hash: payment
                    .lightning
                    .as_ref()
                    .and_then(|l| l.payment_hash.clone())
                    .unwrap_or_default(),
                amount_msats,
                fees_paid: fee_msats,
                created_at: chrono::DateTime::parse_from_rfc3339(&payment.created)
                    .map(|dt| dt.timestamp())
                    .unwrap_or(0),
                expires_at: 0,
                settled_at: if payment.state == "COMPLETED" {
                    payment
                        .completed
                        .as_ref()
                        .and_then(|dt| chrono::DateTime::parse_from_rfc3339(dt).ok())
                        .map(|dt| dt.timestamp())
                        .unwrap_or(0)
                } else {
                    0
                },
                description: payment.description.unwrap_or_default(),
                description_hash: "".to_string(),
                payer_note: Some("".to_string()),
                external_id: Some(payment.id),
            });
        }
    }

    // Sort by created date descending
    transactions.sort_by(|a, b| b.created_at.cmp(&a.created_at));

    Ok(transactions)
}

// Core logic shared by both implementations
pub async fn poll_invoice_events<F>(config: StrikeConfig, params: OnInvoiceEventParams, mut callback: F)
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
            config.clone(),
            params.payment_hash.clone(),
            None,
            None,
            params.search.clone(),
        ).await {
            Ok(transaction) => {
                if transaction.settled_at > 0 {
                    ("settled".to_string(), Some(transaction))
                } else {
                    ("pending".to_string(), Some(transaction))
                }
            }
            Err(_) => ("error".to_string(), None),
        };

        match status.as_str() {
            "settled" => {
                callback("success".to_string(), transaction);
                break;
            }
            "error" => {
                callback("failure".to_string(), transaction);
            }
            _ => {
                callback("pending".to_string(), transaction);
            }
        }

        tokio::time::sleep(tokio::time::Duration::from_secs(params.polling_delay_sec as u64)).await;
    }
}

pub async fn on_invoice_events(
    config: StrikeConfig,
    params: OnInvoiceEventParams,
    callback: std::sync::Arc<dyn OnInvoiceEventCallback>,
) {
    poll_invoice_events(config, params, move |status, tx| match status.as_str() {
        "success" => callback.success(tx),
        "pending" => callback.pending(tx),
        "failure" => callback.failure(tx),
        _ => {}
    }).await;
}
