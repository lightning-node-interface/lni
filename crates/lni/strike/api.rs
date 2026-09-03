use std::collections::HashMap;
use std::str::FromStr;
use std::time::Duration;

use lightning_invoice::Bolt11Invoice;

use super::types::{
    Amount, CreateReceiveRequestRequest, OnchainPaymentExecutionResponse,
    OnchainPaymentQuoteRequest, OnchainPaymentQuoteResponse, OnchainTierResponse,
    OnchainTiersRequest, Payment, PaymentExecutionResponse, PaymentQuoteRequest,
    PaymentQuoteResponse, PaymentsResponse, ReceiveRequestBolt11, StrikePaymentByIdResponse,
    StrikeReceive, StrikeReceiveRequestResponse, StrikeReceivesWithCountResponse,
};
use super::StrikeConfig;
use crate::error_normalization::{
    provider_error_from_response, transport_error, ProviderErrorInfo,
};
use crate::types::NodeInfo;
use crate::{
    ApiError, CreateInvoiceParams, InvoiceType, Offer, OnInvoiceEventCallback,
    OnInvoiceEventParams, OnchainFeePayer, OnchainFeePreference, OnchainFeePreferenceType,
    OnchainFeeSpeed, OnchainTransaction, PayInvoiceParams, PayInvoiceResponse, PayOnchainOptions,
    PayOnchainResponse, PrepareOnchainTransactionParams, SettlementState, SettlementType,
    Transaction,
};
use reqwest::header;

// Docs
// https://docs.strike.me/api/

fn async_client(config: &StrikeConfig) -> Result<reqwest::Client, ApiError> {
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
    if let Some(proxy_url) = config.socks5_proxy.as_deref().filter(|url| !url.is_empty()) {
        let mut client_builder = crate::http_client_builder().default_headers(headers.clone());
        if config.accept_invalid_certs.unwrap_or(false) {
            client_builder = client_builder.danger_accept_invalid_certs(true);
        }
        if let Some(http_timeout) = config.http_timeout {
            client_builder =
                client_builder.timeout(std::time::Duration::from_secs(http_timeout as u64));
        }

        let proxy = reqwest::Proxy::all(proxy_url).map_err(|_| ApiError::Http {
            reason: "Invalid Strike SOCKS5 proxy configuration".to_string(),
        })?;
        return client_builder
            .proxy(proxy)
            .build()
            .map_err(|_| ApiError::Http {
                reason: "Failed to build Strike SOCKS5 proxy client".to_string(),
            });
    }

    // Default client creation
    let mut client_builder = crate::http_client_builder().default_headers(headers);
    if config.accept_invalid_certs.unwrap_or(false) {
        client_builder = client_builder.danger_accept_invalid_certs(true);
    }
    if config.http_timeout.is_some() {
        client_builder = client_builder.timeout(std::time::Duration::from_secs(
            config.http_timeout.unwrap_or_default() as u64,
        ));
    }
    crate::build_http_client(client_builder, "Failed to build Strike HTTP client")
}

fn get_base_url(config: &StrikeConfig) -> &str {
    config
        .base_url
        .as_deref()
        .unwrap_or("https://api.strike.me/v1")
}

fn map_strike_provider_error(info: &ProviderErrorInfo) -> Option<&'static str> {
    match info.code.as_deref()?.to_ascii_uppercase().as_str() {
        "BALANCE_TOO_LOW" => Some("INSUFFICIENT_BALANCE"),
        "RATE_LIMIT_EXCEEDED" | "TOO_MANY_ATTEMPTS" => Some("RATE_LIMITED"),
        "FORBIDDEN" => Some("RESTRICTED"),
        "UNAUTHORIZED" => Some("UNAUTHORIZED"),
        "AMOUNT_TOO_HIGH" | "TOO_MANY_TRANSACTIONS" | "DEPOSIT_LIMIT_EXCEEDED" => {
            Some("QUOTA_EXCEEDED")
        }
        "INVALID_LN_INVOICE"
        | "INVALID_STATE_FOR_INVOICE_EXPIRED"
        | "LN_ROUTE_NOT_FOUND"
        | "PAYMENT_QUOTE_EXPIRED"
        | "INVALID_RECIPIENT" => Some("PAYMENT_FAILED"),
        "EXCHANGE_RATE_NOT_AVAILABLE"
        | "LN_UNAVAILABLE"
        | "SERVICE_UNAVAILABLE"
        | "MAINTENANCE_MODE"
        | "BAD_GATEWAY"
        | "GATEWAY_TIMEOUT"
        | "INTERNAL_SERVER_ERROR" => Some("INTERNAL"),
        "NOT_FOUND" => Some("NOT_FOUND"),
        _ => None,
    }
}

fn strike_nwc_error_from_response(
    status: reqwest::StatusCode,
    body: String,
    operation: &str,
    _context: &str,
) -> ApiError {
    provider_error_from_response("strike", operation, status, body, map_strike_provider_error)
}

