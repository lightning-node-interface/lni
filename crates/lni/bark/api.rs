use std::sync::Arc;

use bark::actions::lightning::pay::LightningSendState;
use bark::ark::lightning::PaymentHash;
use bark::movement::{Movement, MovementStatus};
use bark::Wallet;
use bitcoin::Amount;

use crate::types::NodeInfo;
use crate::{
    ApiError, CreateInvoiceParams, InvoiceType, ListTransactionsParams, LookupInvoiceParams, Offer,
    OnInvoiceEventCallback, OnInvoiceEventParams, PayInvoiceParams, PayInvoiceResponse,
    Transaction,
};

fn sats_to_msats(sats: u64) -> i64 {
    i64::try_from(sats)
        .ok()
        .and_then(|s| s.checked_mul(1000))
        .unwrap_or(i64::MAX)
}

fn signed_sats_to_msats(sats: i64) -> i64 {
    sats.checked_mul(1000).unwrap_or_else(|| {
        if sats.is_negative() {
            i64::MIN
        } else {
            i64::MAX
        }
    })
}

fn msats_to_amount(msats: i64) -> Result<Amount, ApiError> {
    if msats <= 0 {
        return Err(ApiError::InvalidInput(
            "amount_msats must be greater than zero for Bark".to_string(),
        ));
    }

    let sats = u64::try_from(msats / 1000)
        .map_err(|_| ApiError::InvalidInput("amount_msats is out of range for Bark".to_string()))?;

    if sats == 0 {
        return Err(ApiError::InvalidInput(
            "Bark requires at least 1000 msats".to_string(),
        ));
    }

    Ok(Amount::from_sat(sats))
}

fn optional_msats_to_amount(msats: Option<i64>) -> Result<Option<Amount>, ApiError> {
    msats.map(msats_to_amount).transpose()
}

fn payment_hash_matches(hash: PaymentHash, search: &str) -> bool {
    hash.to_string().eq_ignore_ascii_case(search)
}

fn movement_to_transaction(movement: &Movement) -> Transaction {
    let invoice = movement
        .lightning_invoice()
        .map(ToString::to_string)
        .unwrap_or_default();
    let payment_hash = movement
        .lightning_payment_hash()
        .map(|hash| hash.to_string())
        .unwrap_or_default();
    let description = movement
        .metadata
        .get("description")
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .to_string();

    let effective_msats = signed_sats_to_msats(movement.effective_balance.to_sat());
    let amount_msats = if effective_msats == 0 {
        movement
            .sent_to
            .iter()
            .chain(movement.received_on.iter())
            .map(|destination| sats_to_msats(destination.amount.to_sat()))
            .next()
            .unwrap_or(0)
    } else {
        effective_msats.abs()
    };

    Transaction {
        type_: if movement.effective_balance.to_sat() < 0 {
            "outgoing".to_string()
        } else {
            "incoming".to_string()
        },
        invoice,
        description,
        description_hash: "".to_string(),
        preimage: "".to_string(),
        payment_hash,
        amount_msats,
        fees_paid: sats_to_msats(movement.offchain_fee.to_sat()),
        created_at: movement.time.created_at.timestamp(),
        expires_at: 0,
        settled_at: match movement.status {
            MovementStatus::Successful => movement
                .time
                .completed_at
                .unwrap_or(movement.time.updated_at)
                .timestamp(),
            _ => 0,
        },
        payer_note: None,
        external_id: Some(movement.id.to_string()),
    }
}

pub async fn get_info(wallet: Arc<Wallet>, network: &str) -> Result<NodeInfo, ApiError> {
    wallet.sync().await;

    let balance = wallet.balance().await.map_err(|e| ApiError::Api {
        reason: e.to_string(),
    })?;

    Ok(NodeInfo {
        alias: "Bark Node".to_string(),
        color: "".to_string(),
        pubkey: wallet.fingerprint().to_string(),
        network: network.to_string(),
        block_height: 0,
        block_hash: "".to_string(),
        send_balance_msat: sats_to_msats(balance.spendable.to_sat()),
        receive_balance_msat: 0,
        fee_credit_balance_msat: 0,
        unsettled_send_balance_msat: sats_to_msats(balance.pending_lightning_send.to_sat()),
        unsettled_receive_balance_msat: sats_to_msats(balance.claimable_lightning_receive.to_sat()),
        pending_open_send_balance: sats_to_msats(balance.pending_in_round.to_sat()),
        pending_open_receive_balance: sats_to_msats(balance.pending_board.to_sat()),
    })
}

