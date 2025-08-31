use crate::{ApiError, CreateInvoiceParams, PayInvoiceParams, PayCode, Transaction, PayInvoiceResponse, NodeInfo, ListTransactionsParams};
use crate::nwc::NwcConfig;
use crate::types::OnInvoiceEventParams;
use nwc::prelude::*;
use std::str::FromStr;
use std::thread;
use std::time::Duration;
use sha2::{Digest, Sha256};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

// Helper function to create NWC client
async fn create_nwc_client(config: &NwcConfig) -> Result<NWC, ApiError> {
    let uri = NostrWalletConnectURI::from_str(&config.nwc_uri)
        .map_err(|e| ApiError::Api { reason: format!("Invalid NWC URI: {}", e) })?;
    
    let opts = NostrWalletConnectOptions::default();
    let nwc = NWC::with_opts(uri, opts);
    
    Ok(nwc)
}

pub fn get_info(config: &NwcConfig) -> Result<NodeInfo, ApiError> {
    let rt = tokio::runtime::Runtime::new()
        .map_err(|e| ApiError::Api { reason: format!("Failed to create runtime: {}", e) })?;
    
    rt.block_on(async {
        let nwc = create_nwc_client(config).await?;
        
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
    })
}

pub fn create_invoice(config: &NwcConfig, params: CreateInvoiceParams) -> Result<Transaction, ApiError> {
    let rt = tokio::runtime::Runtime::new()
        .map_err(|e| ApiError::Api { reason: format!("Failed to create runtime: {}", e) })?;
    
    rt.block_on(async {
        let nwc = create_nwc_client(config).await?;
        
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
    })
}

pub fn pay_invoice(config: &NwcConfig, params: PayInvoiceParams) -> Result<PayInvoiceResponse, ApiError> {
    let rt = tokio::runtime::Runtime::new()
        .map_err(|e| ApiError::Api { reason: format!("Failed to create runtime: {}", e) })?;
    
    rt.block_on(async {
        let nwc = create_nwc_client(config).await?;
        
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
    })
}

pub fn get_offer(_config: &NwcConfig, _search: Option<String>) -> Result<PayCode, ApiError> {
    // NWC doesn't support offers/BOLT12 yet
    Err(ApiError::Api { reason: "NWC does not support offers (BOLT12) yet".to_string() })
}

pub fn list_offers(_config: &NwcConfig, _search: Option<String>) -> Result<Vec<PayCode>, ApiError> {
    // NWC doesn't support offers/BOLT12 yet
    Err(ApiError::Api { reason: "NWC does not support offers (BOLT12) yet".to_string() })
}

pub fn pay_offer(
    _config: &NwcConfig,
    _offer: String,
    _amount_msats: i64,
    _payer_note: Option<String>,
) -> Result<PayInvoiceResponse, ApiError> {
    // NWC doesn't support offers/BOLT12 yet
    Err(ApiError::Api { reason: "NWC does not support offers (BOLT12) yet".to_string() })
}

pub fn lookup_invoice(
    config: &NwcConfig,
    payment_hash: Option<String>,
    invoice: Option<String>,
) -> Result<Transaction, ApiError> {
    let rt = tokio::runtime::Runtime::new()
        .map_err(|e| ApiError::Api { reason: format!("Failed to create runtime: {}", e) })?;
    
    rt.block_on(async {
        let nwc = create_nwc_client(config).await?;
        
        let request = LookupInvoiceRequest {
            payment_hash: payment_hash.clone(),
            invoice: invoice.clone(),
        };
        
        let response = nwc.lookup_invoice(request).await
            .map_err(|e| ApiError::Api { reason: format!("Failed to lookup invoice: {}", e) })?;
        
        Ok(Transaction {
            type_: match response.transaction_type {
                Some(t) => format!("{:?}", t).to_lowercase(),
                None => "unknown".to_string(),
            },
            invoice: response.invoice.unwrap_or_default(),
            description: response.description.unwrap_or_default(),
            description_hash: "".to_string(),
            preimage: response.preimage.unwrap_or_default(),
            payment_hash: payment_hash.unwrap_or_default(),
            amount_msats: response.amount as i64,
            fees_paid: response.fees_paid as i64,
            created_at: response.created_at.as_u64() as i64,
            expires_at: response.expires_at.map(|t| t.as_u64() as i64).unwrap_or(0),
            settled_at: response.settled_at.map(|t| t.as_u64() as i64).unwrap_or(0),
            payer_note: None,
            external_id: None,
        })
    })
}

