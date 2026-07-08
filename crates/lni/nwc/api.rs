use crate::nwc::{NwcConfig, NwcLightningAddress};
use crate::types::{OnInvoiceEventCallback, OnInvoiceEventParams};
use crate::{
    ApiError, CreateInvoiceParams, ListTransactionsParams, NodeInfo, Offer, PayInvoiceParams,
    PayInvoiceResponse, Transaction,
};
use lightning_invoice::Bolt11Invoice;
use nwc::nostr::nips::nip47;
use nwc::prelude::*;
use sha2::{Digest, Sha256};
use std::future::Future;
use std::str::FromStr;
use std::time::Duration;

// Helper function to create NWC client
async fn create_nwc_client(config: &NwcConfig) -> Result<NWC, ApiError> {
    let uri = NostrWalletConnectURI::from_str(&config.nwc_uri).map_err(|e| ApiError::Api {
        reason: format!("Invalid NWC URI: {}", e),
    })?;

    let relay_opts = RelayOptions::default()
        .verify_subscriptions(true)
        .ban_relay_on_mismatch(true);
    let timeout = nwc_request_timeout(config);
    let opts = NostrWalletConnectOptions::default()
        .relay(relay_opts)
        .timeout(timeout);
    let nwc = NWC::with_opts(uri, opts);

    Ok(nwc)
}

fn nwc_request_timeout(config: &NwcConfig) -> Duration {
    Duration::from_secs(config.http_timeout.unwrap_or(60).max(1) as u64)
}

fn nwc_timeout_error(timeout: Duration) -> ApiError {
    ApiError::NetworkError(format!(
        "NWC request timed out after {} seconds",
        timeout.as_secs()
    ))
}

fn upstream_nwc_timeout_error() -> ApiError {
    ApiError::NetworkError("NWC request timed out".to_string())
}

async fn execute_nwc_request<T, F>(
    timeout: Duration,
    request: F,
    fallback_prefix: &str,
) -> Result<T, ApiError>
where
    F: Future<Output = Result<T, nwc::Error>>,
{
    match tokio::time::timeout(timeout, request).await {
        Ok(Ok(response)) => Ok(response),
        Ok(Err(error)) => Err(map_nwc_error(error, fallback_prefix)),
        Err(_) => Err(nwc_timeout_error(timeout)),
    }
}

fn is_network_error(error: &ApiError) -> bool {
    matches!(error, ApiError::NetworkError(_))
}

fn nwc_error_code_to_string(code: nip47::ErrorCode) -> String {
    serde_json::to_string(&code)
        .map(|value| value.trim_matches('"').to_string())
        .unwrap_or_else(|_| format!("{:?}", code))
}

fn map_nwc_error(error: nwc::Error, fallback_prefix: &str) -> ApiError {
    match error {
        nwc::Error::Timeout => upstream_nwc_timeout_error(),
        nwc::Error::NIP47(nip47::Error::ErrorCode(nwc_error)) => ApiError::Nwc {
            code: nwc_error_code_to_string(nwc_error.code),
            message: nwc_error.message,
        },
        other => ApiError::Api {
            reason: format!("{}: {}", fallback_prefix, other),
        },
    }
}

