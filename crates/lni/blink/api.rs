use std::str::FromStr;
use std::time::Duration;

use lightning_invoice::Bolt11Invoice;

use super::types::*;
use super::BlinkConfig;
use crate::error_normalization::{
    map_provider_message, nwc_error, provider_error_from_response, transport_error,
    ProviderErrorInfo,
};
use crate::types::NodeInfo;
use crate::{
    ApiError, CreateInvoiceParams, InvoiceType, Offer, OnInvoiceEventCallback,
    OnInvoiceEventParams, OnchainFeePayer, OnchainFeePreference, OnchainFeePreferenceType,
    OnchainFeeSpeed, OnchainTransaction, PayInvoiceParams, PayInvoiceResponse, PayOnchainOptions,
    PayOnchainResponse, PrepareOnchainTransactionParams, Transaction,
};
use reqwest::header;

// Docs: https://dev.blink.sv/

fn map_blink_provider_error(info: &ProviderErrorInfo) -> Option<&'static str> {
    let code = info.code.as_deref().unwrap_or_default().to_uppercase();
    if code.contains("AUTHENTICATION") || code.contains("UNAUTHORIZED") {
        return Some("UNAUTHORIZED");
    }
    if code.contains("FORBIDDEN") || code.contains("PERMISSION") || code.contains("SCOPE") {
        return Some("RESTRICTED");
    }
    if code.contains("INSUFFICIENT") || code.contains("BALANCE") {
        return Some("INSUFFICIENT_BALANCE");
    }
    if code.contains("LIMIT") || code.contains("QUOTA") {
        return Some("QUOTA_EXCEEDED");
    }
    if code.contains("INVALID_INVOICE") || code.contains("NO_ROUTE") || code.contains("PAYMENT") {
        return Some("PAYMENT_FAILED");
    }

    map_provider_message(info.message.as_deref())
}

fn blink_graphql_error(errors: &[GraphQLError], fallback_code: &str) -> ApiError {
    let first = errors.first();
    let info = ProviderErrorInfo {
        code: first.and_then(|error| error.code.clone()),
        status: None,
        message: Some(
            errors
                .iter()
                .map(|error| error.message.clone())
                .collect::<Vec<_>>()
                .join(", "),
        ),
    };
    let code = map_blink_provider_error(&info).unwrap_or(fallback_code);
    nwc_error(
        code,
        info.message
            .unwrap_or_else(|| "Blink GraphQL error".to_string()),
    )
}

fn client(config: &BlinkConfig) -> reqwest::Client {
    let mut headers = reqwest::header::HeaderMap::new();

    match header::HeaderValue::from_str(&config.api_key) {
        Ok(api_key_header) => headers.insert("X-API-KEY", api_key_header),
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
    client_builder
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
}

async fn execute_graphql_query<T>(
    config: &BlinkConfig,
    query: &str,
    variables: Option<serde_json::Value>,
    operation: &str,
) -> Result<T, ApiError>
where
    T: for<'de> serde::Deserialize<'de>,
{
    let client = client(config);
    let request = GraphQLRequest {
        query: query.to_string(),
        variables,
    };

    let response = client
        .post(
            config
                .base_url
                .as_deref()
                .unwrap_or("https://api.blink.sv/graphql"),
        )
        .json(&request)
        .send()
        .await
        .map_err(|e| transport_error("blink", operation, e))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response.text().await.unwrap_or_default();
        return Err(provider_error_from_response(
            "blink",
            operation,
            status,
            error_text,
            map_blink_provider_error,
        ));
    }

    let response_text = response.text().await.map_err(|e| ApiError::Http {
        reason: format!("Failed to read GraphQL response: {}", e),
    })?;
    let graphql_response: GraphQLResponse<T> =
        serde_json::from_str(&response_text).map_err(|e| ApiError::Json {
            reason: format!(
                "Failed to parse GraphQL response: {} - Response: {}",
                e, response_text
            ),
        })?;

    if let Some(errors) = graphql_response.errors {
        return Err(blink_graphql_error(&errors, "OTHER"));
    }

    graphql_response
        .data
        .ok_or_else(|| nwc_error("INTERNAL", "No data in GraphQL response"))
}

