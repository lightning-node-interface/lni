use std::str::FromStr;
use std::time::Duration;

use lightning_invoice::Bolt11Invoice;

use super::types::{
    Amount, CreateReceiveRequestRequest, OnchainPaymentExecutionResponse,
    OnchainPaymentQuoteRequest, OnchainPaymentQuoteResponse, OnchainTierResponse,
    OnchainTiersRequest, PaymentExecutionResponse, PaymentQuoteRequest, PaymentQuoteResponse,
    PaymentsResponse, ReceiveRequestBolt11, StrikePaymentByIdResponse,
    StrikeReceiveRequestResponse, StrikeReceivesWithCountResponse,
};
use super::StrikeConfig;
use crate::types::NodeInfo;
use crate::{
    ApiError, CreateInvoiceParams, InvoiceType, Offer, OnInvoiceEventCallback,
    OnInvoiceEventParams, OnchainFeePayer, OnchainFeePreference, OnchainFeePreferenceType,
    OnchainFeeSpeed, OnchainTransaction, PayInvoiceParams, PayInvoiceResponse, PayOnchainOptions,
    PayOnchainResponse, PrepareOnchainTransactionParams, Transaction,
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
    client_builder
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
}

fn get_base_url(config: &StrikeConfig) -> &str {
    config
        .base_url
        .as_deref()
        .unwrap_or("https://api.strike.me/v1")
}

fn sats_to_btc_amount(amount_sats: i64) -> Result<Amount, ApiError> {
    if amount_sats <= 0 {
        return Err(ApiError::InvalidInput(
            "pay_onchain requires a positive amount_sats".to_string(),
        ));
    }

    Ok(Amount {
        amount: format!("{:.8}", amount_sats as f64 / 100_000_000.0),
        currency: "BTC".to_string(),
        fee_policy: None,
    })
}

fn amount_to_sats(amount: Option<&Amount>) -> Option<i64> {
    let amount = amount?;
    if amount.currency != "BTC" {
        return None;
    }

    amount
        .amount
        .parse::<f64>()
        .ok()
        .map(|btc| (btc * 100_000_000.0).round() as i64)
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

fn default_onchain_fee() -> OnchainFeePreference {
    OnchainFeePreference {
        preference_type: OnchainFeePreferenceType::Speed,
        speed: Some(OnchainFeeSpeed::Normal),
        target_conf: None,
        sats_per_vbyte: None,
        backend: None,
    }
}

fn resolve_onchain_fee_payer(fee_payer: Option<OnchainFeePayer>) -> OnchainFeePayer {
    fee_payer.unwrap_or(OnchainFeePayer::Sender)
}

fn strike_fee_policy(fee_payer: &OnchainFeePayer) -> &'static str {
    match fee_payer {
        OnchainFeePayer::Recipient => "INCLUSIVE",
        OnchainFeePayer::Sender => "EXCLUSIVE",
    }
}

fn normalize_strike_tier_speed(fee: &OnchainFeePreference) -> Result<&'static str, ApiError> {
    match fee.preference_type {
        OnchainFeePreferenceType::Default => Ok("standard"),
        OnchainFeePreferenceType::Backend => Err(ApiError::InvalidInput(
            "backend fee preferences should be handled before tier normalization".to_string(),
        )),
        OnchainFeePreferenceType::TargetConf | OnchainFeePreferenceType::SatsPerVbyte => {
            Err(ApiError::InvalidInput(format!(
                "Strike pay_onchain does not support {:?} fee preferences",
                fee.preference_type
            )))
        }
        OnchainFeePreferenceType::Speed => {
            match fee.speed.clone().unwrap_or(OnchainFeeSpeed::Normal) {
                OnchainFeeSpeed::Fast => Ok("fast"),
                OnchainFeeSpeed::Normal => Ok("standard"),
                OnchainFeeSpeed::Slow | OnchainFeeSpeed::Free => Ok("free"),
            }
        }
    }
}

fn normalize_onchain_state(state: Option<&String>) -> String {
    match state.map(|s| s.to_uppercase()) {
        Some(state) if state == "PENDING" => "pending".to_string(),
        Some(state) if state == "COMPLETED" || state == "SUCCESS" => "completed".to_string(),
        Some(state) if state == "FAILED" || state == "FAILURE" => "failed".to_string(),
        Some(state) => state.to_lowercase(),
        None => "pending".to_string(),
    }
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

    let balances: Vec<super::types::StrikeBalance> =
        response.json().await.map_err(|e| ApiError::Json {
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
                    fee_policy: None,
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
        get_base_url(&config),
        quote_resp.payment_quote_id
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

    let payment_url = format!("{}/payments/{}", get_base_url(&config), payment_id);
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
        payment_hash,             // Extract from BOLT11 invoice
        preimage: "".to_string(), // Strike doesn't expose preimage
        fee_msats,
    })
}

