use std::collections::HashMap;
use std::sync::Arc;

use breez_sdk_spark::{
    BreezSdk, EventListener, GetInfoRequest, GetPaymentRequest, ListPaymentsRequest,
    PaymentDetails, PaymentStatus, PaymentType, PrepareSendPaymentRequest, ReceivePaymentMethod,
    ReceivePaymentRequest, SdkEvent, SendPaymentRequest,
};
use tokio::sync::RwLock;

use crate::types::NodeInfo;
use crate::{
    ApiError, CreateInvoiceParams, InvoiceType, Offer, OnInvoiceEventCallback,
    OnInvoiceEventParams, PayInvoiceParams, PayInvoiceResponse, Transaction,
};

// ── Helpers ──────────────────────────────────────────────────────────

struct PaymentInfo {
    invoice: String,
    payment_hash: String,
    preimage: String,
    description: String,
}

/// Extract invoice/hash/preimage/description from Lightning or Spark payment details.
/// Returns None for non-Lightning/Spark payment types (Deposit, Withdraw, Token, etc.)
fn extract_payment_info(payment: &breez_sdk_spark::Payment) -> Option<PaymentInfo> {
    match &payment.details {
        Some(PaymentDetails::Lightning {
            invoice,
            payment_hash,
            preimage,
            description,
            ..
        }) => Some(PaymentInfo {
            invoice: invoice.clone(),
            payment_hash: payment_hash.clone(),
            preimage: preimage.clone().unwrap_or_default(),
            description: description.clone().unwrap_or_default(),
        }),
        Some(PaymentDetails::Spark {
            invoice_details,
            htlc_details,
            ..
        }) => {
            let invoice = invoice_details
                .as_ref()
                .map(|d| d.invoice.clone())
                .unwrap_or_default();
            let description = invoice_details
                .as_ref()
                .and_then(|d| d.description.clone())
                .unwrap_or_default();
            let (payment_hash, preimage) = if let Some(htlc) = htlc_details {
                (
                    htlc.payment_hash.clone(),
                    htlc.preimage.clone().unwrap_or_default(),
                )
            } else {
                ("".to_string(), "".to_string())
            };
            Some(PaymentInfo {
                invoice,
                payment_hash,
                preimage,
                description,
            })
        }
        _ => None,
    }
}

/// Convert a Breez Payment to an LNI Transaction.
/// Returns None for payment types we can't map (non-Lightning/Spark without extract_payment_info).
fn payment_to_transaction(payment: &breez_sdk_spark::Payment) -> Option<Transaction> {
    let (invoice, payment_hash, preimage, description) = match extract_payment_info(payment) {
        Some(info) => (info.invoice, info.payment_hash, info.preimage, info.description),
        None => {
            // Handle on-chain types
            match &payment.details {
                Some(PaymentDetails::Deposit { tx_id }) => {
                    (tx_id.clone(), "".to_string(), "".to_string(), "Deposit".to_string())
                }
                Some(PaymentDetails::Withdraw { tx_id }) => {
                    (tx_id.clone(), "".to_string(), "".to_string(), "Withdraw".to_string())
                }
                Some(PaymentDetails::Token { tx_hash, .. }) => {
                    (tx_hash.clone(), "".to_string(), "".to_string(), "Token Transfer".to_string())
                }
                _ => return None,
            }
        }
    };

    Some(Transaction {
        type_: match payment.payment_type {
            PaymentType::Send => "outgoing".to_string(),
            PaymentType::Receive => "incoming".to_string(),
        },
        invoice,
        preimage,
        payment_hash,
        amount_msats: (payment.amount as i64) * 1000,
        fees_paid: (payment.fees as i64) * 1000,
        created_at: payment.timestamp as i64,
        expires_at: 0,
        settled_at: if payment.status == PaymentStatus::Completed {
            payment.timestamp as i64
        } else {
            0
        },
        description,
        description_hash: "".to_string(),
        payer_note: None,
        external_id: Some(payment.id.clone()),
    })
}

/// Populate the cache for a payment (paymentHash → paymentId)
async fn cache_payment(
    payment: &breez_sdk_spark::Payment,
    cache: &RwLock<HashMap<String, String>>,
) {
    if let Some(info) = extract_payment_info(payment) {
        if !info.payment_hash.is_empty() {
            cache
                .write()
                .await
                .insert(info.payment_hash, payment.id.clone());
        }
    }
}

// ── Public API ───────────────────────────────────────────────────────