fn strike_nwc_error_from_transport(error: reqwest::Error, operation: &str) -> ApiError {
    transport_error("strike", operation, error)
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

fn amount_to_msats(amount: Option<&Amount>) -> i64 {
    amount_to_sats(amount).unwrap_or_default() * 1000
}

fn normalize_settlement_state(state: Option<&str>) -> SettlementState {
    match state.map(str::to_ascii_uppercase).as_deref() {
        Some("PENDING") => SettlementState::Pending,
        Some("COMPLETED" | "SUCCESS") => SettlementState::Completed,
        Some("FAILED" | "FAILURE") => SettlementState::Failed,
        _ => SettlementState::Unknown,
    }
}

fn normalize_settlement_type(
    type_: Option<&str>,
    state: Option<&str>,
    has_lightning: bool,
    has_onchain: bool,
    has_p2p: bool,
    txid: Option<&str>,
) -> SettlementType {
    let type_ = type_.map(str::to_ascii_uppercase);

    if txid.is_some_and(|value| !value.is_empty()) {
        SettlementType::Onchain
    } else if matches!(type_.as_deref(), Some("P2P")) || has_p2p {
        SettlementType::Intraledger
    } else if matches!(type_.as_deref(), Some("LIGHTNING")) || has_lightning {
        SettlementType::Lightning
    } else if normalize_settlement_state(state) == SettlementState::Completed {
        SettlementType::Intraledger
    } else if matches!(type_.as_deref(), Some("ONCHAIN")) || has_onchain {
        SettlementType::Onchain
    } else {
        SettlementType::Unknown
    }
}

fn normalized_payment_id(payment: &Payment) -> Option<&str> {
    payment.payment_id.as_deref().or(payment.id.as_deref())
}

fn merge_payment_snapshots(mut listed: Payment, direct: Payment) -> Payment {
    let direct_has_lifecycle = direct.state.is_some() || direct.result.is_some();

    macro_rules! replace_if_some {
        ($field:ident) => {
            if direct.$field.is_some() {
                listed.$field = direct.$field;
            }
        };
    }

    replace_if_some!(id);
    replace_if_some!(payment_id);
    replace_if_some!(type_);
    replace_if_some!(amount);
    replace_if_some!(total_fee);
    replace_if_some!(total_amount);
    replace_if_some!(created);
    replace_if_some!(completed);
    replace_if_some!(correlation_id);
    replace_if_some!(description);
    replace_if_some!(p2p);

    if direct_has_lifecycle {
        listed.state = direct.state;
        listed.result = direct.result;
    }

    if let Some(direct_lightning) = direct.lightning {
        if let Some(listed_lightning) = listed.lightning.as_mut() {
            if direct_lightning.network_fee.is_some() {
                listed_lightning.network_fee = direct_lightning.network_fee;
            }
            if direct_lightning.payment_hash.is_some() {
                listed_lightning.payment_hash = direct_lightning.payment_hash;
            }
            if direct_lightning.payment_request.is_some() {
                listed_lightning.payment_request = direct_lightning.payment_request;
            }
            if direct_lightning.pre_image.is_some() {
                listed_lightning.pre_image = direct_lightning.pre_image;
            }
        } else {
            listed.lightning = Some(direct_lightning);
        }
    }

    if let Some(direct_onchain) = direct.onchain {
        if let Some(listed_onchain) = listed.onchain.as_mut() {
            if direct_onchain.txn_id.is_some() {
                listed_onchain.txn_id = direct_onchain.txn_id;
            }
        } else {
            listed.onchain = Some(direct_onchain);
        }
    }

    listed
}

fn parse_strike_payment_id(value: &str) -> Option<uuid::Uuid> {
    let id = uuid::Uuid::parse_str(value).ok()?;
    id.hyphenated()
        .to_string()
        .eq_ignore_ascii_case(value)
        .then_some(id)
}

fn payment_to_transaction(payment: &Payment) -> Transaction {
    let state = payment.state.as_deref().or(payment.result.as_deref());
    let txid = payment
        .onchain
        .as_ref()
        .and_then(|onchain| onchain.txn_id.clone())
        .filter(|value| !value.trim().is_empty());

    Transaction {
        type_: "outgoing".to_string(),
        invoice: payment
            .lightning
            .as_ref()
            .and_then(|lightning| lightning.payment_request.clone())
            .unwrap_or_default(),
        description: payment.description.clone().unwrap_or_default(),
        description_hash: String::new(),
        preimage: payment
            .lightning
            .as_ref()
            .and_then(|lightning| lightning.pre_image.clone())
            .unwrap_or_default(),
        payment_hash: payment
            .lightning
            .as_ref()
            .and_then(|lightning| lightning.payment_hash.clone())
            .unwrap_or_default(),
        amount_msats: amount_to_msats(payment.amount.as_ref()),
        fees_paid: payment
            .lightning
            .as_ref()
            .and_then(|lightning| lightning.network_fee.as_ref())
            .map(|fee| amount_to_msats(Some(fee)))
            .unwrap_or_default(),
        created_at: payment
            .created
            .as_deref()
            .and_then(|created| chrono::DateTime::parse_from_rfc3339(created).ok())
            .map(|created| created.timestamp())
            .unwrap_or_default(),
        expires_at: 0,
        settled_at: if normalize_settlement_state(state) == SettlementState::Completed {
            payment
                .completed
                .as_deref()
                .and_then(|completed| chrono::DateTime::parse_from_rfc3339(completed).ok())
                .map(|completed| completed.timestamp())
                .unwrap_or_default()
        } else {
            0
        },
        payer_note: Some(String::new()),
        external_id: normalized_payment_id(payment).map(str::to_string),
        settlement_type: Some(normalize_settlement_type(
            payment.type_.as_deref(),
            state,
            payment.lightning.is_some(),
            payment.onchain.is_some(),
            payment.p2p.is_some(),
            txid.as_deref(),
        )),
        settlement_state: Some(normalize_settlement_state(state)),
        txid,
    }
}

fn receive_to_transaction(receive: &StrikeReceive) -> Transaction {
    let txid = receive
        .onchain
        .as_ref()
        .and_then(|onchain| {
            onchain
                .transaction_id
                .clone()
                .or_else(|| onchain.transaction_hash.clone())
        })
        .filter(|value| !value.trim().is_empty());
    let lightning = receive.lightning.as_ref();

    Transaction {
        type_: "incoming".to_string(),
        invoice: lightning
            .map(|lightning| lightning.invoice.clone())
            .unwrap_or_default(),
        description: lightning
            .and_then(|lightning| {
                lightning
                    .description
                    .clone()
                    .or_else(|| lightning.description_hash.clone())
            })
            .unwrap_or_default(),
        description_hash: lightning
            .and_then(|lightning| lightning.description_hash.clone())
            .unwrap_or_default(),
        preimage: lightning
            .and_then(|lightning| lightning.preimage.clone())
            .unwrap_or_default(),
        payment_hash: lightning
            .map(|lightning| lightning.payment_hash.clone())
            .unwrap_or_default(),
        amount_msats: amount_to_msats(Some(&receive.amount_received)),
        fees_paid: 0,
        created_at: receive
            .created
            .as_deref()
            .and_then(|created| chrono::DateTime::parse_from_rfc3339(created).ok())
            .map(|created| created.timestamp())
            .unwrap_or_default(),
        expires_at: 0,
        settled_at: if normalize_settlement_state(receive.state.as_deref())
            == SettlementState::Completed
        {
            receive
                .completed
                .as_deref()
                .and_then(|completed| chrono::DateTime::parse_from_rfc3339(completed).ok())
                .map(|completed| completed.timestamp())
                .unwrap_or_default()
        } else {
            0
        },
        payer_note: Some(String::new()),
        external_id: receive
            .receive_id
            .clone()
            .or_else(|| receive.receive_request_id.clone()),
        settlement_type: Some(normalize_settlement_type(
            receive.type_.as_deref(),
            receive.state.as_deref(),
            receive.lightning.is_some(),
            receive.onchain.is_some(),
            receive.p2p.is_some(),
            txid.as_deref(),
        )),
        settlement_state: Some(normalize_settlement_state(receive.state.as_deref())),
        txid,
    }
}

fn is_retryable_payment_read_status(status: reqwest::StatusCode) -> bool {
    status == reqwest::StatusCode::NOT_FOUND || status.is_server_error()
}

fn settle_outcome(state: Option<&str>, preimage: Option<String>) -> Result<String, ApiError> {
    if matches!(preimage.as_deref(), Some(value) if !value.is_empty()) {
        return Ok(preimage.unwrap_or_default());
    }

    if matches!(state, Some(value) if value.eq_ignore_ascii_case("FAILED")) {
        return Err(ApiError::Nwc {
            code: "PAYMENT_FAILED".to_string(),
            message: "Strike payment failed".to_string(),
        });
    }

    Err(ApiError::Api {
        reason: "Strike payment outcome is indeterminate; reconcile it via lookup_invoice or list_transactions before retrying".to_string(),
    })
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
    let client = async_client(&config)?;

    // Get balance from Strike API
    let response = client
        .get(&format!("{}/balances", get_base_url(&config)))
        .send()
        .await
        .map_err(|e| strike_nwc_error_from_transport(e, "get_info"))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response.text().await.unwrap_or_default();
        return Err(strike_nwc_error_from_response(
            status,
            error_text,
            "get_info",
            "Failed to get balances",
        ));
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
    let client = async_client(&config)?;

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
                .map_err(|e| strike_nwc_error_from_transport(e, "make_invoice"))?;

            if !response.status().is_success() {
                let status = response.status();
                let error_text = response.text().await.unwrap_or_default();
                return Err(strike_nwc_error_from_response(
                    status,
                    error_text,
                    "make_invoice",
                    "Failed to create receive request",
                ));
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
                settlement_type: None,
                settlement_state: None,
                txid: None,
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
    let client = async_client(&config)?;

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
        .map_err(|e| strike_nwc_error_from_transport(e, "pay_invoice"))?;

    if !quote_response.status().is_success() {
        let status = quote_response.status();
        let error_text = quote_response.text().await.unwrap_or_default();
        return Err(strike_nwc_error_from_response(
            status,
            error_text,
            "pay_invoice",
            "Failed to create payment quote",
        ));
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
        .map_err(|e| strike_nwc_error_from_transport(e, "pay_invoice"))?;

    if !execute_response.status().is_success() {
        let status = execute_response.status();
        let error_text = execute_response.text().await.unwrap_or_default();
        return Err(strike_nwc_error_from_response(
            status,
            error_text,
            "pay_invoice",
            "Failed to execute payment",
        ));
    }

    let execute_text = execute_response.text().await.map_err(|e| ApiError::Http {
        reason: format!("Failed to read payment execution response: {}", e),
    })?;
    let execute_resp: PaymentExecutionResponse = serde_json::from_str(&execute_text)?;

    // Get the outgoing payment record. The Lightning proof can appear shortly after execution.
    let payment_id = execute_resp.payment_id.clone();
    let payment_url = format!("{}/payments/{}", get_base_url(&config), payment_id);
    let mut payment = Some(execute_resp);

    for attempt in 0..5 {
        let has_preimage = matches!(
            payment
                .as_ref()
                .and_then(|payment| payment.lightning.as_ref())
                .and_then(|lightning| lightning.pre_image.as_deref()),
            Some(preimage) if !preimage.is_empty()
        );
        if has_preimage {
            break;
        }

        if attempt > 0 {
            tokio::time::sleep(Duration::from_millis(400)).await;
        }

        let response = match client.get(&payment_url).send().await {
            Ok(response) => response,
            Err(_) => continue,
        };
        if !response.status().is_success() {
            if is_retryable_payment_read_status(response.status()) {
                continue;
            }
            break;
        }
        let payment_text = match response.text().await {
            Ok(payment_text) => payment_text,
            Err(_) => continue,
        };
        let parsed = match serde_json::from_str::<PaymentExecutionResponse>(&payment_text) {
            Ok(parsed) => parsed,
            Err(_) => continue,
        };
        payment = Some(parsed);
    }

    let fee_msats = payment
        .as_ref()
        .and_then(|payment| payment.lightning.as_ref())
        .and_then(|lightning| lightning.network_fee.as_ref())
        .or_else(|| {
            payment
                .as_ref()
                .and_then(|payment| payment.lightning_network_fee.as_ref())
        })
        .filter(|fee| fee.currency == "BTC")
        .and_then(|fee| fee.amount.parse::<f64>().ok())
        .map(|fee| (fee * 100_000_000_000.0) as i64)
        .unwrap_or(0);

    // Extract payment hash from the original BOLT11 invoice
    let invoice_payment_hash = match Bolt11Invoice::from_str(&invoice_params.invoice) {
        Ok(invoice) => {
            format!("{:x}", invoice.payment_hash())
        }
        Err(_) => "".to_string(), // If parsing fails, return empty string
    };
    let preimage = payment
        .as_ref()
        .and_then(|payment| payment.lightning.as_ref())
        .and_then(|lightning| lightning.pre_image.clone());
    let state = payment.as_ref().map(|payment| payment.state.as_str());
    let preimage = settle_outcome(state, preimage)?;

    Ok(PayInvoiceResponse {
        payment_hash: invoice_payment_hash,
        preimage,
        fee_msats,
    })
}

pub async fn prepare_onchain_transaction(
    config: StrikeConfig,
    params: PrepareOnchainTransactionParams,
) -> Result<OnchainTransaction, ApiError> {
    let client = async_client(&config)?;
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

    let client = async_client(&config)?;
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
    let client = async_client(&config)?;

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
        .map_err(|e| strike_nwc_error_from_transport(e, "lookup_invoice"))?;

    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Err(ApiError::Nwc {
            code: "NOT_FOUND".to_string(),
            message: "Receive not found".to_string(),
        });
    }

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response.text().await.unwrap_or_default();
        return Err(strike_nwc_error_from_response(
            status,
            error_text,
            "lookup_invoice",
            "Failed to get receives",
        ));
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

    receive.lightning.as_ref().ok_or_else(|| ApiError::Json {
        reason: "No lightning information in receive".to_string(),
    })?;

    Ok(receive_to_transaction(&receive))
}

pub async fn list_transactions(
    config: StrikeConfig,
    params: crate::ListTransactionsParams,
) -> Result<Vec<Transaction>, ApiError> {
    let client = async_client(&config)?;
    let from = params.from;
    let limit = params.limit;

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
        .map_err(|e| strike_nwc_error_from_transport(e, "list_transactions"))?;

    let mut transactions: HashMap<String, Transaction> = HashMap::new();
    let mut payments: HashMap<String, Payment> = HashMap::new();

    if receives_response.status().is_success() {
        let receives_text = receives_response.text().await.unwrap();
        let receives_resp: StrikeReceivesWithCountResponse = serde_json::from_str(&receives_text)
            .map_err(|e| ApiError::Json {
            reason: format!(
                "Failed to parse receives response: {} - Response: {}",
                e, receives_text
            ),
        })?;

        for (index, receive) in receives_resp.items.into_iter().enumerate() {
            let transaction = receive_to_transaction(&receive);
            let key = format!(
                "incoming:{}",
                transaction
                    .external_id
                    .clone()
                    .unwrap_or_else(|| format!("receive-{index}"))
            );
            transactions.insert(key, transaction);
        }
    } else {
        let status = receives_response.status();
        let error_text = receives_response.text().await.unwrap_or_default();
        return Err(strike_nwc_error_from_response(
            status,
            error_text,
            "list_transactions",
            "Failed to list receives",
        ));
    }

    // Get payments (outgoing)
    let payments_url = format!(
        "{}/payments?$skip={}&$top={}",
        get_base_url(&config),
        from,
        limit
    );
    let payments_response = client
        .get(&payments_url)
        .send()
        .await
        .map_err(|e| strike_nwc_error_from_transport(e, "list_transactions"))?;

    if payments_response.status().is_success() {
        let payments_text = payments_response.text().await.unwrap();
        let payments_resp: PaymentsResponse = serde_json::from_str(&payments_text)?;

        for (index, payment) in payments_resp.data.into_iter().enumerate() {
            let key = format!(
                "outgoing:{}",
                normalized_payment_id(&payment)
                    .map(str::to_string)
                    .unwrap_or_else(|| format!("payment-{index}"))
            );
            payments.insert(key, payment);
        }
    } else if payments_response.status() != reqwest::StatusCode::NOT_FOUND {
        let status = payments_response.status();
        let error_text = payments_response.text().await.unwrap_or_default();
        return Err(strike_nwc_error_from_response(
            status,
            error_text,
            "list_transactions",
            "Failed to list payments",
        ));
    }

    if let Some(search) = params.search.as_deref().filter(|s| !s.is_empty()) {
        if let Some(payment_id) = parse_strike_payment_id(search) {
            let payment_url = format!("{}/payments/{}", get_base_url(&config), payment_id);
            if let Ok(response) = client.get(&payment_url).send().await {
                if response.status().is_success() {
                    if let Ok(payment) = response.json::<Payment>().await {
                        let key = format!(
                            "outgoing:{}",
                            normalized_payment_id(&payment).unwrap_or(search)
                        );
                        let payment = match payments.remove(&key) {
                            Some(listed) => merge_payment_snapshots(listed, payment),
                            None => payment,
                        };
                        payments.insert(key, payment);
                    }
                }
            }
        }
    }

    for (key, payment) in payments {
        transactions.insert(key, payment_to_transaction(&payment));
    }

    let mut transactions: Vec<Transaction> = transactions
        .into_values()
        .filter(|transaction| {
            params
                .payment_hash
                .as_deref()
                .map(|payment_hash| transaction.payment_hash == payment_hash)
                .unwrap_or(true)
                && params
                    .search
                    .as_deref()
                    .map(|search| crate::transaction_matches_search(transaction, search))
                    .unwrap_or(true)
                && params
                    .created_after
                    .map(|created_after| transaction.created_at >= created_after)
                    .unwrap_or(true)
                && params
                    .created_before
                    .map(|created_before| transaction.created_at <= created_before)
                    .unwrap_or(true)
        })
        .collect();

    transactions.sort_by(|a, b| {
        b.created_at
            .cmp(&a.created_at)
            .then_with(|| a.type_.cmp(&b.type_))
            .then_with(|| {
                a.external_id
                    .as_deref()
                    .unwrap_or_default()
                    .cmp(b.external_id.as_deref().unwrap_or_default())
            })
            .then_with(|| a.payment_hash.cmp(&b.payment_hash))
            .then_with(|| {
                a.txid
                    .as_deref()
                    .unwrap_or_default()
                    .cmp(b.txid.as_deref().unwrap_or_default())
            })
            .then_with(|| a.invoice.cmp(&b.invoice))
    });
    if limit > 0 {
        transactions.truncate(limit as usize);
    }

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
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;
    use tokio::sync::mpsc;

    const PAYMENT_ID: &str = "11111111-1111-4111-8111-111111111111";

    fn payment(value: serde_json::Value) -> Payment {
        serde_json::from_value(value).expect("payment should deserialize")
    }

    fn receive(value: serde_json::Value) -> StrikeReceive {
        serde_json::from_value(value).expect("receive should deserialize")
    }

    async fn test_server(
        responses: Vec<(u16, serde_json::Value)>,
    ) -> (String, mpsc::Receiver<String>) {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("test server should bind");
        let address = listener.local_addr().expect("test server address");
        let (sender, receiver) = mpsc::channel(responses.len());

        tokio::spawn(async move {
            for (status, response_body) in responses {
                let (mut stream, _) = listener.accept().await.expect("request should connect");
                let mut request = Vec::new();
                let mut buffer = [0_u8; 4096];
                loop {
                    let read = stream.read(&mut buffer).await.expect("request should read");
                    if read == 0 {
                        break;
                    }
                    request.extend_from_slice(&buffer[..read]);
                    if request.windows(4).any(|window| window == b"\r\n\r\n") {
                        break;
                    }
                }
                let request = String::from_utf8_lossy(&request).into_owned();
                sender
                    .send(request)
                    .await
                    .expect("request should be recorded");

                let body = response_body.to_string();
                let reason = if status == 200 { "OK" } else { "Not Found" };
                let response = format!(
                    "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                stream
                    .write_all(response.as_bytes())
                    .await
                    .expect("response should write");
            }
        });

        (format!("http://{address}"), receiver)
    }

    fn test_config(base_url: String) -> StrikeConfig {
        StrikeConfig {
            base_url: Some(base_url),
            api_key: "test-token".to_string(),
            socks5_proxy: None,
            accept_invalid_certs: Some(false),
            http_timeout: Some(5),
        }
    }

    fn list_params(search: Option<&str>) -> crate::ListTransactionsParams {
        crate::ListTransactionsParams {
            from: 0,
            limit: 10,
            payment_hash: None,
            search: search.map(str::to_string),
            created_after: None,
            created_before: None,
        }
    }

    #[test]
    fn proxy_client_builds_with_certificate_verification_enabled() {
        let config = StrikeConfig {
            api_key: "fake-api-key".to_string(),
            socks5_proxy: Some("socks5h://127.0.0.1:9150".to_string()),
            ..Default::default()
        };

        let _client = async_client(&config).expect("proxy client should build");
    }

    #[test]
    fn invalid_proxy_configuration_fails_closed() {
        let config = StrikeConfig {
            api_key: "fake-api-key".to_string(),
            socks5_proxy: Some("://invalid".to_string()),
            ..Default::default()
        };

        assert!(async_client(&config).is_err());
    }

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
    fn maps_strike_balance_too_low_to_insufficient_balance() {
        let error = strike_nwc_error_from_response(
            reqwest::StatusCode::UNPROCESSABLE_ENTITY,
            r#"{"data":{"status":422,"code":"BALANCE_TOO_LOW","message":"Balance is too low"}}"#
                .to_string(),
            "pay_invoice",
            "Failed to create payment quote",
        );

        match error {
            ApiError::Nwc { code, message } => {
                assert_eq!(code, "INSUFFICIENT_BALANCE");
                assert_eq!(message, "Balance is too low");
            }
            other => panic!("expected structured NWC error, got {:?}", other),
        }
    }

    #[test]
    fn maps_strike_invalid_invoice_to_payment_failed() {
        let error = strike_nwc_error_from_response(
            reqwest::StatusCode::UNPROCESSABLE_ENTITY,
            r#"{"data":{"code":"INVALID_LN_INVOICE","message":"Invalid lightning invoice"}}"#
                .to_string(),
            "pay_invoice",
            "Failed to create payment quote",
        );

        match error {
            ApiError::Nwc { code, message } => {
                assert_eq!(code, "PAYMENT_FAILED");
                assert_eq!(message, "Invalid lightning invoice");
            }
            other => panic!("expected structured NWC error, got {:?}", other),
        }
    }

    #[test]
    fn maps_unstructured_unauthorized_status_to_unauthorized() {
        let error = strike_nwc_error_from_response(
            reqwest::StatusCode::UNAUTHORIZED,
            "unauthorized".to_string(),
            "get_info",
            "Failed to get balances",
        );

        match error {
            ApiError::Nwc { code, message } => {
                assert_eq!(code, "UNAUTHORIZED");
                assert_eq!(message, "unauthorized");
            }
            other => panic!("expected structured NWC error, got {:?}", other),
        }
    }

    #[test]
    fn classifies_retryable_payment_read_statuses() {
        assert!(is_retryable_payment_read_status(
            reqwest::StatusCode::NOT_FOUND
        ));
        assert!(is_retryable_payment_read_status(
            reqwest::StatusCode::INTERNAL_SERVER_ERROR
        ));
        assert!(!is_retryable_payment_read_status(
            reqwest::StatusCode::UNAUTHORIZED
        ));
    }

    #[test]
    fn settles_payment_when_preimage_is_present() {
        assert_eq!(
            settle_outcome(Some("PENDING"), Some("fake-preimage".to_string())).unwrap(),
            "fake-preimage"
        );
    }

    #[test]
    fn rejects_failed_payment_without_preimage() {
        let error = settle_outcome(Some("failed"), None).unwrap_err();
        assert!(matches!(
            error,
            ApiError::Nwc { code, message }
                if code == "PAYMENT_FAILED" && message.contains("failed")
        ));
    }

    #[test]
    fn rejects_indeterminate_payment_without_preimage() {
        let error = settle_outcome(Some("PENDING"), Some(String::new())).unwrap_err();
        assert!(matches!(error, ApiError::Api { reason } if reason.contains("indeterminate")));
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

    #[test]
    fn maps_outgoing_lifecycle_and_route_evidence_independently() {
        let cases = [
            (
                serde_json::json!({ "id": "methodless-pending", "state": "PENDING" }),
                SettlementType::Unknown,
                SettlementState::Pending,
                None,
            ),
            (
                serde_json::json!({ "id": "methodless-completed", "state": "SUCCESS" }),
                SettlementType::Intraledger,
                SettlementState::Completed,
                None,
            ),
            (
                serde_json::json!({ "id": "p2p", "type": "P2P", "state": "PENDING", "p2p": {} }),
                SettlementType::Intraledger,
                SettlementState::Pending,
                None,
            ),
            (
                serde_json::json!({ "id": "direct", "type": "ONCHAIN", "state": "COMPLETED" }),
                SettlementType::Intraledger,
                SettlementState::Completed,
                None,
            ),
            (
                serde_json::json!({ "id": "waiting", "type": "ONCHAIN", "state": "PENDING", "onchain": {} }),
                SettlementType::Onchain,
                SettlementState::Pending,
                None,
            ),
            (
                serde_json::json!({ "id": "broadcast", "state": "PENDING", "onchain": { "txnId": "pending-txid" } }),
                SettlementType::Onchain,
                SettlementState::Pending,
                Some("pending-txid"),
            ),
            (
                serde_json::json!({ "id": "confirmed", "state": "COMPLETED", "onchain": { "txnId": "completed-txid" } }),
                SettlementType::Onchain,
                SettlementState::Completed,
                Some("completed-txid"),
            ),
            (
                serde_json::json!({ "id": "failed", "type": "ONCHAIN", "state": "FAILURE" }),
                SettlementType::Onchain,
                SettlementState::Failed,
                None,
            ),
            (
                serde_json::json!({ "id": "unknown", "state": "SOMETHING_NEW" }),
                SettlementType::Unknown,
                SettlementState::Unknown,
                None,
            ),
            (
                serde_json::json!({ "id": "lightning", "state": "COMPLETED", "lightning": { "paymentHash": "hash" } }),
                SettlementType::Lightning,
                SettlementState::Completed,
                None,
            ),
        ];

        for (value, expected_type, expected_state, expected_txid) in cases {
            let transaction = payment_to_transaction(&payment(value));
            assert_eq!(transaction.settlement_type, Some(expected_type));
            assert_eq!(transaction.settlement_state, Some(expected_state));
            assert_eq!(transaction.txid.as_deref(), expected_txid);
        }
    }

    #[test]
    fn p2p_lifecycle_updates_preserve_provider_id_and_route() {
        let pending = payment_to_transaction(&payment(serde_json::json!({
            "paymentId": PAYMENT_ID, "type": "P2P", "state": "PENDING", "p2p": {}
        })));
        let completed = payment_to_transaction(&payment(serde_json::json!({
            "paymentId": PAYMENT_ID, "type": "P2P", "state": "COMPLETED", "p2p": {}
        })));

        assert_eq!(pending.external_id, completed.external_id);
        assert_eq!(pending.settlement_type, Some(SettlementType::Intraledger));
        assert_eq!(pending.settlement_state, Some(SettlementState::Pending));
        assert_eq!(completed.settlement_type, Some(SettlementType::Intraledger));
        assert_eq!(completed.settlement_state, Some(SettlementState::Completed));
        assert!(pending.txid.is_none() && completed.txid.is_none());
    }

    #[test]
    fn retains_incoming_p2p_and_onchain_receives() {
        let p2p = receive_to_transaction(&receive(serde_json::json!({
            "receiveId": "receive-p2p",
            "receiveRequestId": "request-p2p",
            "type": "P2P",
            "state": "PENDING",
            "amountReceived": { "amount": "0.00000001", "currency": "BTC" },
            "p2p": { "payerAccountId": "payer" }
        })));
        let intraledger = receive_to_transaction(&receive(serde_json::json!({
            "receiveId": "receive-direct",
            "receiveRequestId": "request-direct",
            "type": "ONCHAIN",
            "state": "COMPLETED",
            "amountReceived": { "amount": "0.00000001", "currency": "BTC" },
            "onchain": { "address": "bc1q" }
        })));
        let onchain = receive_to_transaction(&receive(serde_json::json!({
            "receiveId": "receive-chain",
            "receiveRequestId": "request-chain",
            "type": "ONCHAIN",
            "state": "COMPLETED",
            "amountReceived": { "amount": "0.00000001", "currency": "BTC" },
            "onchain": { "address": "bc1q", "transactionId": "receive-txid" }
        })));

        assert_eq!(p2p.external_id.as_deref(), Some("receive-p2p"));
        assert_eq!(p2p.settlement_type, Some(SettlementType::Intraledger));
        assert_eq!(p2p.settlement_state, Some(SettlementState::Pending));
        assert_eq!(
            intraledger.settlement_type,
            Some(SettlementType::Intraledger)
        );
        assert_eq!(
            intraledger.settlement_state,
            Some(SettlementState::Completed)
        );
        assert!(intraledger.txid.is_none());
        assert_eq!(onchain.settlement_type, Some(SettlementType::Onchain));
        assert_eq!(onchain.txid.as_deref(), Some("receive-txid"));
    }

    #[tokio::test]
    async fn uuid_search_returns_direct_payment_outside_collection_page() {
        let (base_url, mut requests) = test_server(vec![
            (200, serde_json::json!({ "items": [], "count": 0 })),
            (200, serde_json::json!({ "data": [], "count": 0 })),
            (
                200,
                serde_json::json!({
                    "paymentId": PAYMENT_ID,
                    "state": "COMPLETED",
                    "amount": { "amount": "0.00000001", "currency": "BTC" }
                }),
            ),
        ])
        .await;

        let transactions = list_transactions(test_config(base_url), list_params(Some(PAYMENT_ID)))
            .await
            .expect("list transactions should succeed");
        assert_eq!(transactions.len(), 1);
        assert_eq!(transactions[0].external_id.as_deref(), Some(PAYMENT_ID));
        assert_eq!(
            transactions[0].settlement_type,
            Some(SettlementType::Intraledger)
        );

        let request_paths: Vec<String> = [
            requests.recv().await,
            requests.recv().await,
            requests.recv().await,
        ]
        .into_iter()
        .flatten()
        .filter_map(|request| request.lines().next().map(str::to_string))
        .collect();
        assert!(request_paths[2].contains(&format!("/payments/{PAYMENT_ID}")));
    }

    #[tokio::test]
    async fn direct_snapshot_preserves_omitted_fields_from_listed_copy() {
        let listed = serde_json::json!({
            "id": PAYMENT_ID,
            "type": "ONCHAIN",
            "state": "PENDING",
            "created": "2026-01-01T00:00:00Z",
            "description": "listed description",
            "amount": { "amount": "0.00000001", "currency": "BTC" },
            "onchain": { "txnId": "listed-txid" }
        });
        let (base_url, _requests) = test_server(vec![
            (200, serde_json::json!({ "items": [], "count": 0 })),
            (200, serde_json::json!({ "data": [listed], "count": 1 })),
            (
                200,
                serde_json::json!({
                    "paymentId": PAYMENT_ID,
                    "state": "COMPLETED",
                    "completed": "2026-01-01T00:01:00Z"
                }),
            ),
        ])
        .await;

        let transactions = list_transactions(test_config(base_url), list_params(Some(PAYMENT_ID)))
            .await
            .expect("list transactions should succeed");
        assert_eq!(transactions.len(), 1);
        assert_eq!(transactions[0].amount_msats, 1_000);
        assert_eq!(transactions[0].created_at, 1_767_225_600);
        assert_eq!(transactions[0].settled_at, 1_767_225_660);
        assert_eq!(transactions[0].description, "listed description");
        assert_eq!(transactions[0].txid.as_deref(), Some("listed-txid"));
        assert_eq!(
            transactions[0].settlement_type,
            Some(SettlementType::Onchain)
        );
        assert_eq!(
            transactions[0].settlement_state,
            Some(SettlementState::Completed)
        );
    }

    #[tokio::test]
    async fn transaction_limit_uses_deterministic_identifier_tie_breaker() {
        let payments = serde_json::json!([
            {
                "id": "payment-z",
                "state": "PENDING",
                "created": "2026-01-01T00:00:00Z",
                "amount": { "amount": "0.00000001", "currency": "BTC" }
            },
            {
                "id": "payment-a",
                "state": "PENDING",
                "created": "2026-01-01T00:00:00Z",
                "amount": { "amount": "0.00000001", "currency": "BTC" }
            }
        ]);
        let (base_url, _requests) = test_server(vec![
            (200, serde_json::json!({ "items": [], "count": 0 })),
            (200, serde_json::json!({ "data": payments, "count": 2 })),
        ])
        .await;
        let mut params = list_params(None);
        params.limit = 1;

        let transactions = list_transactions(test_config(base_url), params)
            .await
            .expect("list transactions should succeed");

        assert_eq!(transactions.len(), 1);
        assert_eq!(transactions[0].external_id.as_deref(), Some("payment-a"));
    }

    #[tokio::test]
    async fn direct_404_falls_back_and_non_uuid_search_skips_direct_lookup() {
        let listed = serde_json::json!({
            "id": PAYMENT_ID,
            "state": "PENDING",
            "created": "2026-01-01T00:00:00Z",
            "description": "reconciliation target",
            "amount": { "amount": "0.00000001", "currency": "BTC" }
        });
        let (base_url, _requests) = test_server(vec![
            (200, serde_json::json!({ "items": [], "count": 0 })),
            (
                200,
                serde_json::json!({ "data": [listed.clone()], "count": 1 }),
            ),
            (404, serde_json::json!({ "message": "not found" })),
        ])
        .await;
        let fallback = list_transactions(test_config(base_url), list_params(Some(PAYMENT_ID)))
            .await
            .expect("404 should fall back");
        assert_eq!(fallback.len(), 1);

        let (base_url, mut requests) = test_server(vec![
            (200, serde_json::json!({ "items": [], "count": 0 })),
            (200, serde_json::json!({ "data": [listed], "count": 1 })),
        ])
        .await;
        let text_search = list_transactions(test_config(base_url), list_params(Some("TARGET")))
            .await
            .expect("text search should succeed");
        assert_eq!(text_search.len(), 1);
        assert!(requests.recv().await.is_some());
        assert!(requests.recv().await.is_some());
        assert!(requests.recv().await.is_none());
    }
}