pub async fn prepare_onchain_transaction(
    config: StrikeConfig,
    params: PrepareOnchainTransactionParams,
) -> Result<OnchainTransaction, ApiError> {
    let client = async_client(&config);
    let fee = params.fee.clone().unwrap_or_else(default_onchain_fee);
    let fee_payer = resolve_onchain_fee_payer(params.fee_payer.clone());
    let amount = sats_to_btc_amount(params.amount_sats)?;
    let onchain_tier_id =
        resolve_onchain_tier_id(&client, &config, &params.address, &amount, &fee).await?;
    let mut quote_amount = amount;
    quote_amount.fee_policy = Some(strike_fee_policy(&fee_payer).to_string());

    let quote_url = format!("{}/payment-quotes/onchain", get_base_url(&config));
    let quote_request = OnchainPaymentQuoteRequest {
        btc_address: params.address.clone(),
        source_currency: "BTC".to_string(),
        description: params.description.clone(),
        amount: quote_amount,
        onchain_tier_id,
    };

    let mut request = client.post(&quote_url).json(&quote_request);
    if let Some(idempotency_key) = params.idempotency_key {
        request = request.header("idempotency-key", idempotency_key);
    }

    let response = request.send().await.map_err(|e| ApiError::Http {
        reason: e.to_string(),
    })?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response.text().await.unwrap_or_default();
        return Err(ApiError::Http {
            reason: format!(
                "Failed to create on-chain payment quote: {} - {}",
                status, error_text
            ),
        });
    }

    let quote_text = response.text().await.unwrap_or_default();
    let quote: OnchainPaymentQuoteResponse =
        serde_json::from_str(&quote_text).map_err(|e| ApiError::Json {
            reason: format!(
                "Failed to parse on-chain payment quote response: {} - Response: {}",
                e, quote_text
            ),
        })?;

    Ok(onchain_transaction_from_quote(
        params.address,
        params.amount_sats,
        fee,
        fee_payer,
        quote,
        quote_text,
    ))
}

pub async fn pay_onchain(
    config: StrikeConfig,
    transaction: OnchainTransaction,
) -> Result<PayOnchainResponse, ApiError> {
    pay_onchain_with_options(config, transaction, PayOnchainOptions::default()).await
}

pub async fn pay_onchain_with_options(
    config: StrikeConfig,
    transaction: OnchainTransaction,
    options: PayOnchainOptions,
) -> Result<PayOnchainResponse, ApiError> {
    let quote_id = transaction.id.clone().ok_or_else(|| {
        ApiError::InvalidInput("pay_onchain requires an on-chain transaction id".to_string())
    })?;

    assert_onchain_fee_guardrail(&transaction, options)?;

    let client = async_client(&config);
    let execute_url = format!(
        "{}/payment-quotes/{}/execute",
        get_base_url(&config),
        quote_id
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
            reason: format!(
                "Failed to execute on-chain payment: {} - {}",
                status, error_text
            ),
        });
    }

    let execute_text = execute_response.text().await.unwrap_or_default();
    let execution: OnchainPaymentExecutionResponse =
        serde_json::from_str(&execute_text).map_err(|e| ApiError::Json {
            reason: format!(
                "Failed to parse on-chain payment execution response: {} - Response: {}",
                e, execute_text
            ),
        })?;

    let payment_text = fetch_onchain_payment_text(&client, &config, &execution.payment_id).await;
    let payment = payment_text
        .as_deref()
        .and_then(|text| serde_json::from_str::<StrikePaymentByIdResponse>(text).ok());

    Ok(pay_onchain_response_from_payment(
        transaction,
        execution,
        execute_text,
        payment,
        payment_text,
    ))
}

