use std::str::FromStr;
use std::time::Duration;

use lightning_invoice::Bolt11Invoice;
use once_cell::sync::Lazy;
use regex::Regex;

use super::types::*;
use super::{
    GaloyConfig, GaloyInvoiceOperation, GaloyPaymentOutcome, GaloyPaymentResponse,
    GaloyPaymentState, GaloyWalletConfig,
};
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

static SENSITIVE_TEXT: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"(?i)\b(?:(?:lnbc|lntb|lnbcrt)[a-z0-9]+|(?:lno|lnr|lni)1[a-z0-9]+)\b|\b[0-9a-f]{64}\b",
    )
    .expect("sensitive-value regex must compile")
});

static SENSITIVE_LABEL_VALUE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"(?i)\b(api[_-]?key|x-api-key|authorization|access[_-]?token|token|payment[_-]?request|payment[_-]?hash|payment[_-]?secret|preimage)\s*[:=]\s*[^\s,;]+",
    )
    .expect("sensitive-label regex must compile")
});

fn map_galoy_provider_error(info: &ProviderErrorInfo) -> Option<&'static str> {
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

fn redact_json_value(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(fields) => {
            for (key, value) in fields {
                let normalized = key
                    .chars()
                    .filter(|character| character.is_ascii_alphanumeric())
                    .collect::<String>()
                    .to_ascii_lowercase();
                if matches!(
                    normalized.as_str(),
                    "apikey"
                        | "xapikey"
                        | "authorization"
                        | "accesstoken"
                        | "token"
                        | "paymentrequest"
                        | "paymenthash"
                        | "paymentsecret"
                        | "preimage"
                ) {
                    *value = serde_json::Value::String("<redacted>".to_string());
                } else {
                    redact_json_value(value);
                }
            }
        }
        serde_json::Value::Array(values) => {
            for value in values {
                redact_json_value(value);
            }
        }
        serde_json::Value::String(text) => {
            *text = SENSITIVE_TEXT.replace_all(text, "<redacted>").to_string();
        }
        _ => {}
    }
}

fn sanitize_text(config: &GaloyConfig, text: impl AsRef<str>) -> String {
    let mut text = if config.api_key.is_empty() {
        text.as_ref().to_string()
    } else {
        text.as_ref().replace(&config.api_key, "<redacted>")
    };
    if let Some(proxy_url) = config.socks5_proxy.as_deref() {
        if !proxy_url.is_empty() {
            text = text.replace(proxy_url, "<redacted>");
        }
    }
    if let Ok(mut value) = serde_json::from_str::<serde_json::Value>(&text) {
        redact_json_value(&mut value);
        return value.to_string();
    }
    let text = SENSITIVE_TEXT.replace_all(&text, "<redacted>");
    SENSITIVE_LABEL_VALUE
        .replace_all(&text, "$1=<redacted>")
        .to_string()
}

fn provider_nwc_error(config: &GaloyConfig, code: &str, message: impl AsRef<str>) -> ApiError {
    nwc_error(
        code,
        format!(
            "[{}] {}",
            config.provider.id,
            sanitize_text(config, message)
        ),
    )
}

fn attach_provider(config: &GaloyConfig, error: ApiError) -> ApiError {
    match error {
        ApiError::Nwc { code, message } => provider_nwc_error(config, &code, message),
        other => other,
    }
}

fn galoy_graphql_error(
    config: &GaloyConfig,
    errors: &[GraphQLError],
    fallback_code: &str,
) -> ApiError {
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
    let code = map_galoy_provider_error(&info).unwrap_or(fallback_code);
    provider_nwc_error(
        config,
        code,
        info.message
            .unwrap_or_else(|| format!("{} GraphQL error", config.provider.name)),
    )
}

fn client(config: &GaloyConfig) -> Result<reqwest::Client, ApiError> {
    let mut headers = reqwest::header::HeaderMap::new();

    if let Some(additional_headers) = &config.additional_headers {
        for (name, value) in additional_headers {
            let Ok(name) = header::HeaderName::from_str(name) else {
                return Err(ApiError::InvalidInput(format!(
                    "Invalid additional Galoy header name: {}",
                    name
                )));
            };
            if name == header::CONTENT_TYPE || name.as_str().eq_ignore_ascii_case("x-api-key") {
                continue;
            }
            let value = header::HeaderValue::from_str(value).map_err(|_| {
                ApiError::InvalidInput(format!(
                    "Invalid value for additional Galoy header {}",
                    name
                ))
            })?;
            headers.insert(name, value);
        }
    }

    let api_key_header = header::HeaderValue::from_str(&config.api_key)
        .map_err(|_| ApiError::InvalidInput("Invalid Galoy API key header value".to_string()))?;
    headers.insert("X-API-KEY", api_key_header);

    headers.insert(
        "Content-Type",
        header::HeaderValue::from_static("application/json"),
    );

    let mut client_builder = reqwest::Client::builder().default_headers(headers);
    if config.accept_invalid_certs.unwrap_or(false) {
        client_builder = client_builder.danger_accept_invalid_certs(true);
    }
    if let Some(http_timeout) = config.http_timeout {
        client_builder =
            client_builder.timeout(std::time::Duration::from_secs(http_timeout.max(0) as u64));
    }
    if let Some(proxy_url) = config
        .socks5_proxy
        .as_deref()
        .filter(|proxy_url| !proxy_url.is_empty())
    {
        let proxy = reqwest::Proxy::all(proxy_url).map_err(|error| ApiError::Http {
            reason: sanitize_text(
                config,
                format!(
                    "Failed to configure {} SOCKS5 proxy: {}",
                    config.provider.name, error
                ),
            ),
        })?;
        client_builder = client_builder.proxy(proxy);
    }

    client_builder.build().map_err(|error| ApiError::Http {
        reason: sanitize_text(
            config,
            format!(
                "Failed to build {} HTTP client: {}",
                config.provider.name, error
            ),
        ),
    })
}

async fn execute_graphql_query<T>(
    config: &GaloyConfig,
    query: &str,
    variables: Option<serde_json::Value>,
    operation: &str,
) -> Result<T, ApiError>
where
    T: for<'de> serde::Deserialize<'de>,
{
    let client = client(config)?;
    let request = GraphQLRequest {
        query: query.to_string(),
        variables,
    };

    let response = client
        .post(&config.base_url)
        .json(&request)
        .send()
        .await
        .map_err(|e| transport_error(&config.provider.id, operation, e))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response.text().await.unwrap_or_default();
        return Err(attach_provider(
            config,
            provider_error_from_response(
                &config.provider.id,
                operation,
                status,
                sanitize_text(config, error_text),
                map_galoy_provider_error,
            ),
        ));
    }

    let response_text = response.text().await.map_err(|e| ApiError::Http {
        reason: format!("Failed to read GraphQL response: {}", e),
    })?;
    let graphql_response: GraphQLResponse<T> =
        serde_json::from_str(&response_text).map_err(|e| ApiError::Json {
            reason: format!(
                "Failed to parse GraphQL response: {} - Response: {}",
                e,
                sanitize_text(config, &response_text)
            ),
        })?;

    if let Some(errors) = graphql_response.errors {
        return Err(galoy_graphql_error(config, &errors, "OTHER"));
    }

    graphql_response.data.ok_or_else(|| {
        provider_nwc_error(
            config,
            "INTERNAL",
            format!("No data in {} GraphQL response", config.provider.name),
        )
    })
}