/// Get node info from Spark SDK
pub async fn get_info(sdk: Arc<BreezSdk>, network: &str) -> Result<NodeInfo, ApiError> {
    let info = sdk
        .get_info(GetInfoRequest {
            ensure_synced: Some(true),
        })
        .await
        .map_err(|e| ApiError::Api {
            reason: e.to_string(),
        })?;

    Ok(NodeInfo {
        alias: "Spark Node".to_string(),
        color: "".to_string(),
        pubkey: "".to_string(),
        network: network.to_string(),
        block_height: 0,
        block_hash: "".to_string(),
        send_balance_msat: (info.balance_sats as i64) * 1000,
        receive_balance_msat: 0, // Spark doesn't have receive capacity limits
        fee_credit_balance_msat: 0,
        unsettled_send_balance_msat: 0,
        unsettled_receive_balance_msat: 0,
        pending_open_send_balance: 0,
        pending_open_receive_balance: 0,
    })
}

/// Create an invoice using Spark SDK
pub async fn create_invoice(
    sdk: Arc<BreezSdk>,
    invoice_params: CreateInvoiceParams,
) -> Result<Transaction, ApiError> {
    match invoice_params.get_invoice_type() {
        InvoiceType::Bolt11 => {
            let response = sdk
                .receive_payment(ReceivePaymentRequest {
                    payment_method: ReceivePaymentMethod::Bolt11Invoice {
                        description: invoice_params.description.clone().unwrap_or_default(),
                        amount_sats: invoice_params.amount_msats.map(|m| (m / 1000) as u64),
                    },
                })
                .await
                .map_err(|e| ApiError::Api {
                    reason: e.to_string(),
                })?;

            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64;

            Ok(Transaction {
                type_: "incoming".to_string(),
                invoice: response.payment_request.clone(),
                preimage: "".to_string(),
                payment_hash: extract_payment_hash(&response.payment_request).unwrap_or_default(),
                amount_msats: invoice_params.amount_msats.unwrap_or(0),
                fees_paid: response.fee as i64,
                created_at: now,
                expires_at: now + invoice_params.expiry.unwrap_or(86400),
                settled_at: 0,
                description: invoice_params.description.unwrap_or_default(),
                description_hash: invoice_params.description_hash.unwrap_or_default(),
                payer_note: None,
                external_id: None,
            })
        }
        InvoiceType::Bolt12 => Err(ApiError::Api {
            reason: "Bolt12 not yet implemented for Spark".to_string(),
        }),
    }
}

/// Pay an invoice using Spark SDK
pub async fn pay_invoice(
    sdk: Arc<BreezSdk>,
    invoice_params: PayInvoiceParams,
) -> Result<PayInvoiceResponse, ApiError> {
    // Prepare the payment first
    let prepare_response = sdk
        .prepare_send_payment(PrepareSendPaymentRequest {
            payment_request: invoice_params.invoice.clone(),
            amount: invoice_params.amount_msats.map(|m| (m / 1000) as u128),
            token_identifier: None,
        })
        .await
        .map_err(|e| ApiError::Api {
            reason: e.to_string(),
        })?;

    // Execute the payment
    let response = sdk
        .send_payment(SendPaymentRequest {
            prepare_response,
            options: None,
            idempotency_key: None,
        })
        .await
        .map_err(|e| ApiError::Api {
            reason: e.to_string(),
        })?;

    let (payment_hash, preimage) = match response.payment.details {
        Some(PaymentDetails::Lightning {
            payment_hash,
            preimage,
            ..
        }) => (payment_hash, preimage.unwrap_or_default()),
        Some(PaymentDetails::Spark { htlc_details, .. }) => {
            if let Some(htlc) = htlc_details {
                (htlc.payment_hash, htlc.preimage.unwrap_or_default())
            } else {
                ("".to_string(), "".to_string())
            }
        }
        _ => ("".to_string(), "".to_string()),
    };

    Ok(PayInvoiceResponse {
        payment_hash,
        preimage,
        fee_msats: (response.payment.fees as i64) * 1000,
    })
}

// ── Lookup ───────────────────────────────────────────────────────────

/// O(1) lookup by payment ID via get_payment
async fn lookup_by_payment_id(
    sdk: &BreezSdk,
    payment_id: &str,
) -> Result<Option<Transaction>, ApiError> {
    match sdk
        .get_payment(GetPaymentRequest {
            payment_id: payment_id.to_string(),
        })
        .await
    {
        Ok(resp) => Ok(payment_to_transaction(&resp.payment)),
        Err(_) => Ok(None),
    }
}