async fn get_btc_wallet_id(config: &BlinkConfig) -> Result<String, ApiError> {
    let query = r#"
        query Me {
            me {
                defaultAccount {
                    wallets {
                        id
                        walletCurrency
                        balance
                    }
                }
            }
        }
    "#;

    let response: MeQuery = execute_graphql_query(config, query, None, "get_info").await?;

    let btc_wallet = response
        .me
        .default_account
        .wallets
        .into_iter()
        .find(|w| w.wallet_currency == "BTC")
        .ok_or_else(|| nwc_error("OTHER", "No BTC wallet found in account"))?;

    Ok(btc_wallet.id)
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
        preference_type: OnchainFeePreferenceType::Default,
        speed: None,
        target_conf: None,
        sats_per_vbyte: None,
        backend: None,
    }
}

fn resolve_blink_fee_speed(fee: &OnchainFeePreference) -> Result<&'static str, ApiError> {
    match fee.preference_type {
        OnchainFeePreferenceType::Default => Ok("FAST"),
        OnchainFeePreferenceType::Speed => match fee.speed.clone().unwrap_or(OnchainFeeSpeed::Fast)
        {
            OnchainFeeSpeed::Fast => Ok("FAST"),
            OnchainFeeSpeed::Normal => Ok("MEDIUM"),
            OnchainFeeSpeed::Slow => Ok("SLOW"),
            OnchainFeeSpeed::Free => Err(ApiError::InvalidInput(
                "Blink pay_onchain does not support free on-chain fee speed".to_string(),
            )),
        },
        OnchainFeePreferenceType::Backend
        | OnchainFeePreferenceType::TargetConf
        | OnchainFeePreferenceType::SatsPerVbyte => Err(ApiError::InvalidInput(format!(
            "Blink pay_onchain does not support {:?} fee preferences",
            fee.preference_type
        ))),
    }
}

