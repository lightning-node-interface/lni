use std::sync::Arc;
use std::time::Duration;
use std::str::FromStr;

use bip39::{Language, Mnemonic};
use cdk::nuts::{CurrencyUnit, MeltQuoteState, MintQuoteState};
use cdk::wallet::Wallet;
use cdk_sqlite::wallet::memory;

use super::CashuConfig;
use crate::types::NodeInfo;
use crate::{
    ApiError, CreateInvoiceParams, InvoiceType, Offer, OnInvoiceEventCallback, OnInvoiceEventParams,
    PayInvoiceParams, PayInvoiceResponse, Transaction,
};

/// Convert satoshi amount to millisatoshi, with overflow protection
fn sats_to_msats(sats: u64) -> i64 {
    // Limit to prevent overflow when multiplying by 1000
    // i64::MAX is ~9.2 quintillion, so max sats is ~9.2 quadrillion
    let max_sats = (i64::MAX / 1000) as u64;
    let clamped_sats = sats.min(max_sats);
    (clamped_sats as i64) * 1000
}

/// Generate a random seed for the wallet
fn generate_seed() -> [u8; 64] {
    let mut seed = [0u8; 64];
    // Use getrandom for secure random number generation
    getrandom::getrandom(&mut seed).expect("Failed to generate random seed");
    seed
}

/// Parse seed from string - supports both mnemonic phrases and hex seeds
fn parse_seed(seed_str: &str) -> Result<[u8; 64], ApiError> {
    let trimmed = seed_str.trim();
    
    // Check if it looks like a mnemonic (contains spaces and words)
    if trimmed.contains(' ') {
        // Parse as BIP39 mnemonic
        let mnemonic = Mnemonic::from_str(trimmed).map_err(|e| ApiError::Api {
            reason: format!("Invalid mnemonic phrase: {}", e),
        })?;
        
        // Convert mnemonic to seed (uses empty passphrase)
        let seed_bytes = mnemonic.to_seed("");
        let mut seed_arr = [0u8; 64];
        seed_arr.copy_from_slice(&seed_bytes);
        Ok(seed_arr)
    } else {
        // Parse as hex seed
        let seed_bytes = hex::decode(trimmed).map_err(|e| ApiError::Api {
            reason: format!("Invalid seed hex: {}", e),
        })?;
        let mut seed_arr = [0u8; 64];
        if seed_bytes.len() != 64 {
            return Err(ApiError::Api {
                reason: format!("Seed must be 64 bytes (128 hex chars), got {} bytes", seed_bytes.len()),
            });
        }
        seed_arr.copy_from_slice(&seed_bytes);
        Ok(seed_arr)
    }
}

/// Create a CDK wallet from config
pub async fn create_wallet(config: &CashuConfig) -> Result<Wallet, ApiError> {
    // Use provided seed or generate random one
    let seed: [u8; 64] = if let Some(seed_str) = &config.seed {
        parse_seed(seed_str)?
    } else {
        generate_seed()
    };

    let unit = CurrencyUnit::Sat;

    // Create in-memory store (for now, could be extended to persistent storage)
    let localstore = Arc::new(memory::empty().await.map_err(|e| ApiError::Api {
        reason: format!("Failed to create wallet store: {}", e),
    })?);

    Wallet::new(&config.mint_url, unit, localstore, seed, None).map_err(|e| ApiError::Api {
        reason: format!("Failed to create wallet: {}", e),
    })
}

pub async fn get_info(config: CashuConfig) -> Result<NodeInfo, ApiError> {
    let wallet = create_wallet(&config).await?;

    // Get balance from wallet
    let balance = wallet.total_balance().await.map_err(|e| ApiError::Api {
        reason: format!("Failed to get balance: {}", e),
    })?;

    // Try to get mint info
    let mint_info = wallet.load_mint_info().await.ok();
    let alias = mint_info
        .as_ref()
        .and_then(|info| info.name.clone())
        .unwrap_or_else(|| "Cashu Wallet".to_string());

    // Convert balance to millisats (Cashu uses sats)
    let balance_sats: u64 = balance.into();
    let balance_msat = sats_to_msats(balance_sats);

    Ok(NodeInfo {
        alias,
        color: "".to_string(),
        pubkey: mint_info
            .as_ref()
            .and_then(|info| info.pubkey.as_ref())
            .map(|pk| pk.to_string())
            .unwrap_or_default(),
        network: "mainnet".to_string(),
        block_height: 0,
        block_hash: "".to_string(),
        send_balance_msat: balance_msat,
        receive_balance_msat: balance_msat,
        fee_credit_balance_msat: 0,
        unsettled_send_balance_msat: 0,
        unsettled_receive_balance_msat: 0,
        pending_open_send_balance: 0,
        pending_open_receive_balance: 0,
    })
}

