use crate::{ApiError, CreateInvoiceParams, PayInvoiceParams, Offer, Transaction, PayInvoiceResponse, NodeInfo, ListTransactionsParams};
use crate::nwc::NwcConfig;
use crate::nwc::types::{
    ListTransactionsResponse as LocalListTransactionsResponse,
    LookupInvoiceResponse as LocalLookupInvoiceResponse, NwcTransaction,
};
use crate::types::{OnInvoiceEventParams, OnInvoiceEventCallback};
use lightning_invoice::Bolt11Invoice;
use nwc::prelude::*;
use serde::Deserialize;
use std::str::FromStr;
use std::time::Duration;
use sha2::{Digest, Sha256};

// Helper function to create NWC client
async fn create_nwc_client(config: &NwcConfig) -> Result<NWC, ApiError> {
    let uri = NostrWalletConnectURI::from_str(&config.nwc_uri)
        .map_err(|e| ApiError::Api { reason: format!("Invalid NWC URI: {}", e) })?;
    
    let opts = NostrWalletConnectOptions::default();
    let nwc = NWC::with_opts(uri, opts);
    
    Ok(nwc)
}

pub async fn get_info(config: NwcConfig) -> Result<NodeInfo, ApiError> {
        let nwc = create_nwc_client(&config).await?;
        
        // Get balance first
        let balance = nwc.get_balance().await
            .map_err(|e| ApiError::Api { reason: format!("Failed to get balance: {}", e) })?;
        
        // Try to get more info using get_info method if available
        let info_result = nwc.get_info().await;
        
        match info_result {
            Ok(nwc_info) => {
                Ok(NodeInfo {
                    alias: nwc_info.alias.unwrap_or_else(|| "NWC Node".to_string()),
                    color: nwc_info.color.unwrap_or_default(),
                    pubkey: nwc_info.pubkey.map(|pk| pk.to_string()).unwrap_or_else(|| {
                        // If no pubkey in get_info, try to extract from URI
                        config.nwc_uri.split("?").next()
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
                let pubkey = config.nwc_uri.split("?").next()
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
    let info = nwc.get_info().await.map_err(|e| ApiError::Api {
        reason: format!("Failed to get NWC permissions: {}", e),
    })?;

    if info.methods.is_empty() {
        Ok(crate::permissions::nwc_method_permissions())
    } else {
        Ok(crate::permissions::normalize_nwc_permissions(info.methods))
    }
}

pub async fn create_invoice(config: NwcConfig, params: CreateInvoiceParams) -> Result<Transaction, ApiError> {
    let nwc = create_nwc_client(&config).await?;
    
    let request = MakeInvoiceRequest {
        amount: params.amount_msats.unwrap_or(0) as u64,
        description: params.description.clone(),
        description_hash: None,
        expiry: params.expiry.map(|e| e as u64),
    };
    
    let response = nwc.make_invoice(request).await
        .map_err(|e| ApiError::Api { reason: format!("Failed to create invoice: {}", e) })?;
    
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

pub async fn pay_invoice(config: NwcConfig, params: PayInvoiceParams) -> Result<PayInvoiceResponse, ApiError> {
    let nwc = create_nwc_client(&config).await?;
    
    let request = PayInvoiceRequest::new(params.invoice);
    
    let response = nwc.pay_invoice(request).await
        .map_err(|e| ApiError::Api { reason: format!("Failed to pay invoice: {}", e) })?;
    
    // Compute payment hash from preimage (payment_hash = SHA256(preimage))
    let payment_hash = if !response.preimage.is_empty() {
        let preimage_bytes = hex::decode(&response.preimage)
            .map_err(|e| ApiError::Api { reason: format!("Invalid preimage hex: {}", e) })?;
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
    Err(ApiError::Api { reason: "NWC does not support offers (BOLT12) yet".to_string() })
}

pub async fn list_offers(_config: &NwcConfig, _search: Option<String>) -> Result<Vec<Offer>, ApiError> {
    // NWC doesn't support offers/BOLT12 yet
    Err(ApiError::Api { reason: "NWC does not support offers (BOLT12) yet".to_string() })
}

pub async fn pay_offer(
    _config: &NwcConfig,
    _offer: String,
    _amount_msats: i64,
    _payer_note: Option<String>,
) -> Result<PayInvoiceResponse, ApiError> {
    // NWC doesn't support offers/BOLT12 yet
    Err(ApiError::Api { reason: "NWC does not support offers (BOLT12) yet".to_string() })
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
        invoice: if lookup_payment_hash.is_some() { None } else { invoice.clone() },
    };

    let response = match send_lookup_invoice_request(&config, request).await {
        Ok(response) => response,
        Err(error) => {
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

    Ok(lookup_response_to_transaction(response, lookup_payment_hash.or(payment_hash)))
}

async fn lookup_invoice_from_transactions(
    config: NwcConfig,
    payment_hash: Option<&str>,
    invoice: Option<&str>,
) -> Result<Option<Transaction>, ApiError> {
    if payment_hash.is_none() && invoice.is_none() {
        return Ok(None);
    }

    let transactions = list_transactions(
        config,
        ListTransactionsParams {
            from: 0,
            limit: 100,
            payment_hash: None,
            search: None,
            created_after: None,
            created_before: None,
        },
    )
    .await?;

    Ok(transactions.into_iter().find(|transaction| {
        payment_hash
            .map(|target| transaction.payment_hash == target)
            .unwrap_or(false)
            || invoice
                .map(|target| transaction.invoice == target)
                .unwrap_or(false)
    }))
}

fn lookup_response_to_transaction(
    response: LocalLookupInvoiceResponse,
    requested_payment_hash: Option<String>,
) -> Transaction {
    let invoice = response.invoice.unwrap_or_default();
    let payment_hash = response
        .payment_hash
        .or(requested_payment_hash)
        .or_else(|| invoice_payment_hash(Some(&invoice)))
        .unwrap_or_default();

    Transaction {
        type_: response.type_.unwrap_or_else(|| "unknown".to_string()),
        invoice,
        description: response.description.unwrap_or_default(),
        description_hash: response.description_hash.unwrap_or_default(),
        preimage: response.preimage.unwrap_or_default(),
        payment_hash,
        amount_msats: response.amount,
        fees_paid: response.fees_paid,
        created_at: response.created_at,
        expires_at: response.expires_at.unwrap_or(0),
        settled_at: response.settled_at.unwrap_or(0),
        payer_note: None,
        external_id: None,
    }
}

pub async fn list_transactions(config: NwcConfig, params: ListTransactionsParams) -> Result<Vec<Transaction>, ApiError> {
    let request = ListTransactionsRequest {
        from: Some(Timestamp::from(params.from as u64)),
        until: None,
        limit: Some(params.limit as u64),
        offset: None,
        unpaid: None,
        transaction_type: None,
    };

    let response = send_list_transactions_request(&config, request).await?;

    let mut transactions = Vec::new();
    for tx in response.transactions {
        transactions.push(nwc_transaction_to_transaction(tx));
    }

    Ok(transactions)
}

fn nwc_transaction_to_transaction(tx: NwcTransaction) -> Transaction {
    let invoice = tx.invoice.unwrap_or_default();
    let payment_hash = tx
        .payment_hash
        .or_else(|| invoice_payment_hash(Some(&invoice)))
        .unwrap_or_default();

    Transaction {
        type_: tx.type_,
        invoice,
        description: tx.description.unwrap_or_default(),
        description_hash: tx.description_hash.unwrap_or_default(),
        preimage: tx.preimage.unwrap_or_default(),
        payment_hash,
        amount_msats: tx.amount,
        fees_paid: tx.fees_paid,
        created_at: tx.created_at,
        expires_at: tx.expires_at.unwrap_or(0),
        settled_at: tx.settled_at.unwrap_or(0),
        payer_note: None,
        external_id: None,
    }
}

#[derive(Debug, Deserialize)]
struct RawNwcResponse {
    result_type: String,
    error: Option<RawNwcError>,
    result: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct RawNwcError {
    code: String,
    message: String,
}

async fn send_list_transactions_request(
    config: &NwcConfig,
    request: ListTransactionsRequest,
) -> Result<LocalListTransactionsResponse, ApiError> {
    let result = send_nwc_request_result(
        config,
        Request::list_transactions(request),
        "list_transactions",
        "list transactions",
    )
    .await?;

    serde_json::from_value(result).map_err(|e| ApiError::Api {
        reason: format!("Failed to deserialize list_transactions result: {}", e),
    })
}

async fn send_lookup_invoice_request(
    config: &NwcConfig,
    request: LookupInvoiceRequest,
) -> Result<LocalLookupInvoiceResponse, ApiError> {
    let result = send_nwc_request_result(
        config,
        Request::lookup_invoice(request),
        "lookup_invoice",
        "lookup invoice",
    )
    .await?;

    serde_json::from_value(result).map_err(|e| ApiError::Api {
        reason: format!("Failed to deserialize lookup_invoice result: {}", e),
    })
}

async fn send_nwc_request_result(
    config: &NwcConfig,
    req: Request,
    expected_result_type: &str,
    operation: &str,
) -> Result<serde_json::Value, ApiError> {
    let uri = NostrWalletConnectURI::from_str(&config.nwc_uri)
        .map_err(|e| ApiError::Api { reason: format!("Invalid NWC URI: {}", e) })?;
    let pool = RelayPool::default();

    for url in uri.relays.iter() {
        pool.add_relay(url, RelayOptions::default())
            .await
            .map_err(|e| ApiError::Api { reason: format!("Failed to add NWC relay: {}", e) })?;
    }

    pool.connect().await;

    let event = req
        .to_event(&uri)
        .map_err(|e| ApiError::Api { reason: format!("Failed to create NWC request: {}", e) })?;
    let filter = Filter::new()
        .author(uri.public_key)
        .kind(Kind::WalletConnectResponse)
        .event(event.id);
    let timeout = Duration::from_secs(config.http_timeout.unwrap_or(60).max(1) as u64);
    let mut stream = pool
        .stream_events(filter, timeout, ReqExitPolicy::WaitForEvents(5))
        .await
        .map_err(|e| ApiError::Api { reason: format!("Failed to subscribe for NWC response: {}", e) })?;

    pool.send_event(&event)
        .await
        .map_err(|e| ApiError::Api { reason: format!("Failed to send NWC request: {}", e) })?;

    let mut last_error = None;

    while let Some(received_event) = stream.next().await {
        let decrypted = match nip04::decrypt(&uri.secret, &received_event.pubkey, &received_event.content) {
            Ok(decrypted) => decrypted,
            Err(e) => {
                last_error = Some(format!("Failed to decrypt NWC response: {}", e));
                continue;
            }
        };

        let response: RawNwcResponse = match serde_json::from_str(&decrypted) {
            Ok(response) => response,
            Err(e) => {
                last_error = Some(format!("Failed to deserialize NWC response: {}", e));
                continue;
            }
        };

        if response.result_type != expected_result_type {
            last_error = Some(format!("Unexpected NWC response type: {}", response.result_type));
            continue;
        }

        if let Some(error) = response.error {
            return Err(ApiError::Api {
                reason: format!("NWC error {}: {}", error.code, error.message),
            });
        }

        return response.result.ok_or_else(|| ApiError::Api {
            reason: format!("Missing {} result", expected_result_type),
        });
    }

    Err(ApiError::Api {
        reason: last_error.unwrap_or_else(|| format!("Failed to {}: no NWC response received", operation)),
    })
}

fn invoice_payment_hash(invoice: Option<&str>) -> Option<String> {
    invoice
        .and_then(|invoice| Bolt11Invoice::from_str(invoice).ok())
        .map(|invoice| format!("{:x}", invoice.payment_hash()))
}

pub async fn decode(_config: NwcConfig, str: String) -> Result<String, ApiError> {
    // NWC doesn't have a decode method, just return the input
    Ok(str)
}

// Core logic shared with other implementations - processes lookup result and determines status
fn process_invoice_lookup_result(transaction_result: Result<Transaction, ApiError>) -> (String, Option<Transaction>) {
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
    fn deserializes_list_transactions_without_payment_hash() {
        let response: LocalListTransactionsResponse = serde_json::from_value(serde_json::json!({
            "transactions": [{
                "type": "incoming",
                "amount": 1000,
                "fees_paid": 0,
                "created_at": 1777416147
            }]
        }))
        .expect("NWC transactions without payment_hash should deserialize");

        let transaction = nwc_transaction_to_transaction(
            response.transactions.into_iter().next().expect("transaction"),
        );

        assert_eq!(transaction.type_, "incoming");
        assert_eq!(transaction.payment_hash, "");
        assert_eq!(transaction.amount_msats, 1000);
    }

    #[test]
    fn invoice_lookup_errors_are_pending_while_polling() {
        let (status, transaction) = process_invoice_lookup_result(Err(ApiError::Api {
            reason: "temporary lookup failure".to_string(),
        }));

        assert_eq!(status, "pending");
        assert!(transaction.is_none());
    }
}