pub fn list_transactions(config: &NwcConfig, params: ListTransactionsParams) -> Result<Vec<Transaction>, ApiError> {
    let rt = tokio::runtime::Runtime::new()
        .map_err(|e| ApiError::Api { reason: format!("Failed to create runtime: {}", e) })?;
    
    rt.block_on(async {
        let nwc = create_nwc_client(config).await?;
        
        let request = ListTransactionsRequest {
            from: Some(Timestamp::from(params.from as u64)),
            until: None,
            limit: Some(params.limit as u64),
            offset: None,
            unpaid: None,
            transaction_type: None,
        };
        
        let response = nwc.list_transactions(request).await
            .map_err(|e| ApiError::Api { reason: format!("Failed to list transactions: {}", e) })?;
        
        let mut transactions = Vec::new();
        for tx in response {
            transactions.push(Transaction {
                type_: match tx.transaction_type {
                    Some(t) => format!("{:?}", t).to_lowercase(),
                    None => "unknown".to_string(),
                },
                invoice: tx.invoice.unwrap_or_default(),
                description: tx.description.unwrap_or_default(),
                description_hash: "".to_string(),
                preimage: tx.preimage.unwrap_or_default(),
                payment_hash: tx.payment_hash,
                amount_msats: tx.amount as i64,
                fees_paid: tx.fees_paid as i64,
                created_at: tx.created_at.as_u64() as i64,
                expires_at: tx.expires_at.map(|t| t.as_u64() as i64).unwrap_or(0),
                settled_at: tx.settled_at.map(|t| t.as_u64() as i64).unwrap_or(0),
                payer_note: None,
                external_id: None,
            });
        }
        
        Ok(transactions)
    })
}

pub fn decode(_config: &NwcConfig, str: String) -> Result<String, ApiError> {
    // NWC doesn't have a decode method, just return the input
    Ok(str)
}

// Simple cancellation token that's uniffi-compatible
#[cfg_attr(feature = "uniffi", derive(uniffi::Object))]
pub struct InvoiceEventsCancellation {
    cancelled: Arc<AtomicBool>,
}

#[cfg_attr(feature = "uniffi", uniffi::export)]
impl InvoiceEventsCancellation {
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }
    
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }
}

// Modified polling function that checks for cancellation
pub fn poll_invoice_events_with_cancellation<F>(
    config: &NwcConfig, 
    params: OnInvoiceEventParams, 
    cancellation: Arc<InvoiceEventsCancellation>,
    mut callback: F
) where
    F: FnMut(String, Option<Transaction>),
{
    let start_time = std::time::Instant::now();
    
    loop {
        // Check for cancellation first
        if cancellation.is_cancelled() {
            callback("cancelled".to_string(), None);
            break;
        }
        
        if start_time.elapsed() > Duration::from_secs(params.max_polling_sec as u64) {
            callback("failure".to_string(), None);
            break;
        }

        let (status, transaction) =
            match lookup_invoice(config, params.payment_hash.clone(), params.search.clone()) {
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
                // Don't break on error, keep polling
            }
            _ => {
                callback("pending".to_string(), transaction);
            }
        }

        // Sleep in small increments to check cancellation more frequently
        let delay_secs = params.polling_delay_sec as u64;
        for _ in 0..delay_secs {
            if cancellation.is_cancelled() {
                callback("cancelled".to_string(), None);
                return;
            }
            thread::sleep(Duration::from_secs(1));
        }
    }
}

// Updated function that returns a cancellation token
pub fn on_invoice_events_with_cancellation(
    config: NwcConfig,
    params: OnInvoiceEventParams,
    callback: Box<dyn crate::types::OnInvoiceEventCallback + Send>,
) -> Arc<InvoiceEventsCancellation> {
    let cancellation = Arc::new(InvoiceEventsCancellation {
        cancelled: Arc::new(AtomicBool::new(false)),
    });
    
    let cancellation_clone = cancellation.clone();
    
    // Spawn on a thread pool to avoid blocking
    std::thread::spawn(move || {
        poll_invoice_events_with_cancellation(&config, params, cancellation_clone, move |status, tx| {
            match status.as_str() {
                "success" => callback.success(tx),
                "pending" => callback.pending(tx),
                "failure" | "cancelled" => callback.failure(tx),
                _ => {}
            }
        });
    });
    
    cancellation
}

// UniFFI-compatible version that doesn't require Send
#[cfg_attr(feature = "uniffi", uniffi::export)]
pub fn nwc_on_invoice_events_with_cancellation(
    config: NwcConfig,
    params: OnInvoiceEventParams,
    callback: Box<dyn crate::types::OnInvoiceEventCallback>,
) -> Arc<InvoiceEventsCancellation> {
    let cancellation = Arc::new(InvoiceEventsCancellation {
        cancelled: Arc::new(AtomicBool::new(false)),
    });
    
    let cancellation_clone = cancellation.clone();
    
    // For UniFFI, run synchronously - UniFFI will handle threading 
    poll_invoice_events_with_cancellation(&config, params, cancellation_clone, move |status, tx| {
        match status.as_str() {
            "success" => callback.success(tx),
            "pending" => callback.pending(tx),
            "failure" | "cancelled" => callback.failure(tx),
            _ => {}
        }
    });
    
    cancellation
}

pub fn poll_invoice_events<F>(config: &NwcConfig, params: OnInvoiceEventParams, mut callback: F)
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
            params.search.clone(),
        ) {
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
                // Don't break on error, keep polling
            }
            _ => {
                callback("pending".to_string(), transaction);
            }
        }

        thread::sleep(Duration::from_secs(params.polling_delay_sec as u64));
    }
}

pub fn on_invoice_events(
    config: NwcConfig,
    params: OnInvoiceEventParams,
    callback: Box<dyn crate::types::OnInvoiceEventCallback>,
) {
    poll_invoice_events(&config, params, move |status, tx| match status.as_str() {
        "success" => callback.success(tx),
        "pending" => callback.pending(tx),
        "failure" => callback.failure(tx),
        _ => {}
    });
}