pub async fn create_invoice(
    config: CashuConfig,
    invoice_params: CreateInvoiceParams,
) -> Result<Transaction, ApiError> {
    let wallet = create_wallet(&config).await?;

    match invoice_params.get_invoice_type() {
        InvoiceType::Bolt11 => {
            let amount_sats = invoice_params.amount_msats.unwrap_or(0) / 1000;
            let amount = cdk::Amount::from(amount_sats as u64);

            // Create a mint quote (request to receive Lightning payment)
            let quote = wallet
                .mint_quote(amount, invoice_params.description.clone())
                .await
                .map_err(|e| ApiError::Api {
                    reason: format!("Failed to create mint quote: {}", e),
                })?;

            let now = chrono::Utc::now().timestamp();
            let expiry = invoice_params.expiry.unwrap_or(3600);

            Ok(Transaction {
                type_: "incoming".to_string(),
                invoice: quote.request.clone(),
                preimage: "".to_string(),
                payment_hash: quote.id.clone(),
                amount_msats: invoice_params.amount_msats.unwrap_or(0),
                fees_paid: 0,
                created_at: now,
                expires_at: now + expiry,
                settled_at: 0,
                description: invoice_params.description.unwrap_or_default(),
                description_hash: invoice_params.description_hash.unwrap_or_default(),
                payer_note: None,
                external_id: Some(quote.id),
            })
        }
        InvoiceType::Bolt12 => Err(ApiError::Api {
            reason: "Bolt12 not implemented for Cashu".to_string(),
        }),
    }
}

pub async fn pay_invoice(
    config: CashuConfig,
    invoice_params: PayInvoiceParams,
) -> Result<PayInvoiceResponse, ApiError> {
    let wallet = create_wallet(&config).await?;

    // Create a melt quote (request to pay Lightning invoice)
    let quote = wallet
        .melt_quote(invoice_params.invoice.clone(), None)
        .await
        .map_err(|e| ApiError::Api {
            reason: format!("Failed to create melt quote: {}", e),
        })?;

    // Execute the melt (pay the invoice)
    let melt_response = wallet
        .melt(&quote.id)
        .await
        .map_err(|e| ApiError::Api {
            reason: format!("Failed to pay invoice: {}", e),
        })?;

    // Extract payment hash from the BOLT11 invoice
    let payment_hash = match lightning_invoice::Bolt11Invoice::from_str(&invoice_params.invoice) {
        Ok(invoice) => format!("{:x}", invoice.payment_hash()),
        Err(_) => quote.id.clone(),
    };

    // Fee is the total amount minus the invoice amount
    let fee_sats: u64 = melt_response.fee_paid.into();
    let fee_msats = sats_to_msats(fee_sats);

    Ok(PayInvoiceResponse {
        payment_hash,
        preimage: melt_response
            .preimage
            .map(|p| p.to_string())
            .unwrap_or_default(),
        fee_msats,
    })
}

pub fn decode(_config: &CashuConfig, str: String) -> Result<String, ApiError> {
    // Try to decode as Cashu token
    if str.starts_with("cashu") {
        return Ok(format!("Cashu token: {}", str));
    }

    // Try to decode as BOLT11
    if str.starts_with("lnbc") || str.starts_with("lntb") || str.starts_with("lnbcrt") {
        if let Ok(invoice) = lightning_invoice::Bolt11Invoice::from_str(&str) {
            return Ok(format!(
                "BOLT11 Invoice: amount={:?}, description={:?}",
                invoice.amount_milli_satoshis(),
                invoice.description()
            ));
        }
    }

    Ok(str)
}

pub fn get_offer(_config: &CashuConfig, _search: Option<String>) -> Result<Offer, ApiError> {
    Err(ApiError::Api {
        reason: "BOLT12 offers not implemented for Cashu".to_string(),
    })
}

pub async fn list_offers(
    _config: &CashuConfig,
    _search: Option<String>,
) -> Result<Vec<Offer>, ApiError> {
    Err(ApiError::Api {
        reason: "BOLT12 offers not implemented for Cashu".to_string(),
    })
}

pub fn pay_offer(
    _config: &CashuConfig,
    _offer: String,
    _amount_msats: i64,
    _payer_note: Option<String>,
) -> Result<PayInvoiceResponse, ApiError> {
    Err(ApiError::Api {
        reason: "BOLT12 offers not implemented for Cashu".to_string(),
    })
}