pub async fn get_info(config: NwcConfig) -> Result<NodeInfo, ApiError> {
    let nwc = create_nwc_client(&config).await?;
    let timeout = nwc_request_timeout(&config);

    // Get balance first
    let balance = execute_nwc_request(timeout, nwc.get_balance(), "Failed to get balance").await?;

    // Try to get more info using get_info method if available
    let info_result = execute_nwc_request(timeout, nwc.get_info(), "Failed to get info").await;

    match info_result {
        Ok(nwc_info) => {
            Ok(NodeInfo {
                alias: nwc_info.alias.unwrap_or_else(|| "NWC Node".to_string()),
                color: nwc_info.color.unwrap_or_default(),
                pubkey: nwc_info.pubkey.map(|pk| pk.to_string()).unwrap_or_else(|| {
                    // If no pubkey in get_info, try to extract from URI
                    config
                        .nwc_uri
                        .split("?")
                        .next()
                        .and_then(|part| part.strip_prefix("nostr+walletconnect://"))
                        .unwrap_or_default()
                        .to_string()
                }),
                network: nwc_info.network.unwrap_or_else(|| "mainnet".to_string()),
                block_height: nwc_info.block_height.unwrap_or(0) as i64,
                block_hash: nwc_info.block_hash.unwrap_or_default(),
                send_balance_msat: balance as i64,
                receive_balance_msat: 0, // NWC doesn't provide separate receive balance
                fee_credit_balance_msat: 0,
                unsettled_send_balance_msat: 0,
                unsettled_receive_balance_msat: 0,
                pending_open_send_balance: 0,
                pending_open_receive_balance: 0,
            })
        }
        Err(_) => {
            // Fallback: extract pubkey from NWC URI if get_info is not available
            let pubkey = config
                .nwc_uri
                .split("?")
                .next()
                .and_then(|part| part.strip_prefix("nostr+walletconnect://"))
                .unwrap_or_default()
                .to_string();

            Ok(NodeInfo {
                alias: "NWC Node".to_string(),
                color: "".to_string(),
                pubkey,
                network: "mainnet".to_string(),
                block_height: 0,
                block_hash: "".to_string(),
                send_balance_msat: balance as i64,
                receive_balance_msat: 0,
                fee_credit_balance_msat: 0,
                unsettled_send_balance_msat: 0,
                unsettled_receive_balance_msat: 0,
                pending_open_send_balance: 0,
                pending_open_receive_balance: 0,
            })
        }
    }
}

pub async fn get_permissions(config: NwcConfig) -> Result<crate::Permissions, ApiError> {
    let nwc = create_nwc_client(&config).await?;
    let timeout = nwc_request_timeout(&config);
    let info =
        execute_nwc_request(timeout, nwc.get_info(), "Failed to get NWC permissions").await?;

    if info.methods.is_empty() {
        Ok(crate::permissions::nwc_method_permissions())
    } else {
        Ok(crate::permissions::normalize_nwc_permissions(info.methods))
    }
}

pub fn lightning_address_from_nwc_uri(config: &NwcConfig) -> Result<String, ApiError> {
    let normalized_uri = config
        .nwc_uri
        .replace("nostrwalletconnect://", "http://")
        .replace("nostr+walletconnect://", "http://")
        .replace("nostrwalletconnect:", "http://")
        .replace("nostr+walletconnect:", "http://");
    let uri = reqwest::Url::parse(&normalized_uri).map_err(|e| ApiError::Api {
        reason: format!("Invalid NWC URI: {}", e),
    })?;
    let lightning_address = uri
        .query_pairs()
        .find(|(key, _)| key == "lud16")
        .map(|(_, value)| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            ApiError::InvalidInput("NWC URI does not include a lud16 Lightning Address".to_string())
        })?;

    match crate::lnurl::PaymentDestination::parse(&lightning_address)? {
        crate::lnurl::PaymentDestination::LightningAddress { .. } => Ok(lightning_address),
        _ => Err(ApiError::InvalidInput(
            "NWC lud16 value must be a Lightning Address".to_string(),
        )),
    }
}

pub async fn get_lightning_address(config: NwcConfig) -> Result<NwcLightningAddress, ApiError> {
    let lightning_address = lightning_address_from_nwc_uri(&config)?;
    let lnurl_verify_supported =
        crate::lnurl::lightning_address_lnurl_verify_supported(&lightning_address).await;

    Ok(NwcLightningAddress {
        lightning_address,
        lnurl_verify_supported,
    })
}