async fn resolve_wallet(config: &GaloyConfig) -> Result<Wallet, ApiError> {
    if let GaloyWalletConfig::Explicit { id, currency } = &config.wallet {
        return Ok(Wallet {
            id: id.clone(),
            wallet_currency: currency.clone(),
            balance: None,
        });
    }

    let requested_currency = config.wallet.currency();
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

    response
        .me
        .default_account
        .wallets
        .into_iter()
        .find(|wallet| {
            wallet
                .wallet_currency
                .eq_ignore_ascii_case(requested_currency)
        })
        .ok_or_else(|| {
            provider_nwc_error(
                config,
                "NOT_FOUND",
                format!(
                    "No {} wallet found for {}",
                    requested_currency, config.provider.name
                ),
            )
        })
}

fn assert_onchain_enabled(config: &GaloyConfig) -> Result<(), ApiError> {
    if config.capabilities.onchain {
        Ok(())
    } else {
        Err(provider_nwc_error(
            config,
            "NOT_IMPLEMENTED",
            format!(
                "On-chain payments are disabled for {}",
                config.provider.name
            ),
        ))
    }
}

async fn resolve_btc_onchain_wallet(config: &GaloyConfig) -> Result<Wallet, ApiError> {
    assert_onchain_enabled(config)?;
    let wallet = resolve_wallet(config).await?;
    if wallet.wallet_currency.eq_ignore_ascii_case("BTC") {
        Ok(wallet)
    } else {
        Err(provider_nwc_error(
            config,
            "NOT_IMPLEMENTED",
            format!(
                "On-chain payments require a BTC wallet for {}",
                config.provider.name
            ),
        ))
    }
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

fn resolve_galoy_fee_speed(fee: &OnchainFeePreference) -> Result<&'static str, ApiError> {
    match fee.preference_type {
        OnchainFeePreferenceType::Default => Ok("FAST"),
        OnchainFeePreferenceType::Speed => match fee.speed.clone().unwrap_or(OnchainFeeSpeed::Fast)
        {
            OnchainFeeSpeed::Fast => Ok("FAST"),
            OnchainFeeSpeed::Normal => Ok("MEDIUM"),
            OnchainFeeSpeed::Slow => Ok("SLOW"),
            OnchainFeeSpeed::Free => Err(ApiError::InvalidInput(
                "Galoy on-chain payments do not support free fee speed".to_string(),
            )),
        },
        OnchainFeePreferenceType::Backend
        | OnchainFeePreferenceType::TargetConf
        | OnchainFeePreferenceType::SatsPerVbyte => Err(ApiError::InvalidInput(format!(
            "Galoy on-chain payments do not support {:?} fee preferences",
            fee.preference_type
        ))),
    }
}