fn resolve_blink_fee_payer(
    fee_payer: Option<OnchainFeePayer>,
) -> Result<OnchainFeePayer, ApiError> {
    match fee_payer.unwrap_or(OnchainFeePayer::Sender) {
        OnchainFeePayer::Sender => Ok(OnchainFeePayer::Sender),
        OnchainFeePayer::Recipient => Err(ApiError::InvalidInput(
            "Blink pay_onchain only supports sender-paid on-chain fees".to_string(),
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

fn normalize_onchain_state(status: &str) -> String {
    match status.to_uppercase().as_str() {
        "PENDING" => "pending".to_string(),
        "SUCCESS" | "ALREADY_PAID" => "completed".to_string(),
        "FAILED" | "FAILURE" => "failed".to_string(),
        state => state.to_lowercase(),
    }
}

fn blink_transaction_amount_to_sats(amount: Option<i64>, currency: Option<&String>) -> Option<i64> {
    if currency.map(|currency| currency == "BTC").unwrap_or(false) {
        amount.map(|amount| amount.abs())
    } else {
        None
    }
}

fn blink_transaction_memo(transaction: &OnchainTransaction) -> Option<String> {
    let raw = transaction.raw.as_deref()?;
    let value: serde_json::Value = serde_json::from_str(raw).ok()?;
    value
        .get("memo")
        .and_then(|memo| memo.as_str())
        .filter(|memo| !memo.is_empty())
        .map(|memo| memo.to_string())
}

pub async fn get_info(config: &BlinkConfig) -> Result<NodeInfo, ApiError> {
    let query = r#"
        query Me {
            me {
                defaultAccount {
                    wallets {
                        id
                        walletCurrency
                        balance
                    }
                }
            }
        }
    "#;

    let response: MeQuery = execute_graphql_query(config, query, None, "get_info").await?;

    let btc_wallet = response
        .me
        .default_account
        .wallets
        .iter()
        .find(|w| w.wallet_currency == "BTC");

    let balance_sats = btc_wallet.map(|w| w.balance).unwrap_or(0);
    let balance_msats = balance_sats * 1000;

    Ok(NodeInfo {
        alias: "Blink Node".to_string(),
        color: "".to_string(),
        pubkey: "".to_string(),
        network: "mainnet".to_string(),
        block_height: 0,
        block_hash: "".to_string(),
        send_balance_msat: balance_msats,
        receive_balance_msat: balance_msats,
        fee_credit_balance_msat: 0,
        unsettled_send_balance_msat: 0,
        unsettled_receive_balance_msat: 0,
        pending_open_send_balance: 0,
        pending_open_receive_balance: 0,
    })
}

pub async fn create_invoice(
    config: &BlinkConfig,
    invoice_params: CreateInvoiceParams,
) -> Result<Transaction, ApiError> {
    match invoice_params.get_invoice_type() {
        InvoiceType::Bolt11 => {
            let wallet_id = get_btc_wallet_id(config).await?;

            let amount_sats = invoice_params.amount_msats.unwrap_or(0) / 1000;

            let query = r#"
                mutation LnInvoiceCreate($input: LnInvoiceCreateInput!) {
                    lnInvoiceCreate(input: $input) {
                        invoice {
                            paymentRequest
                            paymentHash
                            paymentSecret
                            satoshis
                        }
                        errors {
                            message
                        }
                    }
                }
            "#;

            let variables = serde_json::json!({
                "input": {
                    "amount": amount_sats.to_string(),
                    "walletId": wallet_id,
                    "memo": invoice_params.description
                }
            });

            let response: LnInvoiceCreateResponse =
                execute_graphql_query(config, query, Some(variables), "make_invoice").await?;

            if let Some(errors) = &response.ln_invoice_create.errors {
                if !errors.is_empty() {
                    return Err(blink_graphql_error(errors, "OTHER"));
                }
            }

            let invoice = response
                .ln_invoice_create
                .invoice
                .ok_or_else(|| nwc_error("INTERNAL", "No invoice data in response"))?;

            // Parse the BOLT11 invoice to get expiry
            let expires_at = match Bolt11Invoice::from_str(&invoice.payment_request) {
                Ok(bolt11) => bolt11
                    .expires_at()
                    .map(|duration| duration.as_secs() as i64)
                    .unwrap_or(0),
                Err(_) => 0,
            };

            Ok(Transaction {
                type_: "incoming".to_string(),
                invoice: invoice.payment_request,
                preimage: "".to_string(),
                payment_hash: invoice.payment_hash,
                amount_msats: invoice.satoshis * 1000,
                fees_paid: 0,
                created_at: chrono::Utc::now().timestamp(),
                expires_at,
                settled_at: 0,
                description: invoice_params.description.unwrap_or_default(),
                description_hash: invoice_params.description_hash.unwrap_or_default(),
                payer_note: Some("".to_string()),
                external_id: Some("".to_string()),
            })
        }
        InvoiceType::Bolt12 => Err(nwc_error(
            "NOT_IMPLEMENTED",
            "Bolt12 not implemented for Blink",
        )),
    }
}

pub async fn pay_invoice(
    config: &BlinkConfig,
    invoice_params: PayInvoiceParams,
) -> Result<PayInvoiceResponse, ApiError> {
    let wallet_id = get_btc_wallet_id(config).await?;

    // First probe the fee
    let fee_probe_query = r#"
        mutation lnInvoiceFeeProbe($input: LnInvoiceFeeProbeInput!) {
            lnInvoiceFeeProbe(input: $input) {
                errors {
                    message
                }
                amount
            }
        }
    "#;

    let fee_probe_variables = serde_json::json!({
        "input": {
            "paymentRequest": invoice_params.invoice,
            "walletId": wallet_id
        }
    });

    let fee_response: LnInvoiceFeeProbeResponse = execute_graphql_query(
        config,
        fee_probe_query,
        Some(fee_probe_variables),
        "pay_invoice",
    )
    .await?;

    let fee_msats = if let Some(errors) = &fee_response.ln_invoice_fee_probe.errors {
        if !errors.is_empty() {
            return Err(blink_graphql_error(errors, "PAYMENT_FAILED"));
        } else {
            fee_response.ln_invoice_fee_probe.amount.unwrap_or(0) * 1000
        }
    } else {
        fee_response.ln_invoice_fee_probe.amount.unwrap_or(0) * 1000
    };

    // Now send the payment
    let payment_query = r#"
        mutation LnInvoicePaymentSend($input: LnInvoicePaymentInput!) {
            lnInvoicePaymentSend(input: $input) {
                status
                errors {
                    message
                    path
                    code
                }
                transaction {
                    settlementVia {
                        __typename
                        ... on SettlementViaLn {
                            preImage
                        }
                        ... on SettlementViaIntraLedger {
                            preImage
                        }
                    }
                }
            }
        }
    "#;

    let payment_variables = serde_json::json!({
        "input": {
            "paymentRequest": invoice_params.invoice,
            "walletId": wallet_id
        }
    });

    let payment_response: LnInvoicePaymentSendResponse = execute_graphql_query(
        config,
        payment_query,
        Some(payment_variables),
        "pay_invoice",
    )
    .await?;

    if let Some(errors) = &payment_response.ln_invoice_payment_send.errors {
        if !errors.is_empty() {
            return Err(blink_graphql_error(errors, "PAYMENT_FAILED"));
        }
    }

    if payment_response.ln_invoice_payment_send.status != "SUCCESS" {
        let status = payment_response.ln_invoice_payment_send.status;
        return Err(nwc_error(
            if status == "FAILED" {
                "PAYMENT_FAILED"
            } else {
                "OTHER"
            },
            format!("Payment failed with status: {}", status),
        ));
    }

    // Extract payment hash from the BOLT11 invoice
    let payment_hash = match Bolt11Invoice::from_str(&invoice_params.invoice) {
        Ok(invoice) => format!("{:x}", invoice.payment_hash()),
        Err(_) => "".to_string(),
    };
    let preimage = match payment_response
        .ln_invoice_payment_send
        .transaction
        .and_then(|transaction| transaction.settlement_via)
    {
        Some(SettlementVia::SettlementViaLn { pre_image })
        | Some(SettlementVia::SettlementViaIntraLedger { pre_image }) => {
            pre_image.unwrap_or_default()
        }
        _ => "".to_string(),
    };

    Ok(PayInvoiceResponse {
        payment_hash,
        preimage,
        fee_msats,
    })
}

pub async fn prepare_onchain_transaction(
    config: &BlinkConfig,
    params: PrepareOnchainTransactionParams,
) -> Result<OnchainTransaction, ApiError> {
    assert_valid_onchain_amount(params.amount_sats)?;

    let fee = params.fee.clone().unwrap_or_else(default_onchain_fee);
    let fee_payer = resolve_blink_fee_payer(params.fee_payer.clone())?;
    let speed = resolve_blink_fee_speed(&fee)?;
    let wallet_id = get_btc_wallet_id(config).await?;

    let query = r#"
        query onChainTxFee($walletId: WalletId!, $address: OnChainAddress!, $amount: SatAmount!, $speed: PayoutSpeed!) {
            onChainTxFee(walletId: $walletId, address: $address, amount: $amount, speed: $speed) {
                amount
            }
        }
    "#;

    let variables = serde_json::json!({
        "walletId": wallet_id,
        "address": params.address.clone(),
        "amount": params.amount_sats,
        "speed": speed,
    });

    let response: OnChainTxFeeResponse =
        execute_graphql_query(config, query, Some(variables), "pay_invoice").await?;
    let fee_sats = response.on_chain_tx_fee.amount;

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
        estimated_delivery_seconds: None,
        raw: Some(
            serde_json::json!({
                "speed": speed,
                "memo": params.description,
            })
            .to_string(),
        ),
    })
}

pub async fn pay_onchain(
    config: &BlinkConfig,
    transaction: OnchainTransaction,
) -> Result<PayOnchainResponse, ApiError> {
    pay_onchain_with_options(config, transaction, PayOnchainOptions::default()).await
}

pub async fn pay_onchain_with_options(
    config: &BlinkConfig,
    transaction: OnchainTransaction,
    options: PayOnchainOptions,
) -> Result<PayOnchainResponse, ApiError> {
    assert_valid_onchain_amount(transaction.amount_sats)?;
    let _fee_payer = resolve_blink_fee_payer(Some(transaction.fee_payer.clone()))?;
    let speed = resolve_blink_fee_speed(&transaction.fee)?;
    assert_onchain_fee_guardrail(&transaction, options)?;

    let wallet_id = get_btc_wallet_id(config).await?;
    let memo = blink_transaction_memo(&transaction);
    let query = r#"
        mutation onChainPaymentSend($input: OnChainPaymentSendInput!) {
            onChainPaymentSend(input: $input) {
                status
                transaction {
                    id
                    settlementAmount
                    settlementCurrency
                    settlementFee
                    settlementVia {
                        __typename
                        ... on SettlementViaOnChain {
                            transactionHash
                        }
                    }
                }
                errors {
                    message
                }
            }
        }
    "#;

    let variables = serde_json::json!({
        "input": {
            "address": transaction.address.clone(),
            "amount": transaction.amount_sats,
            "walletId": wallet_id,
            "memo": memo,
            "speed": speed,
        }
    });

    let response: OnChainPaymentSendResponse =
        execute_graphql_query(config, query, Some(variables), "pay_invoice").await?;
    let payment = response.on_chain_payment_send;

    if let Some(errors) = &payment.errors {
        if !errors.is_empty() {
            return Err(blink_graphql_error(errors, "PAYMENT_FAILED"));
        }
    }

    let payment_transaction = payment.transaction;
    let payment_id = payment_transaction.as_ref().and_then(|tx| tx.id.clone());
    let txid = payment_transaction
        .as_ref()
        .and_then(|tx| match tx.settlement_via.as_ref() {
            Some(SettlementVia::SettlementViaOnChain { transaction_hash }) => {
                transaction_hash.clone()
            }
            _ => None,
        });
    let fee_sats = payment_transaction
        .as_ref()
        .and_then(|tx| {
            blink_transaction_amount_to_sats(tx.settlement_fee, tx.settlement_currency.as_ref())
        })
        .or(transaction.fee_sats);
    let amount_sats = payment_transaction
        .as_ref()
        .and_then(|tx| {
            blink_transaction_amount_to_sats(tx.settlement_amount, tx.settlement_currency.as_ref())
        })
        .unwrap_or(transaction.amount_sats);

    Ok(PayOnchainResponse {
        payment_id,
        txid,
        state: normalize_onchain_state(&payment.status),
        address: transaction.address,
        amount_sats,
        fee_sats,
        total_amount_sats: fee_sats
            .map(|fee_sats| transaction.amount_sats + fee_sats)
            .or(transaction.total_amount_sats),
        recipient_amount_sats: transaction
            .recipient_amount_sats
            .or(Some(transaction.amount_sats)),
        created_at: None,
        raw: None,
    })
}

pub async fn decode(str: String) -> Result<String, ApiError> {
    crate::utils::decode_bolt11(str)
}

pub async fn decode_offer(offer: String) -> Result<String, ApiError> {
    crate::utils::decode_offer(offer)
}

pub async fn get_offer(_config: &BlinkConfig, _search: Option<String>) -> Result<Offer, ApiError> {
    Err(nwc_error(
        "NOT_IMPLEMENTED",
        "Bolt12 not implemented for Blink",
    ))
}

pub async fn list_offers(
    _config: &BlinkConfig,
    _search: Option<String>,
) -> Result<Vec<Offer>, ApiError> {
    Err(nwc_error(
        "NOT_IMPLEMENTED",
        "Bolt12 not implemented for Blink",
    ))
}

pub async fn create_offer(
    _config: &BlinkConfig,
    _amount_msats: Option<i64>,
    _description: Option<String>,
    _expiry: Option<i64>,
) -> Result<Transaction, ApiError> {
    Err(nwc_error(
        "NOT_IMPLEMENTED",
        "Bolt12 not implemented for Blink",
    ))
}

pub async fn fetch_invoice_from_offer(
    _config: &BlinkConfig,
    _offer: String,
    _amount_msats: i64,
    _payer_note: Option<String>,
) -> Result<crate::cln::types::FetchInvoiceResponse, ApiError> {
    Err(nwc_error(
        "NOT_IMPLEMENTED",
        "Bolt12 not implemented for Blink",
    ))
}

pub async fn pay_offer(
    _config: &BlinkConfig,
    _offer: String,
    _amount_msats: i64,
    _payer_note: Option<String>,
) -> Result<PayInvoiceResponse, ApiError> {
    Err(nwc_error(
        "NOT_IMPLEMENTED",
        "Bolt12 not implemented for Blink",
    ))
}

pub async fn lookup_invoice(
    config: &BlinkConfig,
    payment_hash: Option<String>,
    from: Option<i64>,
    limit: Option<i64>,
    search: Option<String>,
) -> Result<Transaction, ApiError> {
    let target_payment_hash = payment_hash.unwrap_or_default();

    // Get transactions and look for the specific payment hash, using parameters or defaults
    let transactions =
        list_transactions(config, from.unwrap_or(0), limit.unwrap_or(100), search).await?;

    let transaction = transactions
        .into_iter()
        .find(|t| t.payment_hash == target_payment_hash)
        .ok_or_else(|| {
            nwc_error(
                "NOT_FOUND",
                format!(
                    "Transaction not found for payment hash: {}",
                    target_payment_hash
                ),
            )
        })?;

    Ok(transaction)
}

pub async fn list_transactions(
    config: &BlinkConfig,
    from: i64,
    limit: i64,
    search: Option<String>,
) -> Result<Vec<Transaction>, ApiError> {
    let query = r#"
        query TransactionsQuery($first: Int, $last: Int, $after: String, $before: String) {
            me {
                defaultAccount {
                    transactions(first: $first, last: $last, after: $after, before: $before) {
                        edges {
                            cursor
                            node {
                                id
                                createdAt
                                direction
                                status
                                memo
                                settlementAmount
                                settlementCurrency
                                settlementFee
                                settlementDisplayAmount
                                settlementDisplayCurrency
                                settlementDisplayFee
                                settlementPrice {
                                    base
                                    offset
                                    currencyUnit
                                }
                                initiationVia {
                                    __typename
                                    ... on InitiationViaLn {
                                        paymentHash
                                    }
                                }
                                settlementVia {
                                    __typename
                                    ... on SettlementViaLn {
                                        preImage
                                    }
                                    ... on SettlementViaIntraLedger {
                                        preImage
                                    }
                                }
                            }
                        }
                        pageInfo {
                            hasNextPage
                            hasPreviousPage
                            startCursor
                            endCursor
                        }
                    }
                }
            }
        }
    "#;

    // Simple approach: map limit directly to $first, handle from with client-side skip
    // This is cleaner than trying to convert integer offsets to opaque cursors
    let variables = serde_json::json!({
        "first": (from + limit) as i32,  // Fetch enough to skip 'from' records
        "last": serde_json::Value::Null,
        "after": serde_json::Value::Null,
        "before": serde_json::Value::Null
    });

    let response: TransactionsQuery =
        execute_graphql_query(config, query, Some(variables), "list_transactions").await?;

    let mut all_transactions = Vec::new();

    for edge in response.me.default_account.transactions.edges {
        let node = edge.node;

        // Extract Lightning-specific information
        let payment_hash = match node.initiation_via {
            Some(InitiationVia::InitiationViaLn { payment_hash }) => payment_hash,
            _ => "".to_string(),
        };

        let preimage = match node.settlement_via {
            Some(SettlementVia::SettlementViaLn { pre_image })
            | Some(SettlementVia::SettlementViaIntraLedger { pre_image }) => {
                pre_image.unwrap_or_default()
            }
            _ => "".to_string(),
        };

        // Handle amount conversion based on settlement currency
        let (amount_msats, fees_paid) = if let Some(currency) = &node.settlement_currency {
            if currency == "BTC" {
                // BTC amounts are in satoshis, convert to millisatoshis
                let amount = (node.settlement_amount.unwrap_or(0).abs()) * 1000;
                let fees = (node.settlement_fee.unwrap_or(0).abs()) * 1000;
                (amount, fees)
            } else if currency == "USD" {
                // USD amounts - for now return 0 as we can't meaningfully convert to satoshis
                // without current exchange rate data
                (0, 0)
            } else {
                // Other currencies
                (0, 0)
            }
        } else {
            // No settlement currency available
            (0, 0)
        };

        // Use the timestamp directly since it's already a Unix timestamp
        let created_at = node.created_at;

        let settled_at = if node.status == "SUCCESS" {
            created_at
        } else {
            0
        };

        all_transactions.push(Transaction {
            type_: if node.direction == "SEND" {
                "outgoing"
            } else {
                "incoming"
            }
            .to_string(),
            invoice: "".to_string(), // Not available from this query
            preimage,
            payment_hash,
            amount_msats,
            fees_paid,
            created_at,
            expires_at: 0, // Not available from this query
            settled_at,
            description: node.memo.unwrap_or_default(),
            description_hash: "".to_string(),
            payer_note: Some("".to_string()),
            external_id: Some(node.id),
        });
    }

    // Apply client-side search filtering if search term is provided
    if let Some(search_term) = search {
        let search_lower = search_term.to_lowercase();
        all_transactions.retain(|tx| {
            tx.description.to_lowercase().contains(&search_lower)
                || tx.payment_hash.to_lowercase().contains(&search_lower)
                || tx.preimage.to_lowercase().contains(&search_lower)
        });
    }

    // Apply client-side pagination: skip 'from' records and take 'limit' records
    let skip_count = from as usize;
    let take_count = limit as usize;

    if skip_count < all_transactions.len() {
        let end_index = std::cmp::min(skip_count + take_count, all_transactions.len());
        all_transactions = all_transactions[skip_count..end_index].to_vec();
    } else {
        all_transactions.clear();
    }

    Ok(all_transactions)
}

// Core logic shared by both implementations
pub async fn poll_invoice_events<F>(
    config: &BlinkConfig,
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
            config,
            params.payment_hash.clone(),
            None,
            None,
            params.search.clone(),
        )
        .await
        {
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
    config: BlinkConfig,
    params: OnInvoiceEventParams,
    callback: std::sync::Arc<dyn OnInvoiceEventCallback>,
) {
    poll_invoice_events(&config, params, move |status, tx| match status.as_str() {
        "success" => callback.success(tx),
        "pending" => callback.pending(tx),
        "failure" | _ => callback.failure(tx),
    })
    .await;
}