pub async fn create_invoice(
    config: NwcConfig,
    params: CreateInvoiceParams,
) -> Result<Transaction, ApiError> {
    let nwc = create_nwc_client(&config).await?;
    let timeout = nwc_request_timeout(&config);

    let request = MakeInvoiceRequest {
        amount: params.amount_msats.unwrap_or(0) as u64,
        description: params.description.clone(),
        description_hash: None,
        expiry: params.expiry.map(|e| e as u64),
    };

    let response = execute_nwc_request(
        timeout,
        nwc.make_invoice(request),
        "Failed to create invoice",
    )
    .await?;

    Ok(Transaction {
        type_: "incoming".to_string(),
        invoice: response.invoice,
        description: params.description.unwrap_or_default(),
        description_hash: "".to_string(),
        preimage: "".to_string(), // Not available in response
        payment_hash: response.payment_hash,
        amount_msats: params.amount_msats.unwrap_or(0),
        fees_paid: 0,
        created_at: 0, // Not available in response
        expires_at: 0, // Not available in response
        settled_at: 0, // Not settled yet
        payer_note: None,
        external_id: None,
    })
}

pub async fn pay_invoice(
    config: NwcConfig,
    params: PayInvoiceParams,
) -> Result<PayInvoiceResponse, ApiError> {
    let nwc = create_nwc_client(&config).await?;
    let timeout = nwc_request_timeout(&config);

    let request = PayInvoiceRequest::new(params.invoice);

    let response =
        execute_nwc_request(timeout, nwc.pay_invoice(request), "Failed to pay invoice").await?;

    // Compute payment hash from preimage (payment_hash = SHA256(preimage))
    let payment_hash = if !response.preimage.is_empty() {
        let preimage_bytes = hex::decode(&response.preimage).map_err(|e| ApiError::Api {
            reason: format!("Invalid preimage hex: {}", e),
        })?;
        let mut hasher = Sha256::new();
        hasher.update(preimage_bytes);
        hex::encode(hasher.finalize())
    } else {
        "".to_string()
    };

    Ok(PayInvoiceResponse {
        payment_hash,
        preimage: response.preimage,
        fee_msats: 0, // Not available in response
    })
}

pub async fn get_offer(_config: &NwcConfig, _search: Option<String>) -> Result<Offer, ApiError> {
    // NWC doesn't support offers/BOLT12 yet
    Err(ApiError::Api {
        reason: "NWC does not support offers (BOLT12) yet".to_string(),
    })
}

pub async fn list_offers(
    _config: &NwcConfig,
    _search: Option<String>,
) -> Result<Vec<Offer>, ApiError> {
    // NWC doesn't support offers/BOLT12 yet
    Err(ApiError::Api {
        reason: "NWC does not support offers (BOLT12) yet".to_string(),
    })
}

pub async fn pay_offer(
    _config: &NwcConfig,
    _offer: String,
    _amount_msats: i64,
    _payer_note: Option<String>,
) -> Result<PayInvoiceResponse, ApiError> {
    // NWC doesn't support offers/BOLT12 yet
    Err(ApiError::Api {
        reason: "NWC does not support offers (BOLT12) yet".to_string(),
    })
}

pub async fn lookup_invoice(
    config: NwcConfig,
    payment_hash: Option<String>,
    invoice: Option<String>,
) -> Result<Transaction, ApiError> {
    let lookup_payment_hash = payment_hash
        .clone()
        .or_else(|| invoice_payment_hash(invoice.as_deref()));
    let request = LookupInvoiceRequest {
        payment_hash: lookup_payment_hash.clone(),
        invoice: if lookup_payment_hash.is_some() {
            None
        } else {
            invoice.clone()
        },
    };
    let nwc = create_nwc_client(&config).await?;
    let timeout = nwc_request_timeout(&config);

    let response = match execute_nwc_request(
        timeout,
        nwc.lookup_invoice(request),
        "Failed to lookup invoice",
    )
    .await
    {
        Ok(response) => response,
        Err(error) => {
            if is_network_error(&error) {
                return Err(error);
            }

            if let Some(transaction) = lookup_invoice_from_transactions(
                config.clone(),
                lookup_payment_hash.as_deref(),
                invoice.as_deref(),
            )
            .await?
            {
                return Ok(transaction);
            }

            return Err(error);
        }
    };

    Ok(lookup_response_to_transaction(
        response,
        lookup_payment_hash.or(payment_hash),
    ))
}