async fn resolve_onchain_tier_id(
    client: &reqwest::Client,
    config: &StrikeConfig,
    address: &str,
    amount: &Amount,
    fee: &OnchainFeePreference,
) -> Result<String, ApiError> {
    if fee.preference_type == OnchainFeePreferenceType::Backend {
        return fee.backend.clone().ok_or_else(|| {
            ApiError::InvalidInput(
                "Strike backend fee preference requires a tier id value".to_string(),
            )
        });
    }

    let tier_speed = normalize_strike_tier_speed(fee)?;
    let tiers_url = format!("{}/payment-quotes/onchain/tiers", get_base_url(config));
    let tiers_response = client
        .post(&tiers_url)
        .json(&OnchainTiersRequest {
            btc_address: address.to_string(),
            amount: amount.clone(),
        })
        .send()
        .await
        .map_err(|e| ApiError::Http {
            reason: e.to_string(),
        })?;

    if !tiers_response.status().is_success() {
        let status = tiers_response.status();
        let error_text = tiers_response.text().await.unwrap_or_default();
        return Err(ApiError::Http {
            reason: format!(
                "Failed to get on-chain payment tiers: {} - {}",
                status, error_text
            ),
        });
    }

    let tiers_text = tiers_response.text().await.unwrap_or_default();
    let tiers: Vec<OnchainTierResponse> =
        serde_json::from_str(&tiers_text).map_err(|e| ApiError::Json {
            reason: format!(
                "Failed to parse on-chain payment tiers response: {} - Response: {}",
                e, tiers_text
            ),
        })?;

    tiers
        .iter()
        .find(|tier| tier.id == format!("tier_{}", tier_speed))
        .or_else(|| {
            tiers
                .iter()
                .find(|tier| tier.id.to_lowercase().contains(tier_speed))
        })
        .map(|tier| tier.id.clone())
        .ok_or_else(|| ApiError::Api {
            reason: format!(
                "Strike did not return an on-chain fee tier for {}",
                tier_speed
            ),
        })
}

fn onchain_transaction_from_quote(
    address: String,
    amount_sats: i64,
    fee: OnchainFeePreference,
    fee_payer: OnchainFeePayer,
    quote: OnchainPaymentQuoteResponse,
    raw: String,
) -> OnchainTransaction {
    OnchainTransaction {
        id: Some(quote.payment_quote_id),
        address,
        amount_sats,
        fee_sats: amount_to_sats(quote.total_fee.as_ref()),
        total_amount_sats: amount_to_sats(Some(&quote.total_amount)),
        recipient_amount_sats: amount_to_sats(Some(&quote.amount)),
        fee_payer,
        fee,
        expires_at: quote
            .valid_until
            .as_ref()
            .and_then(|dt| chrono::DateTime::parse_from_rfc3339(dt).ok())
            .map(|dt| dt.timestamp()),
        estimated_delivery_seconds: quote.estimated_delivery_duration_in_min.map(|m| m * 60),
        raw: Some(raw),
    }
}

async fn fetch_onchain_payment_text(
    client: &reqwest::Client,
    config: &StrikeConfig,
    payment_id: &str,
) -> Option<String> {
    let payment_url = format!("{}/payments/{}", get_base_url(config), payment_id);
    let response = client.get(&payment_url).send().await.ok()?;
    if !response.status().is_success() {
        return None;
    }
    response.text().await.ok()
}

fn pay_onchain_response_from_payment(
    transaction: OnchainTransaction,
    execution: OnchainPaymentExecutionResponse,
    execute_raw: String,
    payment: Option<StrikePaymentByIdResponse>,
    payment_raw: Option<String>,
) -> PayOnchainResponse {
    let payment_id = payment
        .as_ref()
        .and_then(|p| p.payment_id.clone().or_else(|| p.id.clone()))
        .unwrap_or_else(|| execution.payment_id.clone());
    let txid = payment
        .as_ref()
        .and_then(|p| p.onchain.as_ref().and_then(|o| o.txn_id.clone()))
        .or_else(|| execution.onchain.as_ref().and_then(|o| o.txn_id.clone()));
    let state = normalize_onchain_state(
        payment
            .as_ref()
            .and_then(|p| p.state.as_ref())
            .or(execution.state.as_ref()),
    );
    let amount_sats = amount_to_sats(
        payment
            .as_ref()
            .and_then(|p| p.amount.as_ref())
            .or(execution.amount.as_ref()),
    )
    .unwrap_or(transaction.amount_sats);
    let fee_sats = amount_to_sats(
        payment
            .as_ref()
            .and_then(|p| p.total_fee.as_ref())
            .or(execution.total_fee.as_ref()),
    )
    .or(transaction.fee_sats);
    let total_amount_sats = amount_to_sats(
        payment
            .as_ref()
            .and_then(|p| p.total_amount.as_ref())
            .or(execution.total_amount.as_ref()),
    )
    .or(transaction.total_amount_sats);
    let created_at = payment
        .as_ref()
        .and_then(|p| p.created.as_ref())
        .and_then(|dt| chrono::DateTime::parse_from_rfc3339(dt).ok())
        .map(|dt| dt.timestamp());

    PayOnchainResponse {
        payment_id: Some(payment_id),
        txid,
        state,
        address: transaction.address,
        amount_sats,
        fee_sats,
        total_amount_sats,
        recipient_amount_sats: transaction.recipient_amount_sats,
        created_at,
        raw: payment_raw.or(Some(execute_raw)),
    }
}