/// Scan payments within a time window, looking for a specific payment hash.
/// Populates the cache for all payments seen.
async fn scan_for_hash(
    sdk: &BreezSdk,
    target_hash: &str,
    from_timestamp: Option<u64>,
    cache: &RwLock<HashMap<String, String>>,
) -> Result<Option<Transaction>, ApiError> {
    const PAGE_SIZE: u32 = 100;
    let mut offset: u32 = 0;

    loop {
        let payments = sdk
            .list_payments(ListPaymentsRequest {
                offset: Some(offset),
                limit: Some(PAGE_SIZE),
                from_timestamp,
                ..Default::default()
            })
            .await
            .map_err(|e| ApiError::Api {
                reason: e.to_string(),
            })?;

        if payments.payments.is_empty() {
            break;
        }

        for payment in &payments.payments {
            cache_payment(payment, cache).await;

            if let Some(info) = extract_payment_info(payment) {
                if info.payment_hash == target_hash {
                    return Ok(payment_to_transaction(payment));
                }
            }
        }

        if (payments.payments.len() as u32) < PAGE_SIZE {
            break;
        }

        offset += PAGE_SIZE;
    }

    Ok(None)
}

/// Lookup an invoice by payment hash using tiered strategy:
/// 1. Cache hit → O(1) get_payment
/// 2. 1-hour window scan
/// 3. 24-hour window scan
/// 4. Full scan (fallback)
pub async fn lookup_invoice(
    sdk: Arc<BreezSdk>,
    payment_hash: Option<String>,
    _from: Option<i64>,
    _limit: Option<i64>,
    _search: Option<String>,
    cache: Arc<RwLock<HashMap<String, String>>>,
) -> Result<Transaction, ApiError> {
    let target_hash = payment_hash.ok_or_else(|| ApiError::Api {
        reason: "Payment hash is required for lookup".to_string(),
    })?;

    if target_hash.is_empty() {
        return Err(ApiError::Api {
            reason: "Payment hash cannot be empty".to_string(),
        });
    }

    // Tier 1: Cache hit → O(1) lookup
    {
        let cached_id = cache.read().await.get(&target_hash).cloned();
        if let Some(payment_id) = cached_id {
            if let Some(txn) = lookup_by_payment_id(&sdk, &payment_id).await? {
                // Verify the hash still matches (stale eviction)
                if txn.payment_hash == target_hash {
                    return Ok(txn);
                }
            }
            // Stale entry — evict
            cache.write().await.remove(&target_hash);
        }
    }

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    // Tier 2: 1-hour window
    if let Some(txn) = scan_for_hash(&sdk, &target_hash, Some(now - 3600), &cache).await? {
        return Ok(txn);
    }

    // Tier 3: 24-hour window
    if let Some(txn) = scan_for_hash(&sdk, &target_hash, Some(now - 86400), &cache).await? {
        return Ok(txn);
    }

    // Tier 4: Full scan (no time filter)
    if let Some(txn) = scan_for_hash(&sdk, &target_hash, None, &cache).await? {
        return Ok(txn);
    }

    Err(ApiError::Api {
        reason: format!("Invoice not found for payment hash: {}", target_hash),
    })
}

// ── List Transactions ────────────────────────────────────────────────

/// List transactions from Spark SDK with optional date filters
pub async fn list_transactions(
    sdk: Arc<BreezSdk>,
    from: i64,
    limit: i64,
    _search: Option<String>,
    created_after: Option<i64>,
    created_before: Option<i64>,
    cache: Arc<RwLock<HashMap<String, String>>>,
) -> Result<Vec<Transaction>, ApiError> {
    let offset = u32::try_from(from.max(0)).unwrap_or(u32::MAX);
    let limit = u32::try_from(limit.max(0)).unwrap_or(u32::MAX);
    let from_timestamp = created_after.and_then(|t| u64::try_from(t).ok());
    let to_timestamp = created_before.and_then(|t| u64::try_from(t).ok());

    let payments = sdk
        .list_payments(ListPaymentsRequest {
            offset: Some(offset),
            limit: Some(limit),
            from_timestamp,
            to_timestamp,
            ..Default::default()
        })
        .await
        .map_err(|e| ApiError::Api {
            reason: e.to_string(),
        })?;

    let mut transactions = Vec::new();

    for payment in &payments.payments {
        cache_payment(payment, &cache).await;

        if let Some(txn) = payment_to_transaction(payment) {
            transactions.push(txn);
        }
    }

    Ok(transactions)
}