async fn lookup_invoice_from_transactions(
    config: NwcConfig,
    payment_hash: Option<&str>,
    invoice: Option<&str>,
) -> Result<Option<Transaction>, ApiError> {
    if payment_hash.is_none() && invoice.is_none() {
        return Ok(None);
    }

    let params = ListTransactionsParams {
        from: 0,
        limit: 100,
        payment_hash: payment_hash.map(ToOwned::to_owned),
        search: invoice.map(ToOwned::to_owned),
        created_after: None,
        created_before: None,
    };
    let limit = normalized_limit(params.limit);
    let mut offset = 0;

    loop {
        let page = list_transactions_page_raw(&config, &params, Some(offset)).await?;
        let page_len = page.len();

        if let Some(transaction) = page
            .into_iter()
            .find(|transaction| transaction_matches_lookup(transaction, payment_hash, invoice))
        {
            return Ok(Some(transaction));
        }

        if page_len < limit {
            break;
        }

        offset += page_len as u64;
    }

    Ok(None)
}

fn lookup_response_to_transaction(
    response: LookupInvoiceResponse,
    requested_payment_hash: Option<String>,
) -> Transaction {
    let invoice = response.invoice.unwrap_or_default();
    let response_payment_hash = if response.payment_hash.is_empty() {
        None
    } else {
        Some(response.payment_hash)
    };
    let payment_hash = response_payment_hash
        .or(requested_payment_hash)
        .or_else(|| invoice_payment_hash(Some(&invoice)))
        .unwrap_or_default();

    Transaction {
        type_: match response.transaction_type {
            Some(TransactionType::Incoming) => "incoming".to_string(),
            Some(TransactionType::Outgoing) => "outgoing".to_string(),
            None => "unknown".to_string(),
        },
        invoice,
        description: response.description.unwrap_or_default(),
        description_hash: response.description_hash.unwrap_or_default(),
        preimage: response.preimage.unwrap_or_default(),
        payment_hash,
        amount_msats: response.amount as i64,
        fees_paid: response.fees_paid as i64,
        created_at: response.created_at.as_u64() as i64,
        expires_at: response
            .expires_at
            .map(|timestamp| timestamp.as_u64() as i64)
            .unwrap_or(0),
        settled_at: response
            .settled_at
            .map(|timestamp| timestamp.as_u64() as i64)
            .unwrap_or(0),
        payer_note: None,
        external_id: None,
    }
}

pub async fn list_transactions(
    config: NwcConfig,
    params: ListTransactionsParams,
) -> Result<Vec<Transaction>, ApiError> {
    list_transactions_page(&config, &params, None).await
}

async fn list_transactions_page(
    config: &NwcConfig,
    params: &ListTransactionsParams,
    offset: Option<u64>,
) -> Result<Vec<Transaction>, ApiError> {
    Ok(list_transactions_page_raw(config, params, offset)
        .await?
        .into_iter()
        .filter(|transaction| transaction_matches_params(transaction, params))
        .collect())
}

async fn list_transactions_page_raw(
    config: &NwcConfig,
    params: &ListTransactionsParams,
    offset: Option<u64>,
) -> Result<Vec<Transaction>, ApiError> {
    let request = list_transactions_request(params, offset);
    let nwc = create_nwc_client(config).await?;
    let timeout = nwc_request_timeout(config);
    let response = execute_nwc_request(
        timeout,
        nwc.list_transactions(request),
        "Failed to list transactions",
    )
    .await?;

    Ok(response
        .into_iter()
        .map(lookup_response_to_transaction_from_list)
        .collect())
}