pub fn decode(str: String) -> Result<String, ApiError> {
    crate::utils::decode_bolt11(str)
}

pub fn decode_offer(offer: String) -> Result<String, ApiError> {
    crate::utils::decode_offer(offer)
}

pub fn get_offer(_config: &StrikeConfig, _search: Option<String>) -> Result<Offer, ApiError> {
    Err(ApiError::Json {
        reason: "Bolt12 not implemented for Strike".to_string(),
    })
}

pub async fn list_offers(
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
        get_base_url(&config),
        target_payment_hash
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
    let receives_resp: StrikeReceivesWithCountResponse = serde_json::from_str(&response_text)
        .map_err(|e| ApiError::Json {
            reason: format!(
                "Failed to parse receives response: {} - Response: {}",
                e, response_text
            ),
        })?;

    // Get the first item from the response
    let receive = receives_resp
        .items
        .into_iter()
        .next()
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
            receive
                .completed
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
        get_base_url(&config),
        from,
        limit
    );
    let receives_response = client
        .get(&receives_url)
        .send()
        .await
        .map_err(|e| ApiError::Http {
            reason: e.to_string(),
        })?;

    let mut transactions: Vec<Transaction> = Vec::new();

    if receives_response.status().is_success() {
        let receives_text = receives_response.text().await.unwrap();
        let receives_resp: StrikeReceivesWithCountResponse = serde_json::from_str(&receives_text)
            .map_err(|e| ApiError::Json {
            reason: format!(
                "Failed to parse receives response: {} - Response: {}",
                e, receives_text
            ),
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
                        receive
                            .completed
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
    let payments_url = format!(
        "{}/payments?skip={}&top={}",
        get_base_url(&config),
        from,
        limit
    );
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
pub async fn poll_invoice_events<F>(
    config: StrikeConfig,
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

        let (status, transaction) = match lookup_invoice(
            config.clone(),
            params.payment_hash.clone(),
            None,
            None,
            params.search.clone(),
        )
        .await
        {
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

        tokio::time::sleep(tokio::time::Duration::from_secs(
            params.polling_delay_sec as u64,
        ))
        .await;
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
    })
    .await;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_onchain_transaction(fee_sats: Option<i64>) -> OnchainTransaction {
        OnchainTransaction {
            id: Some("quote-1".to_string()),
            address: "bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh".to_string(),
            amount_sats: 10_000,
            fee_sats,
            total_amount_sats: fee_sats.map(|fee| 10_000 + fee),
            recipient_amount_sats: Some(10_000),
            fee_payer: OnchainFeePayer::Sender,
            fee: OnchainFeePreference {
                preference_type: OnchainFeePreferenceType::Speed,
                speed: Some(OnchainFeeSpeed::Normal),
                target_conf: None,
                sats_per_vbyte: None,
                backend: None,
            },
            expires_at: None,
            estimated_delivery_seconds: None,
            raw: None,
        }
    }

    #[test]
    fn default_fee_guardrail_blocks_high_percent_fee() {
        let error = assert_onchain_fee_guardrail(
            &test_onchain_transaction(Some(3_000)),
            PayOnchainOptions::default(),
        )
        .unwrap_err();

        assert!(error.to_string().contains("max_fee_percent"));
    }

    #[test]
    fn custom_fee_guardrail_can_allow_higher_fee() {
        let options = PayOnchainOptions {
            fee_guardrail: Some(crate::types::OnchainFeeGuardrail {
                max_fee_sats: Some(5_000),
                max_fee_percent: Some(50.0),
            }),
            dangerously_disable_fee_guardrail: false,
        };

        assert!(
            assert_onchain_fee_guardrail(&test_onchain_transaction(Some(3_000)), options).is_ok()
        );
    }

    #[test]
    fn fee_guardrail_fails_closed_when_fee_is_unknown() {
        let error = assert_onchain_fee_guardrail(
            &test_onchain_transaction(None),
            PayOnchainOptions::default(),
        )
        .unwrap_err();

        assert!(error.to_string().contains("fee_sats is unknown"));
    }

    #[test]
    fn dangerous_fee_guardrail_opt_out_allows_unknown_fee() {
        let options = PayOnchainOptions {
            fee_guardrail: None,
            dangerously_disable_fee_guardrail: true,
        };

        assert!(assert_onchain_fee_guardrail(&test_onchain_transaction(None), options).is_ok());
    }
}