// ── Decode / Offers ──────────────────────────────────────────────────

/// Decode a payment request using the SDK's parse functionality
pub async fn decode(sdk: Arc<BreezSdk>, input: String) -> Result<String, ApiError> {
    match sdk.parse(&input).await {
        Ok(parsed) => Ok(format!("{:?}", parsed)),
        Err(e) => Err(ApiError::Api {
            reason: format!("Failed to decode input: {}", e),
        }),
    }
}

/// Get offer (not implemented for Spark yet)
pub fn get_offer(_search: Option<String>) -> Result<Offer, ApiError> {
    Err(ApiError::Api {
        reason: "Bolt12 offers not yet implemented for Spark".to_string(),
    })
}

/// List offers (not implemented for Spark yet)
pub fn list_offers(_search: Option<String>) -> Result<Vec<Offer>, ApiError> {
    Err(ApiError::Api {
        reason: "Bolt12 offers not yet implemented for Spark".to_string(),
    })
}

/// Pay offer (not implemented for Spark yet)
pub fn pay_offer(
    _offer: String,
    _amount_msats: i64,
    _payer_note: Option<String>,
) -> Result<PayInvoiceResponse, ApiError> {
    Err(ApiError::Api {
        reason: "Bolt12 offers not yet implemented for Spark".to_string(),
    })
}

// ── Invoice Events (event-driven) ───────────────────────────────────

/// EventListener that forwards PaymentSucceeded events through an mpsc channel
struct InvoiceEventListener {
    tx: tokio::sync::mpsc::UnboundedSender<breez_sdk_spark::Payment>,
}

#[async_trait::async_trait]
impl EventListener for InvoiceEventListener {
    async fn on_event(&self, event: SdkEvent) {
        if let SdkEvent::PaymentSucceeded { payment } = event {
            let _ = self.tx.send(payment);
        }
    }
}

/// Handle invoice events using event listener + timeout
pub async fn on_invoice_events(
    sdk: Arc<BreezSdk>,
    mut params: OnInvoiceEventParams,
    callback: std::sync::Arc<dyn OnInvoiceEventCallback>,
    cache: Arc<RwLock<HashMap<String, String>>>,
) {
    // Use payment_hash if provided, otherwise fall back to search
    if params.payment_hash.is_none() {
        params.payment_hash = params.search.clone();
    }

    let target_hash = match &params.payment_hash {
        Some(h) if !h.is_empty() => h.clone(),
        _ => {
            callback.failure(None);
            return;
        }
    };

    // Check if already settled
    if let Ok(txn) = lookup_invoice(
        sdk.clone(),
        Some(target_hash.clone()),
        None,
        None,
        None,
        cache.clone(),
    )
    .await
    {
        if txn.settled_at > 0 {
            callback.success(Some(txn));
            return;
        }
        callback.pending(Some(txn));
    }

    // Register event listener
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let listener = InvoiceEventListener { tx };
    let listener_id = sdk.add_event_listener(Box::new(listener)).await;

    let timeout = tokio::time::Duration::from_secs(params.max_polling_sec as u64);

    // Wait for matching event or timeout
    let result = tokio::time::timeout(timeout, async {
        while let Some(payment) = rx.recv().await {
            cache_payment(&payment, &cache).await;
            if let Some(info) = extract_payment_info(&payment) {
                if info.payment_hash == target_hash {
                    return Some(payment_to_transaction(&payment));
                }
            }
        }
        None
    })
    .await;

    // Cleanup listener
    sdk.remove_event_listener(&listener_id).await;

    match result {
        Ok(Some(Some(txn))) => callback.success(Some(txn)),
        Ok(Some(None)) => callback.failure(None), // matched but couldn't convert
        Ok(None) => callback.failure(None),        // channel closed
        Err(_) => {
            // Timeout — do one final lookup in case we missed the event
            if let Ok(txn) = lookup_invoice(
                sdk.clone(),
                Some(target_hash),
                None,
                None,
                None,
                cache,
            )
            .await
            {
                if txn.settled_at > 0 {
                    callback.success(Some(txn));
                    return;
                }
            }
            callback.failure(None);
        }
    }
}

/// Extract payment hash from a BOLT11 invoice
fn extract_payment_hash(invoice: &str) -> Option<String> {
    use lightning_invoice::Bolt11Invoice;
    use std::str::FromStr;

    Bolt11Invoice::from_str(invoice)
        .ok()
        .map(|inv| format!("{:x}", inv.payment_hash()))
}