fn list_transactions_request(
    params: &ListTransactionsParams,
    offset: Option<u64>,
) -> ListTransactionsRequest {
    let from = params.created_after.unwrap_or(params.from).max(0) as u64;
    let until = params
        .created_before
        .filter(|created_before| *created_before >= 0)
        .map(|created_before| Timestamp::from(created_before as u64));

    ListTransactionsRequest {
        from: Some(Timestamp::from(from)),
        until,
        limit: Some(normalized_limit(params.limit) as u64),
        offset,
        unpaid: None,
        transaction_type: None,
    }
}

fn lookup_response_to_transaction_from_list(response: LookupInvoiceResponse) -> Transaction {
    lookup_response_to_transaction(response, None)
}

fn normalized_limit(limit: i64) -> usize {
    limit.max(1) as usize
}

fn transaction_matches_lookup(
    transaction: &Transaction,
    payment_hash: Option<&str>,
    invoice: Option<&str>,
) -> bool {
    if let Some(target) = payment_hash {
        return transaction.payment_hash == target;
    }

    invoice
        .map(|target| transaction.invoice == target)
        .unwrap_or(false)
}

fn transaction_matches_params(transaction: &Transaction, params: &ListTransactionsParams) -> bool {
    if let Some(payment_hash) = params.payment_hash.as_deref() {
        if transaction.payment_hash != payment_hash {
            return false;
        }
    }

    if let Some(search) = params.search.as_deref() {
        let search = search.to_lowercase();
        let matches_search = transaction.payment_hash.to_lowercase().contains(&search)
            || transaction.invoice.to_lowercase().contains(&search)
            || transaction.description.to_lowercase().contains(&search)
            || transaction
                .payer_note
                .as_deref()
                .unwrap_or_default()
                .to_lowercase()
                .contains(&search);

        if !matches_search {
            return false;
        }
    }

    if let Some(created_after) = params.created_after {
        if transaction.created_at < created_after {
            return false;
        }
    }

    if let Some(created_before) = params.created_before {
        if transaction.created_at > created_before {
            return false;
        }
    }

    true
}

fn invoice_payment_hash(invoice: Option<&str>) -> Option<String> {
    invoice
        .and_then(|invoice| Bolt11Invoice::from_str(invoice).ok())
        .map(|invoice| format!("{:x}", invoice.payment_hash()))
}

pub async fn decode(str: String) -> Result<String, ApiError> {
    crate::utils::decode_bolt11(str)
}

pub async fn decode_offer(offer: String) -> Result<String, ApiError> {
    crate::utils::decode_offer(offer)
}

// Core logic shared with other implementations - processes lookup result and determines status
fn process_invoice_lookup_result(
    transaction_result: Result<Transaction, ApiError>,
) -> (String, Option<Transaction>) {
    match transaction_result {
        Ok(transaction) => {
            if transaction.settled_at > 0 {
                ("settled".to_string(), Some(transaction))
            } else {
                ("pending".to_string(), Some(transaction))
            }
        }
        Err(_) => ("pending".to_string(), None),
    }
}

// Core logic shared with other implementations - determines if we should continue polling
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