fn resolve_galoy_fee_payer(
    fee_payer: Option<OnchainFeePayer>,
) -> Result<OnchainFeePayer, ApiError> {
    match fee_payer.unwrap_or(OnchainFeePayer::Sender) {
        OnchainFeePayer::Sender => Ok(OnchainFeePayer::Sender),
        OnchainFeePayer::Recipient => Err(ApiError::InvalidInput(
            "Galoy on-chain payments only support sender-paid fees".to_string(),
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

fn galoy_transaction_amount_to_sats(amount: Option<i64>, currency: Option<&String>) -> Option<i64> {
    if currency.map(|currency| currency == "BTC").unwrap_or(false) {
        amount.map(|amount| amount.abs())
    } else {
        None
    }
}

fn galoy_transaction_memo(transaction: &OnchainTransaction) -> Option<String> {
    let raw = transaction.raw.as_deref()?;
    let value: serde_json::Value = serde_json::from_str(raw).ok()?;
    value
        .get("memo")
        .and_then(|memo| memo.as_str())
        .filter(|memo| !memo.is_empty())
        .map(|memo| memo.to_string())
}

pub async fn get_info(config: &GaloyConfig) -> Result<NodeInfo, ApiError> {
    let wallet = resolve_wallet(config).await?;
    let balance_sats = if wallet.wallet_currency.eq_ignore_ascii_case("BTC") {
        wallet.balance.unwrap_or(0)
    } else {
        0
    };
    let balance_msats = balance_sats * 1000;

    Ok(NodeInfo {
        alias: format!("{} Node", config.provider.name),
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
    config: &GaloyConfig,
    invoice_params: CreateInvoiceParams,
) -> Result<Transaction, ApiError> {
    match invoice_params.get_invoice_type() {
        InvoiceType::Bolt11 => {
            match config.invoice_operations.create {
                GaloyInvoiceOperation::Unsupported => {
                    return Err(provider_nwc_error(
                        config,
                        "NOT_IMPLEMENTED",
                        format!("Invoice creation is disabled for {}", config.provider.name),
                    ));
                }
                GaloyInvoiceOperation::UsdCents => {
                    return Err(provider_nwc_error(
                        config,
                        "NOT_IMPLEMENTED",
                        format!(
                            "{} USD invoice creation requires a USD-cent amount, which cannot be represented safely by LNI's amount_msats input",
                            config.provider.name
                        ),
                    ));
                }
                GaloyInvoiceOperation::BtcSats => {}
            }

            let wallet = resolve_wallet(config).await?;
            let field = "lnInvoiceCreate";
            let input_type = "LnInvoiceCreateInput";

            let amount_sats = invoice_params.amount_msats.unwrap_or(0) / 1000;

            let query = r#"
                mutation __FIELD__($input: __INPUT__!) {
                    __FIELD__(input: $input) {
                        invoice {
                            paymentRequest
                            paymentHash
                            satoshis
                        }
                        errors {
                            code
                            message
                            path
                        }
                    }
                }
            "#
            .replace("__FIELD__", field)
            .replace("__INPUT__", input_type);

            let variables = serde_json::json!({
                "input": {
                    "amount": amount_sats.to_string(),
                    "walletId": wallet.id,
                    "memo": invoice_params.description
                }
            });

            let response: LnInvoiceCreateResponse =
                execute_graphql_query(config, &query, Some(variables), "make_invoice").await?;
            let result = response.ln_invoice_create.ok_or_else(|| {
                provider_nwc_error(
                    config,
                    "INTERNAL",
                    format!("No {} result in response", field),
                )
            })?;

            if let Some(errors) = &result.errors {
                if !errors.is_empty() {
                    return Err(galoy_graphql_error(config, errors, "OTHER"));
                }
            }

            let invoice = result
                .invoice
                .ok_or_else(|| provider_nwc_error(config, "INTERNAL", "No invoice in response"))?;

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
        InvoiceType::Bolt12 => Err(provider_nwc_error(
            config,
            "NOT_IMPLEMENTED",
            format!("Bolt12 is not implemented for {}", config.provider.name),
        )),
    }
}

pub async fn pay_invoice(
    config: &GaloyConfig,
    invoice_params: PayInvoiceParams,
) -> Result<PayInvoiceResponse, ApiError> {
    Ok(pay_invoice_with_status(config, invoice_params)
        .await?
        .payment)
}

pub async fn pay_invoice_with_status(
    config: &GaloyConfig,
    invoice_params: PayInvoiceParams,
) -> Result<GaloyPaymentOutcome, ApiError> {
    let parses_without_amount = Bolt11Invoice::from_str(&invoice_params.invoice)
        .map(|invoice| invoice.amount_milli_satoshis().is_none())
        .unwrap_or(false);
    if parses_without_amount {
        return Err(ApiError::InvalidInput(format!(
            "{} cannot pay amountless BOLT11 invoices because Galoy's payment mutation has no amount field",
            config.provider.name
        )));
    }

    let wallet = resolve_wallet(config).await?;
    let fee_operation = config.invoice_operations.fee_probe;
    let (fee_field, fee_input_type) = match fee_operation {
        GaloyInvoiceOperation::BtcSats => ("lnInvoiceFeeProbe", "LnInvoiceFeeProbeInput"),
        GaloyInvoiceOperation::UsdCents => ("lnUsdInvoiceFeeProbe", "LnUsdInvoiceFeeProbeInput"),
        GaloyInvoiceOperation::Unsupported => {
            return Err(provider_nwc_error(
                config,
                "NOT_IMPLEMENTED",
                format!(
                    "Lightning fee probing is not configured for {} wallet currency {}",
                    config.provider.name, wallet.wallet_currency
                ),
            ));
        }
    };

    let fee_probe_query = r#"
        mutation __FIELD__($input: __INPUT__!) {
            __FIELD__(input: $input) {
                errors {
                    code
                    message
                    path
                }
                amount
            }
        }
    "#
    .replace("__FIELD__", fee_field)
    .replace("__INPUT__", fee_input_type);

    let fee_probe_variables = serde_json::json!({
        "input": {
            "paymentRequest": invoice_params.invoice,
            "walletId": wallet.id
        }
    });

    let fee_response: LnInvoiceFeeProbeResponse = execute_graphql_query(
        config,
        &fee_probe_query,
        Some(fee_probe_variables),
        "pay_invoice",
    )
    .await?;
    let fee_result = match fee_operation {
        GaloyInvoiceOperation::BtcSats => fee_response.ln_invoice_fee_probe,
        GaloyInvoiceOperation::UsdCents => fee_response.ln_usd_invoice_fee_probe,
        GaloyInvoiceOperation::Unsupported => unreachable!("unsupported fee probes return above"),
    }
    .ok_or_else(|| {
        provider_nwc_error(
            config,
            "INTERNAL",
            format!("No {} result in response", fee_field),
        )
    })?;

    if let Some(errors) = &fee_result.errors {
        if !errors.is_empty() {
            return Err(galoy_graphql_error(config, errors, "PAYMENT_FAILED"));
        }
    }
    let fee_msats = if fee_operation == GaloyInvoiceOperation::BtcSats {
        fee_result.amount.unwrap_or(0) * 1000
    } else {
        0
    };

    let transaction_selection = match config.payment.response {
        GaloyPaymentResponse::TransactionWithPreimage => {
            r#"
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
        "#
        }
        GaloyPaymentResponse::StatusOnly => "",
    };
    let payment_query = r#"
        mutation LnInvoicePaymentSend($input: LnInvoicePaymentInput!) {
            lnInvoicePaymentSend(input: $input) {
                status
                errors {
                    message
                    path
                    code
                }
                __TRANSACTION_SELECTION__
            }
        }
    "#
    .replace("__TRANSACTION_SELECTION__", transaction_selection);

    let payment_variables = serde_json::json!({
        "input": {
            "paymentRequest": invoice_params.invoice,
            "walletId": wallet.id
        }
    });

    let payment_response: LnInvoicePaymentSendResponse = execute_graphql_query(
        config,
        &payment_query,
        Some(payment_variables),
        "pay_invoice",
    )
    .await?;

    let status = payment_response.ln_invoice_payment_send.status.clone();
    if !config.payment.accepted_statuses.contains(&status) {
        if let Some(errors) = &payment_response.ln_invoice_payment_send.errors {
            if !errors.is_empty() {
                let provider_message = errors
                    .iter()
                    .map(|error| error.message.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                let info = ProviderErrorInfo {
                    code: errors.first().and_then(|error| error.code.clone()),
                    status: None,
                    message: Some(provider_message.clone()),
                };
                return Err(provider_nwc_error(
                    config,
                    map_galoy_provider_error(&info).unwrap_or("PAYMENT_FAILED"),
                    format!(
                        "{} payment was not accepted with status {} ({})",
                        config.provider.name, status, provider_message
                    ),
                ));
            }
        }
        return Err(provider_nwc_error(
            config,
            if status == "FAILED" || status == "FAILURE" {
                "PAYMENT_FAILED"
            } else {
                "OTHER"
            },
            format!(
                "{} payment was not accepted with status {}",
                config.provider.name, status
            ),
        ));
    }

    let preimage = match payment_response
        .ln_invoice_payment_send
        .transaction
        .as_ref()
        .and_then(|transaction| transaction.settlement_via.as_ref())
    {
        Some(SettlementVia::SettlementViaLn { pre_image })
        | Some(SettlementVia::SettlementViaIntraLedger { pre_image }) => {
            pre_image.clone().unwrap_or_default()
        }
        _ => "".to_string(),
    };

    if let Some(errors) = &payment_response.ln_invoice_payment_send.errors {
        let unexpected_errors = errors
            .iter()
            .filter(|error| {
                let proof_unavailable = preimage.is_empty()
                    && error.code.as_deref().is_some_and(|code| {
                        config
                            .payment
                            .proof_unavailable_error_codes
                            .iter()
                            .any(|accepted| accepted.eq_ignore_ascii_case(code))
                    });
                !proof_unavailable
            })
            .cloned()
            .collect::<Vec<_>>();
        if !unexpected_errors.is_empty() {
            return Err(galoy_graphql_error(
                config,
                &unexpected_errors,
                "PAYMENT_FAILED",
            ));
        }
    }

    // Extract payment hash from the BOLT11 invoice
    let payment_hash = match Bolt11Invoice::from_str(&invoice_params.invoice) {
        Ok(invoice) => format!("{:x}", invoice.payment_hash()),
        Err(_) => "".to_string(),
    };
    let state = if let Some(mapping) = &config.payment.status_mapping {
        if mapping.settled.contains(&status) {
            GaloyPaymentState::Settled
        } else if mapping.pending.contains(&status) {
            GaloyPaymentState::Pending
        } else {
            GaloyPaymentState::Accepted
        }
    } else {
        GaloyPaymentState::Accepted
    };

    Ok(GaloyPaymentOutcome {
        payment: PayInvoiceResponse {
            payment_hash,
            preimage,
            fee_msats,
        },
        state,
        provider_status: status,
    })
}

pub async fn prepare_onchain_transaction(
    config: &GaloyConfig,
    params: PrepareOnchainTransactionParams,
) -> Result<OnchainTransaction, ApiError> {
    assert_onchain_enabled(config)?;
    assert_valid_onchain_amount(params.amount_sats)?;

    let fee = params.fee.clone().unwrap_or_else(default_onchain_fee);
    let fee_payer = resolve_galoy_fee_payer(params.fee_payer.clone())?;
    let speed = resolve_galoy_fee_speed(&fee)?;
    let wallet_id = resolve_btc_onchain_wallet(config).await?.id;

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

    let response: OnChainTxFeeResponse = execute_graphql_query(
        config,
        query,
        Some(variables),
        "prepare_onchain_transaction",
    )
    .await?;
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
    config: &GaloyConfig,
    transaction: OnchainTransaction,
) -> Result<PayOnchainResponse, ApiError> {
    pay_onchain_with_options(config, transaction, PayOnchainOptions::default()).await
}

pub async fn pay_onchain_with_options(
    config: &GaloyConfig,
    transaction: OnchainTransaction,
    options: PayOnchainOptions,
) -> Result<PayOnchainResponse, ApiError> {
    assert_onchain_enabled(config)?;
    assert_valid_onchain_amount(transaction.amount_sats)?;
    let _fee_payer = resolve_galoy_fee_payer(Some(transaction.fee_payer.clone()))?;
    let speed = resolve_galoy_fee_speed(&transaction.fee)?;
    assert_onchain_fee_guardrail(&transaction, options)?;

    let wallet_id = resolve_btc_onchain_wallet(config).await?.id;
    let memo = galoy_transaction_memo(&transaction);
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
        execute_graphql_query(config, query, Some(variables), "pay_onchain").await?;
    let payment = response.on_chain_payment_send;

    if let Some(errors) = &payment.errors {
        if !errors.is_empty() {
            return Err(galoy_graphql_error(config, errors, "PAYMENT_FAILED"));
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
            galoy_transaction_amount_to_sats(tx.settlement_fee, tx.settlement_currency.as_ref())
        })
        .or(transaction.fee_sats);
    let amount_sats = payment_transaction
        .as_ref()
        .and_then(|tx| {
            galoy_transaction_amount_to_sats(tx.settlement_amount, tx.settlement_currency.as_ref())
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
            .map(|fee_sats| amount_sats + fee_sats)
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

pub fn not_implemented<T>(
    config: &GaloyConfig,
    feature: &str,
    _operation: &str,
) -> Result<T, ApiError> {
    Err(provider_nwc_error(
        config,
        "NOT_IMPLEMENTED",
        format!(
            "{} is not implemented for {}",
            feature, config.provider.name
        ),
    ))
}

pub async fn get_offer(config: &GaloyConfig, _search: Option<String>) -> Result<Offer, ApiError> {
    not_implemented(config, "Bolt12", "lookup_invoice")
}

pub async fn list_offers(
    config: &GaloyConfig,
    _search: Option<String>,
) -> Result<Vec<Offer>, ApiError> {
    not_implemented(config, "Bolt12", "list_transactions")
}

pub async fn create_offer(
    config: &GaloyConfig,
    _amount_msats: Option<i64>,
    _description: Option<String>,
    _expiry: Option<i64>,
) -> Result<Transaction, ApiError> {
    not_implemented(config, "Bolt12", "make_invoice")
}

pub async fn fetch_invoice_from_offer(
    config: &GaloyConfig,
    _offer: String,
    _amount_msats: i64,
    _payer_note: Option<String>,
) -> Result<crate::cln::types::FetchInvoiceResponse, ApiError> {
    not_implemented(config, "Bolt12", "pay_invoice")
}

pub async fn pay_offer(
    config: &GaloyConfig,
    _offer: String,
    _amount_msats: i64,
    _payer_note: Option<String>,
) -> Result<PayInvoiceResponse, ApiError> {
    not_implemented(config, "Bolt12", "pay_invoice")
}

pub async fn lookup_invoice(
    config: &GaloyConfig,
    payment_hash: Option<String>,
    from: Option<i64>,
    limit: Option<i64>,
    search: Option<String>,
) -> Result<Transaction, ApiError> {
    if !config.capabilities.transaction_lookup {
        return not_implemented(config, "Transaction lookup", "lookup_invoice");
    }
    let target_payment_hash = payment_hash.unwrap_or_default();

    let transactions =
        list_transactions_impl(config, from.unwrap_or(0), limit.unwrap_or(100), search).await?;

    let transaction = transactions
        .into_iter()
        .find(|t| t.payment_hash == target_payment_hash)
        .ok_or_else(|| {
            provider_nwc_error(
                config,
                "NOT_FOUND",
                "Transaction not found for the requested payment hash",
            )
        })?;

    Ok(transaction)
}

pub async fn list_transactions(
    config: &GaloyConfig,
    from: i64,
    limit: i64,
    search: Option<String>,
) -> Result<Vec<Transaction>, ApiError> {
    if !config.capabilities.transaction_history {
        return not_implemented(config, "Transaction history", "list_transactions");
    }
    list_transactions_impl(config, from, limit, search).await
}

async fn list_transactions_impl(
    config: &GaloyConfig,
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
    let normalized_from = from.max(0);
    let normalized_limit = limit.clamp(1, 1000);
    let fetch_count = normalized_from
        .saturating_add(normalized_limit)
        .clamp(1, 1000) as i32;
    let variables = serde_json::json!({
        "first": fetch_count,  // Fetch enough to skip 'from' records, within Galoy's safe cap
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
            if currency.eq_ignore_ascii_case("BTC") {
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
    let skip_count = normalized_from as usize;
    let take_count = normalized_limit as usize;

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
    config: &GaloyConfig,
    params: OnInvoiceEventParams,
    mut callback: F,
) where
    F: FnMut(String, Option<Transaction>),
{
    if !config.capabilities.invoice_events || !config.capabilities.transaction_lookup {
        callback("failure".to_string(), None);
        return;
    }
    let start_time = std::time::Instant::now();
    let max_polling_sec = params.max_polling_sec.max(0) as u64;
    let polling_delay_sec = params.polling_delay_sec.clamp(1, 3600) as u64;
    loop {
        if start_time.elapsed() > Duration::from_secs(max_polling_sec) {
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

        tokio::time::sleep(Duration::from_secs(polling_delay_sec)).await;
    }
}

pub async fn on_invoice_events(
    config: GaloyConfig,
    params: OnInvoiceEventParams,
    callback: std::sync::Arc<dyn OnInvoiceEventCallback>,
) {
    if !config.capabilities.invoice_events || !config.capabilities.transaction_lookup {
        callback.failure(None);
        return;
    }
    poll_invoice_events(&config, params, move |status, tx| match status.as_str() {
        "success" => callback.success(tx),
        "pending" => callback.pending(tx),
        _ => callback.failure(tx),
    })
    .await;
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;
    use tokio::sync::mpsc;

    use super::*;
    use crate::galoy::{
        GaloyCapabilities, GaloyInvoiceOperation, GaloyInvoiceOperationsConfig, GaloyPaymentConfig,
        GaloyPaymentStatusMapping, GaloyPermissionsMode, GaloyProvider,
    };

    const BOLT11: &str = "lnbc2500u1pvjluezsp5zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zygspp5qqqsyqcyq5rqwzqfqqqsyqcyq5rqwzqfqqqsyqcyq5rqwzqfqypqdq5xysxxatsyp3k7enxv4jsxqzpu9qrsgquk0rl77nj30yxdy8j9vdx85fkpmdla2087ne0xh8nhedh8w27kyke0lp53ut353s06fv3qfegext0eh0ymjpf39tuven09sam30g4vgpfna3rh";
    const ONE_SAT_BOLT11S: [(&str, &str); 3] = [
        (
            "mainnet",
            "lnbc10n1pj48ugqdphf38yjgz8v9kx77fqxys8xct5ypex2emjv4ehx6t0dcsxv6tcw36hyegpp5qurswpc8qurswpc8qurswpc8qurswpc8qurswpc8qurswpc8qurssp5pqyqszqgpqyqszqgpqyqszqgpqyqszqgpqyqszqgpqyqszqgpqyq9qrsgqcqpj85c7x06rkw8s97xtjaarx4y4sgumglauw96fkcdr3yatkshg23gj57pj350za5ppku4d4hl8p6xj9ty7t84z2594q9hl7vf4em9en8cp3rvsy3",
        ),
        (
            "testnet",
            "lntb10n1pj48ugqdphf38yjgz8v9kx77fqxys8xct5ypex2emjv4ehx6t0dcsxv6tcw36hyegpp5qurswpc8qurswpc8qurswpc8qurswpc8qurswpc8qurswpc8qurssp5pqyqszqgpqyqszqgpqyqszqgpqyqszqgpqyqszqgpqyqszqgpqyq9qrsgqcqpjglkgmmeh8nlna32uhyqvpmrxk52er02glraz7jywwxg0tz0ahuxrtzanfvxjrugv2zuv8dxvakvk5p3fuxeexym8ff96m25s7ks750sqknxxzt",
        ),
        (
            "regtest",
            "lnbcrt10n1pj48ugqdphf38yjgz8v9kx77fqxys8xct5ypex2emjv4ehx6t0dcsxv6tcw36hyegpp5qurswpc8qurswpc8qurswpc8qurswpc8qurswpc8qurswpc8qurssp5pqyqszqgpqyqszqgpqyqszqgpqyqszqgpqyqszqgpqyqszqgpqyq9qrsgqcqpj7n3tcnerf357qjapjupmduwrryy3vdfk63jh45ssw86v34cxtdc9kk2n8hlhgs9f6uprj3eaxz54fwp3w2rkafhphh05llhtjdqp4sqptf062s",
        ),
    ];
    const AMOUNTLESS_BOLT11: &str = "lnbc1pj48ugqdplf38yjgz8v9kx77fqv9kk7atww3kx2umnypex2emjv4ehx6t0dcsxv6tcw36hyegpp5pyysjzgfpyysjzgfpyysjzgfpyysjzgfpyysjzgfpyysjzgfpyyssp5pg9q5zs2pg9q5zs2pg9q5zs2pg9q5zs2pg9q5zs2pg9q5zs2pg9q9qrsgqcqpjvcwldrltwv8ce6n00l8gl20vz5q3vu56hhmla07u39tmdy0ll6cs9crysytmdvugwrv2e6nwhfvlhd0mnjvskaefd43j9vdzjaggtygqe8yu0t";

    fn explicit_config(base_url: String, currency: &str) -> GaloyConfig {
        GaloyConfig {
            api_key: "server-api-key".to_string(),
            base_url,
            provider: GaloyProvider {
                id: "flash".to_string(),
                name: "Flash".to_string(),
            },
            wallet: GaloyWalletConfig::Explicit {
                id: "wallet-explicit".to_string(),
                currency: currency.to_string(),
            },
            invoice_operations: GaloyInvoiceOperationsConfig {
                create: GaloyInvoiceOperation::Unsupported,
                fee_probe: if currency.eq_ignore_ascii_case("BTC") {
                    GaloyInvoiceOperation::BtcSats
                } else if currency.eq_ignore_ascii_case("USD") {
                    GaloyInvoiceOperation::UsdCents
                } else {
                    GaloyInvoiceOperation::Unsupported
                },
            },
            payment: GaloyPaymentConfig {
                response: GaloyPaymentResponse::StatusOnly,
                accepted_statuses: vec![
                    "SUCCESS".to_string(),
                    "PENDING".to_string(),
                    "ALREADY_PAID".to_string(),
                ],
                status_mapping: Some(GaloyPaymentStatusMapping {
                    settled: vec!["SUCCESS".to_string(), "ALREADY_PAID".to_string()],
                    pending: vec!["PENDING".to_string()],
                }),
                proof_unavailable_error_codes: vec!["PROOF_UNAVAILABLE".to_string()],
            },
            capabilities: GaloyCapabilities {
                transaction_lookup: false,
                transaction_history: false,
                invoice_events: false,
                onchain: false,
            },
            permissions: GaloyPermissionsMode::Configured,
            additional_headers: None,
            http_timeout: Some(5),
            socks5_proxy: None,
            accept_invalid_certs: Some(false),
        }
    }

    fn find_header_end(bytes: &[u8]) -> Option<usize> {
        bytes
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .map(|position| position + 4)
    }

    async fn test_server(responses: Vec<serde_json::Value>) -> (String, mpsc::Receiver<String>) {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("test server should bind");
        let address = listener.local_addr().expect("test server address");
        let (sender, receiver) = mpsc::channel(responses.len());

        tokio::spawn(async move {
            for response_body in responses {
                let (mut stream, _) = listener.accept().await.expect("request should connect");
                let mut request = Vec::new();
                let mut buffer = [0_u8; 4096];
                loop {
                    let read = stream.read(&mut buffer).await.expect("request should read");
                    if read == 0 {
                        break;
                    }
                    request.extend_from_slice(&buffer[..read]);
                    let Some(header_end) = find_header_end(&request) else {
                        continue;
                    };
                    let headers = String::from_utf8_lossy(&request[..header_end]);
                    let content_length = headers
                        .lines()
                        .find_map(|line| {
                            let (name, value) = line.split_once(':')?;
                            name.eq_ignore_ascii_case("content-length")
                                .then(|| value.trim().parse::<usize>().ok())
                                .flatten()
                        })
                        .unwrap_or(0);
                    if request.len() >= header_end + content_length {
                        break;
                    }
                }

                sender
                    .send(String::from_utf8_lossy(&request).to_string())
                    .await
                    .expect("request capture should send");

                let body = response_body.to_string();
                let response = format!(
                    "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                stream
                    .write_all(response.as_bytes())
                    .await
                    .expect("response should write");
            }
        });

        (format!("http://{}/graphql", address), receiver)
    }

    fn graphql_body(request: &str) -> serde_json::Value {
        let (_, body) = request
            .split_once("\r\n\r\n")
            .expect("HTTP request should have a body");
        serde_json::from_str(body).expect("GraphQL body should be JSON")
    }

    #[test]
    fn configured_proxy_fails_closed_without_leaking_credentials() {
        let mut config = explicit_config("https://galoy.test/graphql".to_string(), "BTC");
        config.socks5_proxy = Some("socks5://user:password@[".to_string());

        let error = client(&config).expect_err("invalid proxy should fail client creation");
        let message = format!("{error:?}");
        assert!(message.contains("SOCKS5 proxy"));
        assert!(!message.contains("user"));
        assert!(!message.contains("password"));
    }

    #[tokio::test]
    async fn one_sat_invoices_reach_fee_probe_and_payment() {
        for (network, invoice) in ONE_SAT_BOLT11S {
            let decoded = Bolt11Invoice::from_str(invoice)
                .unwrap_or_else(|error| panic!("{network} fixture should decode: {error}"));
            assert_eq!(decoded.amount_milli_satoshis(), Some(1_000));

            let (base_url, mut requests) = test_server(vec![
                serde_json::json!({
                    "data": {
                        "lnInvoiceFeeProbe": {"amount": 1, "errors": []}
                    }
                }),
                serde_json::json!({
                    "data": {
                        "lnInvoicePaymentSend": {
                            "status": "SUCCESS",
                            "errors": []
                        }
                    }
                }),
            ])
            .await;
            let config = explicit_config(base_url, "BTC");

            let payment = pay_invoice(
                &config,
                PayInvoiceParams {
                    invoice: invoice.to_string(),
                    ..Default::default()
                },
            )
            .await
            .unwrap_or_else(|error| panic!("{network} 1-sat payment should proceed: {error:?}"));

            assert_eq!(payment.payment_hash, "07".repeat(32));
            assert!(payment.preimage.is_empty());
            assert_eq!(payment.fee_msats, 1_000);

            let fee_request = graphql_body(
                &requests
                    .recv()
                    .await
                    .unwrap_or_else(|| panic!("{network} fee request should be captured")),
            );
            assert!(fee_request["query"]
                .as_str()
                .expect("fee query")
                .contains("lnInvoiceFeeProbe"));
            let payment_request = graphql_body(
                &requests
                    .recv()
                    .await
                    .unwrap_or_else(|| panic!("{network} payment request should be captured")),
            );
            assert!(payment_request["query"]
                .as_str()
                .expect("payment query")
                .contains("lnInvoicePaymentSend"));
        }
    }

    #[tokio::test]
    async fn amountless_invoice_is_rejected_before_graphql() {
        let decoded =
            Bolt11Invoice::from_str(AMOUNTLESS_BOLT11).expect("amountless fixture should decode");
        assert_eq!(decoded.amount_milli_satoshis(), None);

        for amount_msats in [None, Some(1_000)] {
            let config = explicit_config("http://127.0.0.1:1/graphql".to_string(), "BTC");
            let error = pay_invoice(
                &config,
                PayInvoiceParams {
                    invoice: AMOUNTLESS_BOLT11.to_string(),
                    amount_msats,
                    ..Default::default()
                },
            )
            .await
            .expect_err("amountless Galoy payment should be rejected");

            assert!(matches!(
                error,
                ApiError::InvalidInput(ref message)
                    if message.contains("Flash cannot pay amountless BOLT11 invoices")
            ));
        }
    }

    #[tokio::test]
    async fn malformed_invoices_are_left_for_the_provider_to_reject() {
        let (base_url, mut requests) = test_server(vec![
            serde_json::json!({
                "data": {
                    "lnInvoiceFeeProbe": {"amount": 0, "errors": []}
                }
            }),
            serde_json::json!({
                "data": {
                    "lnInvoicePaymentSend": {
                        "status": "FAILURE",
                        "errors": [{
                            "code": "INVALID_INVOICE",
                            "message": "Invalid invoice"
                        }]
                    }
                }
            }),
        ])
        .await;
        let config = explicit_config(base_url, "BTC");

        let error = pay_invoice(
            &config,
            PayInvoiceParams {
                invoice: "not-a-bolt11-invoice".to_string(),
                ..Default::default()
            },
        )
        .await
        .expect_err("provider should reject malformed invoice");

        assert!(matches!(
            error,
            ApiError::Nwc { ref code, ref message }
                if code == "PAYMENT_FAILED"
                    && message.contains("[flash]")
                    && message.contains("Invalid invoice")
        ));
        let fee_request = graphql_body(&requests.recv().await.expect("fee request"));
        assert!(fee_request["query"]
            .as_str()
            .expect("fee query")
            .contains("lnInvoiceFeeProbe"));
        let payment_request = graphql_body(&requests.recv().await.expect("payment request"));
        assert!(payment_request["query"]
            .as_str()
            .expect("payment query")
            .contains("lnInvoicePaymentSend"));
    }

    #[tokio::test]
    async fn explicit_operations_are_not_inferred_from_wallet_currency_and_protect_headers() {
        let (base_url, mut requests) = test_server(vec![serde_json::json!({
            "data": {
                "lnInvoiceCreate": {
                    "invoice": {
                        "paymentRequest": BOLT11,
                        "paymentHash": "hash",
                        "satoshis": 21
                    },
                    "errors": []
                }
            }
        })])
        .await;
        let mut config = explicit_config(base_url, "JMD");
        config.invoice_operations.create = GaloyInvoiceOperation::BtcSats;
        config.additional_headers = Some(HashMap::from([
            (
                "x-flash-client-capabilities".to_string(),
                "proofless".to_string(),
            ),
            ("x-api-key".to_string(), "attacker-key".to_string()),
            ("content-type".to_string(), "text/plain".to_string()),
        ]));

        let transaction = create_invoice(
            &config,
            CreateInvoiceParams {
                amount_msats: Some(21_000),
                ..Default::default()
            },
        )
        .await
        .expect("invoice should be created");
        assert_eq!(transaction.amount_msats, 21_000);

        let request = requests.recv().await.expect("request should be captured");
        let request_lower = request.to_ascii_lowercase();
        assert!(request_lower.contains("x-api-key: server-api-key"));
        assert!(!request_lower.contains("attacker-key"));
        assert!(request_lower.contains("content-type: application/json"));
        assert!(request_lower.contains("x-flash-client-capabilities: proofless"));
        let body = graphql_body(&request);
        let query = body["query"].as_str().expect("query");
        assert!(query.contains("lnInvoiceCreate"));
        assert!(!query.contains("lnUsdInvoiceCreate"));
        assert!(!query.contains("query Me"));
        assert_eq!(body["variables"]["input"]["walletId"], "wallet-explicit");
    }

    #[tokio::test]
    async fn usd_cent_invoice_creation_is_rejected_before_graphql() {
        let mut config = explicit_config("http://127.0.0.1:1/graphql".to_string(), "USD");
        config.invoice_operations.create = GaloyInvoiceOperation::UsdCents;

        let error = create_invoice(
            &config,
            CreateInvoiceParams {
                amount_msats: Some(21_000),
                ..Default::default()
            },
        )
        .await
        .expect_err("USD-cent invoice creation must not reinterpret amount_msats");

        assert!(matches!(
            error,
            ApiError::Nwc { ref code, ref message }
                if code == "NOT_IMPLEMENTED" && message.contains("USD-cent")
        ));
    }

    #[tokio::test]
    async fn currency_wallet_mode_selects_currency_and_reports_missing_currency() {
        let (base_url, mut requests) = test_server(vec![serde_json::json!({
            "data": {
                "me": {
                    "defaultAccount": {
                        "wallets": [
                            {"id": "btc", "walletCurrency": "BTC", "balance": 10},
                            {"id": "jmd", "walletCurrency": "JMD", "balance": 500}
                        ]
                    }
                }
            }
        })])
        .await;
        let mut config = explicit_config(base_url, "JMD");
        config.wallet = GaloyWalletConfig::Currency {
            currency: "JMD".to_string(),
        };
        let info = get_info(&config).await.expect("JMD wallet should resolve");
        assert_eq!(info.alias, "Flash Node");
        assert_eq!(info.send_balance_msat, 0);
        assert!(
            graphql_body(&requests.recv().await.expect("request"))["query"]
                .as_str()
                .expect("query")
                .contains("query Me")
        );

        let (missing_url, mut missing_requests) = test_server(vec![serde_json::json!({
            "data": {
                "me": {
                    "defaultAccount": {
                        "wallets": [{"id": "btc", "walletCurrency": "BTC", "balance": 10}]
                    }
                }
            }
        })])
        .await;
        config.base_url = missing_url;
        config.wallet = GaloyWalletConfig::Currency {
            currency: "USD".to_string(),
        };
        let error = get_info(&config).await.expect_err("USD should be missing");
        missing_requests
            .recv()
            .await
            .expect("missing-wallet request");
        match error {
            ApiError::Nwc { code, message } => {
                assert_eq!(code, "NOT_FOUND");
                assert!(message.contains("[flash]"));
                assert!(message.contains("USD"));
            }
            other => panic!("expected NWC error, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn status_only_accepts_pending_omits_transaction_and_returns_zero_non_btc_fee() {
        let (base_url, mut requests) = test_server(vec![
            serde_json::json!({
                "data": {
                    "lnUsdInvoiceFeeProbe": {"amount": 99999, "errors": []}
                }
            }),
            serde_json::json!({
                "data": {
                    "lnInvoicePaymentSend": {
                        "status": "PENDING",
                        "errors": [{
                            "code": "PROOF_UNAVAILABLE",
                            "message": "No proof is exposed"
                        }]
                    }
                }
            }),
        ])
        .await;
        let config = explicit_config(base_url, "USD");

        let outcome = pay_invoice_with_status(
            &config,
            PayInvoiceParams {
                invoice: BOLT11.to_string(),
                ..Default::default()
            },
        )
        .await
        .expect("PENDING should be accepted");
        assert_eq!(outcome.payment.fee_msats, 0);
        assert!(outcome.payment.preimage.is_empty());
        assert!(!outcome.payment.payment_hash.is_empty());
        assert_eq!(outcome.state, GaloyPaymentState::Pending);
        assert_eq!(outcome.provider_status, "PENDING");

        let fee_query = graphql_body(&requests.recv().await.expect("fee request"));
        assert!(fee_query["query"]
            .as_str()
            .expect("fee query")
            .contains("lnUsdInvoiceFeeProbe"));
        let payment_query = graphql_body(&requests.recv().await.expect("payment request"));
        assert!(!payment_query["query"]
            .as_str()
            .expect("payment query")
            .contains("transaction {"));
    }

    #[tokio::test]
    async fn configured_payment_statuses_preserve_settlement_state() {
        for (status, expected_state) in [
            ("SUCCESS", GaloyPaymentState::Settled),
            ("ALREADY_PAID", GaloyPaymentState::Settled),
            ("PENDING", GaloyPaymentState::Pending),
        ] {
            let (base_url, _requests) = test_server(vec![
                serde_json::json!({
                    "data": {
                        "lnUsdInvoiceFeeProbe": {"amount": 75, "errors": []}
                    }
                }),
                serde_json::json!({
                    "data": {
                        "lnInvoicePaymentSend": {
                            "status": status,
                            "errors": []
                        }
                    }
                }),
            ])
            .await;
            let config = explicit_config(base_url, "USD");

            let outcome = pay_invoice_with_status(
                &config,
                PayInvoiceParams {
                    invoice: BOLT11.to_string(),
                    ..Default::default()
                },
            )
            .await
            .expect("configured payment status should be accepted");

            assert_eq!(outcome.state, expected_state);
            assert_eq!(outcome.provider_status, status);
            assert_eq!(outcome.payment.fee_msats, 0);
            assert!(outcome.payment.preimage.is_empty());
            assert!(!outcome.payment.payment_hash.is_empty());
        }
    }

    #[tokio::test]
    async fn unexpected_errors_on_accepted_payments_are_surfaced_and_sanitized() {
        let sensitive_hash = "a".repeat(64);
        let provider_message = format!(
            "unexpected paymentRequest={} paymentHash={} paymentSecret=payment-secret preimage=provider-preimage access_token=access-token api-key=server-api-key",
            BOLT11, sensitive_hash
        );
        let (base_url, _requests) = test_server(vec![
            serde_json::json!({
                "data": {
                    "lnUsdInvoiceFeeProbe": {"amount": 25, "errors": []}
                }
            }),
            serde_json::json!({
                "data": {
                    "lnInvoicePaymentSend": {
                        "status": "SUCCESS",
                        "errors": [{
                            "code": "UNEXPECTED_PROVIDER_ERROR",
                            "message": provider_message
                        }]
                    }
                }
            }),
        ])
        .await;
        let config = explicit_config(base_url, "USD");

        let error = pay_invoice(
            &config,
            PayInvoiceParams {
                invoice: BOLT11.to_string(),
                ..Default::default()
            },
        )
        .await
        .expect_err("unexpected payload errors must fail accepted payments");

        match error {
            ApiError::Nwc { code, message } => {
                assert_eq!(code, "PAYMENT_FAILED");
                assert!(message.contains("[flash]"));
                for sensitive in [
                    BOLT11,
                    sensitive_hash.as_str(),
                    "payment-secret",
                    "provider-preimage",
                    "access-token",
                    "server-api-key",
                ] {
                    assert!(
                        !message.contains(sensitive),
                        "sensitive value leaked: {sensitive}"
                    );
                }
            }
            other => panic!("expected sanitized NWC error, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn unsupported_wallet_fee_operation_fails_before_graphql() {
        let config = explicit_config("http://127.0.0.1:1/graphql".to_string(), "JMD");
        let error = pay_invoice(
            &config,
            PayInvoiceParams {
                invoice: BOLT11.to_string(),
                ..Default::default()
            },
        )
        .await
        .expect_err("JMD must not be inferred to use USD fee operations");

        assert!(matches!(
            error,
            ApiError::Nwc { ref code, ref message }
                if code == "NOT_IMPLEMENTED"
                    && message.contains("[flash]")
                    && message.contains("JMD")
        ));
    }

    #[tokio::test]
    async fn transaction_mode_preserves_preimage_and_btc_fee_conversion() {
        let (base_url, mut requests) = test_server(vec![
            serde_json::json!({
                "data": {"lnInvoiceFeeProbe": {"amount": 2, "errors": []}}
            }),
            serde_json::json!({
                "data": {
                    "lnInvoicePaymentSend": {
                        "status": "SUCCESS",
                        "errors": [],
                        "transaction": {
                            "settlementVia": {
                                "__typename": "SettlementViaLn",
                                "preImage": "settled-preimage"
                            }
                        }
                    }
                }
            }),
        ])
        .await;
        let mut config = explicit_config(base_url, "BTC");
        config.payment = GaloyPaymentConfig {
            response: GaloyPaymentResponse::TransactionWithPreimage,
            accepted_statuses: vec!["SUCCESS".to_string()],
            status_mapping: None,
            proof_unavailable_error_codes: vec![],
        };

        let response = pay_invoice(
            &config,
            PayInvoiceParams {
                invoice: BOLT11.to_string(),
                ..Default::default()
            },
        )
        .await
        .expect("payment should succeed");
        assert_eq!(response.fee_msats, 2_000);
        assert_eq!(response.preimage, "settled-preimage");
        requests.recv().await.expect("fee request");
        let payment_query = graphql_body(&requests.recv().await.expect("payment request"));
        assert!(payment_query["query"]
            .as_str()
            .expect("payment query")
            .contains("transaction {"));
    }

    #[tokio::test]
    async fn onchain_total_uses_provider_settled_amount() {
        let (base_url, mut requests) = test_server(vec![serde_json::json!({
            "data": {
                "onChainPaymentSend": {
                    "status": "SUCCESS",
                    "transaction": {
                        "id": "tx-1",
                        "settlementAmount": 9000,
                        "settlementCurrency": "BTC",
                        "settlementFee": 500,
                        "settlementVia": {
                            "__typename": "SettlementViaOnChain",
                            "transactionHash": "txid-1"
                        }
                    },
                    "errors": []
                }
            }
        })])
        .await;
        let mut config = explicit_config(base_url, "BTC");
        config.capabilities.onchain = true;
        let payment = pay_onchain(
            &config,
            OnchainTransaction {
                id: None,
                address: "bc1qexample".to_string(),
                amount_sats: 10_000,
                fee_sats: Some(500),
                total_amount_sats: Some(10_500),
                recipient_amount_sats: Some(10_000),
                fee_payer: OnchainFeePayer::Sender,
                fee: default_onchain_fee(),
                expires_at: None,
                estimated_delivery_seconds: None,
                raw: None,
            },
        )
        .await
        .expect("on-chain payment should succeed");

        assert_eq!(payment.amount_sats, 9_000);
        assert_eq!(payment.fee_sats, Some(500));
        assert_eq!(payment.total_amount_sats, Some(9_500));
        let request = requests.recv().await.expect("payment request");
        assert!(graphql_body(&request)["query"]
            .as_str()
            .expect("query")
            .contains("onChainPaymentSend"));
    }

    #[tokio::test]
    async fn transaction_fetch_count_is_saturating_and_bounded() {
        let (base_url, mut requests) = test_server(vec![serde_json::json!({
            "data": {
                "me": {
                    "defaultAccount": {
                        "transactions": {
                            "edges": [],
                            "pageInfo": {
                                "hasNextPage": false,
                                "hasPreviousPage": false,
                                "startCursor": null,
                                "endCursor": null
                            }
                        }
                    }
                }
            }
        })])
        .await;
        let mut config = explicit_config(base_url, "BTC");
        config.capabilities.transaction_history = true;

        let transactions = list_transactions(&config, i64::MAX, i64::MAX, None)
            .await
            .expect("bounded transaction query should succeed");
        assert!(transactions.is_empty());

        let request = requests.recv().await.expect("transaction request");
        assert_eq!(graphql_body(&request)["variables"]["first"], 1000);
    }

    #[tokio::test]
    async fn negative_polling_values_timeout_without_wrapping_or_hot_looping() {
        let mut config = explicit_config("http://127.0.0.1:1/graphql".to_string(), "BTC");
        config.capabilities.invoice_events = true;
        config.capabilities.transaction_lookup = true;
        let events = Arc::new(Mutex::new(Vec::new()));
        let captured_events = events.clone();

        poll_invoice_events(
            &config,
            OnInvoiceEventParams {
                payment_hash: Some("hash".to_string()),
                search: None,
                polling_delay_sec: -1,
                max_polling_sec: -1,
            },
            move |status, _| {
                captured_events.lock().expect("events lock").push(status);
            },
        )
        .await;

        assert_eq!(events.lock().expect("events lock").as_slice(), ["failure"]);
    }

    #[tokio::test]
    async fn disabled_capabilities_fail_before_graphql_and_sensitive_values_are_redacted() {
        let config = explicit_config("http://127.0.0.1:1/graphql".to_string(), "JMD");
        let lookup_error = lookup_invoice(&config, Some("a".repeat(64)), None, None, None)
            .await
            .expect_err("lookup should be disabled");
        match lookup_error {
            ApiError::Nwc { code, message } => {
                assert_eq!(code, "NOT_IMPLEMENTED");
                assert!(message.contains("[flash]"));
            }
            other => panic!("expected NWC error, got {other:?}"),
        }

        let history_error = list_transactions(&config, 0, 10, None)
            .await
            .expect_err("history should be disabled");
        assert!(matches!(
            history_error,
            ApiError::Nwc { ref code, .. } if code == "NOT_IMPLEMENTED"
        ));

        let onchain_error = prepare_onchain_transaction(
            &config,
            PrepareOnchainTransactionParams {
                address: "address".to_string(),
                amount_sats: 1,
                fee: None,
                fee_payer: None,
                description: None,
                idempotency_key: None,
            },
        )
        .await
        .expect_err("on-chain should be disabled");
        assert!(matches!(
            onchain_error,
            ApiError::Nwc { ref code, .. } if code == "NOT_IMPLEMENTED"
        ));

        let mut non_btc_onchain_config = config.clone();
        non_btc_onchain_config.capabilities.onchain = true;
        let non_btc_onchain_error = prepare_onchain_transaction(
            &non_btc_onchain_config,
            PrepareOnchainTransactionParams {
                address: "address".to_string(),
                amount_sats: 1,
                fee: None,
                fee_payer: None,
                description: None,
                idempotency_key: None,
            },
        )
        .await
        .expect_err("non-BTC on-chain should be unavailable");
        assert!(matches!(
            non_btc_onchain_error,
            ApiError::Nwc { ref code, .. } if code == "NOT_IMPLEMENTED"
        ));

        let diagnostic = sanitize_text(
            &config,
            serde_json::json!({
                "apiKey": "server-api-key",
                "access_token": "access-token",
                "paymentRequest": BOLT11,
                "paymentHash": "a".repeat(64),
                "paymentSecret": "secret",
                "preImage": "preimage"
            })
            .to_string(),
        );
        assert!(!diagnostic.contains("server-api-key"));
        assert!(!diagnostic.contains("access-token"));
        assert!(!diagnostic.contains(BOLT11));
        assert!(!diagnostic.contains(&"a".repeat(64)));
        assert!(!diagnostic.contains("preimage"));
    }
}