pub async fn create_invoice(
    wallet: Arc<Wallet>,
    params: CreateInvoiceParams,
) -> Result<Transaction, ApiError> {
    match params.get_invoice_type() {
        InvoiceType::Bolt11 => {
            let amount_msats = params.amount_msats.ok_or_else(|| {
                ApiError::InvalidInput("amount_msats is required for Bark invoices".to_string())
            })?;
            let amount = msats_to_amount(amount_msats)?;
            let invoice = wallet
                .bolt11_invoice(amount, params.description.clone())
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
                invoice: invoice.to_string(),
                description: params.description.unwrap_or_default(),
                description_hash: params.description_hash.unwrap_or_default(),
                preimage: "".to_string(),
                payment_hash: format!("{:x}", invoice.payment_hash()),
                amount_msats,
                fees_paid: 0,
                created_at: now,
                expires_at: now
                    + params
                        .expiry
                        .unwrap_or(crate::types::DEFAULT_INVOICE_EXPIRY),
                settled_at: 0,
                payer_note: None,
                external_id: None,
            })
        }
        InvoiceType::Bolt12 => Err(ApiError::Api {
            reason: "Bolt12 invoices are not yet implemented for BarkNode".to_string(),
        }),
    }
}

pub async fn pay_invoice(
    wallet: Arc<Wallet>,
    params: PayInvoiceParams,
) -> Result<PayInvoiceResponse, ApiError> {
    let amount = optional_msats_to_amount(params.amount_msats)?;
    let invoice = wallet
        .pay_lightning_invoice(params.invoice, amount, true)
        .await
        .map_err(|e| ApiError::Api {
            reason: e.to_string(),
        })?;
    let payment_hash = invoice.payment_hash();

    let (preimage, fee_msats) =
        match wallet
            .lightning_send_state(payment_hash)
            .await
            .map_err(|e| ApiError::Api {
                reason: e.to_string(),
            })? {
            LightningSendState::Paid(paid) => (paid.preimage.to_string(), 0),
            LightningSendState::InProgress(send) => {
                ("".to_string(), sats_to_msats(send.fee.to_sat()))
            }
            LightningSendState::Unknown => ("".to_string(), 0),
        };

    Ok(PayInvoiceResponse {
        payment_hash: payment_hash.to_string(),
        preimage,
        fee_msats,
    })
}

pub async fn lookup_invoice(
    wallet: Arc<Wallet>,
    params: LookupInvoiceParams,
) -> Result<Transaction, ApiError> {
    let payment_hash = params.payment_hash.or(params.search).ok_or_else(|| {
        ApiError::InvalidInput(
            "lookup_invoice requires payment_hash or search for BarkNode".to_string(),
        )
    })?;

    let movements = wallet.history().await.map_err(|e| ApiError::Api {
        reason: e.to_string(),
    })?;

    movements
        .iter()
        .find(|movement| {
            movement
                .lightning_payment_hash()
                .map(|hash| payment_hash_matches(hash, &payment_hash))
                .unwrap_or(false)
                || movement
                    .lightning_invoice()
                    .map(|invoice| invoice.to_string() == payment_hash)
                    .unwrap_or(false)
        })
        .map(movement_to_transaction)
        .ok_or_else(|| ApiError::Api {
            reason: format!(
                "Invoice not found for payment hash or search: {}",
                payment_hash
            ),
        })
}

pub async fn list_transactions(
    wallet: Arc<Wallet>,
    params: ListTransactionsParams,
) -> Result<Vec<Transaction>, ApiError> {
    let mut movements = wallet.history().await.map_err(|e| ApiError::Api {
        reason: e.to_string(),
    })?;

    if let Some(search) = params.search {
        movements.retain(|movement| {
            movement
                .lightning_payment_hash()
                .map(|hash| payment_hash_matches(hash, &search))
                .unwrap_or(false)
                || movement
                    .lightning_invoice()
                    .map(|invoice| invoice.to_string().contains(&search))
                    .unwrap_or(false)
                || movement
                    .metadata
                    .values()
                    .any(|value| value.as_str().map(|s| s.contains(&search)).unwrap_or(false))
        });
    }

    if let Some(created_after) = params.created_after {
        movements.retain(|movement| movement.time.created_at.timestamp() >= created_after);
    }

    if let Some(created_before) = params.created_before {
        movements.retain(|movement| movement.time.created_at.timestamp() <= created_before);
    }

    let from = usize::try_from(params.from.max(0)).unwrap_or(usize::MAX);
    let limit = usize::try_from(params.limit.max(0)).unwrap_or(usize::MAX);

    Ok(movements
        .iter()
        .skip(from)
        .take(limit)
        .map(movement_to_transaction)
        .collect())
}

pub fn get_offer(_search: Option<String>) -> Result<Offer, ApiError> {
    Err(ApiError::Api {
        reason: "Bolt12 offers are not yet implemented for BarkNode".to_string(),
    })
}

pub fn list_offers(_search: Option<String>) -> Result<Vec<Offer>, ApiError> {
    Err(ApiError::Api {
        reason: "Bolt12 offers are not yet implemented for BarkNode".to_string(),
    })
}

pub fn pay_offer(
    _offer: String,
    _amount_msats: i64,
    _payer_note: Option<String>,
) -> Result<PayInvoiceResponse, ApiError> {
    Err(ApiError::Api {
        reason: "Bolt12 offers are not yet implemented for BarkNode".to_string(),
    })
}

pub async fn on_invoice_events(
    _wallet: Arc<Wallet>,
    _params: OnInvoiceEventParams,
    callback: Arc<dyn OnInvoiceEventCallback>,
) {
    callback.failure(None);
}