// Async polling logic following the same pattern as LND and Phoenix
pub async fn poll_invoice_events<F>(
    config: &NwcConfig,
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

// Async version for direct async use
pub async fn on_invoice_events(
    config: NwcConfig,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_upstream_list_transaction_response() {
        let transaction = lookup_response_to_transaction_from_list(LookupInvoiceResponse {
            transaction_type: Some(TransactionType::Incoming),
            invoice: Some("invoice".to_string()),
            description: Some("description".to_string()),
            description_hash: None,
            preimage: None,
            payment_hash: "hash".to_string(),
            amount: 1000,
            fees_paid: 25,
            created_at: Timestamp::from(1777416147),
            expires_at: None,
            settled_at: Some(Timestamp::from(1777416150)),
            metadata: None,
        });

        assert_eq!(transaction.type_, "incoming");
        assert_eq!(transaction.payment_hash, "hash");
        assert_eq!(transaction.amount_msats, 1000);
        assert_eq!(transaction.fees_paid, 25);
        assert_eq!(transaction.created_at, 1777416147);
        assert_eq!(transaction.settled_at, 1777416150);
    }

    #[test]
    fn invoice_lookup_errors_are_pending_while_polling() {
        let (status, transaction) = process_invoice_lookup_result(Err(ApiError::Api {
            reason: "temporary lookup failure".to_string(),
        }));

        assert_eq!(status, "pending");
        assert!(transaction.is_none());
    }

    #[test]
    fn maps_nip47_error_codes_to_structured_api_errors() {
        let error = nwc::Error::NIP47(nip47::Error::ErrorCode(nip47::NIP47Error {
            code: nip47::ErrorCode::QuotaExceeded,
            message: "quota spent".to_string(),
        }));

        match map_nwc_error(error, "Failed to pay invoice") {
            ApiError::Nwc { code, message } => {
                assert_eq!(code, "QUOTA_EXCEEDED");
                assert_eq!(message, "quota spent");
            }
            other => panic!("expected structured NWC error, got {:?}", other),
        }
    }

    #[test]
    fn maps_upstream_nwc_timeout_to_network_error() {
        match map_nwc_error(nwc::Error::Timeout, "Failed to pay invoice") {
            ApiError::NetworkError(message) => {
                assert!(message.contains("timed out"));
            }
            other => panic!("expected network timeout error, got {:?}", other),
        }
    }

    #[test]
    fn normalizes_nwc_request_timeout_to_at_least_one_second() {
        let config = NwcConfig {
            nwc_uri: String::new(),
            socks5_proxy: None,
            accept_invalid_certs: None,
            http_timeout: Some(0),
        };

        assert_eq!(nwc_request_timeout(&config), Duration::from_secs(1));
    }

    #[tokio::test]
    async fn execute_nwc_request_maps_elapsed_timeout_to_network_error() {
        let result = execute_nwc_request(
            Duration::from_millis(1),
            async {
                tokio::time::sleep(Duration::from_secs(60)).await;
                Ok::<(), nwc::Error>(())
            },
            "Failed to pay invoice",
        )
        .await;

        match result {
            Err(ApiError::NetworkError(message)) => {
                assert!(message.contains("timed out"));
            }
            other => panic!("expected elapsed timeout error, got {:?}", other),
        }
    }

    #[test]
    fn list_transactions_request_sets_offset_and_bounds() {
        let request = list_transactions_request(
            &ListTransactionsParams {
                from: 10,
                limit: 25,
                payment_hash: None,
                search: None,
                created_after: Some(20),
                created_before: Some(30),
            },
            Some(50),
        );

        assert_eq!(request.from.map(|timestamp| timestamp.as_u64()), Some(20));
        assert_eq!(request.until.map(|timestamp| timestamp.as_u64()), Some(30));
        assert_eq!(request.limit, Some(25));
        assert_eq!(request.offset, Some(50));
    }

    #[test]
    fn transaction_lookup_match_prefers_payment_hash_over_invoice() {
        let transaction = Transaction {
            type_: "incoming".to_string(),
            payment_hash: "hash".to_string(),
            invoice: "invoice".to_string(),
            description: String::new(),
            description_hash: String::new(),
            preimage: String::new(),
            amount_msats: 0,
            fees_paid: 0,
            created_at: 0,
            expires_at: 0,
            settled_at: 0,
            payer_note: None,
            external_id: None,
        };

        assert!(transaction_matches_lookup(&transaction, Some("hash"), None));
        assert!(!transaction_matches_lookup(
            &transaction,
            Some("other"),
            Some("invoice")
        ));
        assert!(transaction_matches_lookup(
            &transaction,
            None,
            Some("invoice")
        ));
        assert!(!transaction_matches_lookup(
            &transaction,
            Some("other"),
            Some("other")
        ));
    }
}