pub async fn lookup_invoice(
    config: CashuConfig,
    payment_hash: Option<String>,
    _from: Option<i64>,
    _limit: Option<i64>,
    _search: Option<String>,
) -> Result<Transaction, ApiError> {
    let wallet = create_wallet(&config).await?;

    let quote_id = payment_hash.ok_or_else(|| ApiError::Api {
        reason: "Payment hash (quote ID) is required".to_string(),
    })?;

    // Check mint quote status
    let quote_state = wallet
        .mint_quote_state(&quote_id)
        .await
        .map_err(|e| ApiError::Api {
            reason: format!("Failed to get quote status: {}", e),
        })?;

    let settled_at = if quote_state.state == MintQuoteState::Paid {
        chrono::Utc::now().timestamp()
    } else {
        0
    };

    // Convert amount from Option<Amount> to i64 msats
    let amount_sats: u64 = quote_state.amount.map(|a| a.into()).unwrap_or(0);
    let amount_msats = sats_to_msats(amount_sats);

    // Convert expiry from Option<u64> to i64
    let expires_at = quote_state.expiry.map(|e| e as i64).unwrap_or(0);

    Ok(Transaction {
        type_: "incoming".to_string(),
        invoice: quote_state.request.clone(),
        preimage: "".to_string(),
        payment_hash: quote_id,
        amount_msats,
        fees_paid: 0,
        created_at: 0,
        expires_at,
        settled_at,
        description: "".to_string(),
        description_hash: "".to_string(),
        payer_note: None,
        external_id: None,
    })
}

pub async fn list_transactions(
    config: CashuConfig,
    from: i64,
    limit: i64,
    _search: Option<String>,
) -> Result<Vec<Transaction>, ApiError> {
    let wallet = create_wallet(&config).await?;

    let mut transactions: Vec<Transaction> = Vec::new();

    // Get pending mint quotes from localstore
    let mint_quotes = wallet.localstore.get_mint_quotes().await.map_err(|e| ApiError::Api {
        reason: format!("Failed to get mint quotes: {}", e),
    })?;

    for quote in mint_quotes.iter().skip(from as usize).take(limit as usize) {
        let settled_at = if quote.state == MintQuoteState::Issued {
            chrono::Utc::now().timestamp()
        } else {
            0
        };

        let amount_sats: u64 = quote.amount.map(|a| a.into()).unwrap_or(0);
        let expires_at = quote.expiry as i64;

        transactions.push(Transaction {
            type_: "incoming".to_string(),
            invoice: quote.request.clone(),
            preimage: "".to_string(),
            payment_hash: quote.id.clone(),
            amount_msats: sats_to_msats(amount_sats),
            fees_paid: 0,
            created_at: 0,
            expires_at,
            settled_at,
            description: "".to_string(),
            description_hash: "".to_string(),
            payer_note: None,
            external_id: Some(quote.id.clone()),
        });
    }

    // Get pending melt quotes from localstore
    let melt_quotes = wallet.localstore.get_melt_quotes().await.map_err(|e| ApiError::Api {
        reason: format!("Failed to get melt quotes: {}", e),
    })?;

    for quote in melt_quotes.iter() {
        let settled_at = if quote.state == MeltQuoteState::Paid {
            chrono::Utc::now().timestamp()
        } else {
            0
        };

        let amount_sats: u64 = quote.amount.into();
        let fee_reserve_sats: u64 = quote.fee_reserve.into();
        let expires_at = quote.expiry as i64;

        transactions.push(Transaction {
            type_: "outgoing".to_string(),
            invoice: quote.request.clone(),
            preimage: "".to_string(),
            payment_hash: quote.id.clone(),
            amount_msats: sats_to_msats(amount_sats),
            fees_paid: sats_to_msats(fee_reserve_sats),
            created_at: 0,
            expires_at,
            settled_at,
            description: "".to_string(),
            description_hash: "".to_string(),
            payer_note: None,
            external_id: Some(quote.id.clone()),
        });
    }

    // Sort by created date descending
    transactions.sort_by(|a, b| b.created_at.cmp(&a.created_at));

    Ok(transactions)
}

// Core logic for invoice event polling
pub async fn poll_invoice_events<F>(
    config: CashuConfig,
    params: OnInvoiceEventParams,
    mut callback: F,
) where
    F: FnMut(String, Option<Transaction>),
{
    let start_time = std::time::Instant::now();

    loop {
        if start_time.elapsed() > Duration::from_secs(params.max_polling_sec as u64) {
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

        tokio::time::sleep(Duration::from_secs(params.polling_delay_sec as u64)).await;
    }
}

pub async fn on_invoice_events(
    config: CashuConfig,
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
