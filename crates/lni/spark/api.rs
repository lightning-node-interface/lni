use std::sync::Arc;
use std::time::Duration;

use breez_sdk_spark::{
    BreezSdk, GetInfoRequest, ListPaymentsRequest, PaymentDetails, PaymentStatus, PaymentType,
    PrepareSendPaymentRequest, ReceivePaymentMethod, ReceivePaymentRequest, SendPaymentRequest,
};

use crate::types::NodeInfo;
use crate::{
    ApiError, CreateInvoiceParams, InvoiceType, Offer, OnInvoiceEventCallback,
    OnInvoiceEventParams, PayInvoiceParams, PayInvoiceResponse, Transaction,
};

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

/// Lookup an invoice by payment hash
pub async fn lookup_invoice(
    sdk: Arc<BreezSdk>,
    payment_hash: Option<String>,
    _from: Option<i64>,
    _limit: Option<i64>,
    _search: Option<String>,
) -> Result<Transaction, ApiError> {
    let target_hash = payment_hash.ok_or_else(|| ApiError::Api {
        reason: "Payment hash is required for lookup".to_string(),
    })?;

    if target_hash.is_empty() {
        return Err(ApiError::Api {
            reason: "Payment hash cannot be empty".to_string(),
        });
    }

    let payments = sdk
        .list_payments(ListPaymentsRequest {
            limit: Some(100),
            ..Default::default()
        })
        .await
        .map_err(|e| ApiError::Api {
            reason: e.to_string(),
        })?;

    for payment in payments.payments {
        let (invoice, p_hash, preimage, description) = match &payment.details {
            Some(PaymentDetails::Lightning {
                invoice,
                payment_hash,
                preimage,
                description,
                ..
            }) => (
                invoice.clone(),
                payment_hash.clone(),
                preimage.clone().unwrap_or_default(),
                description.clone().unwrap_or_default(),
            ),
            Some(PaymentDetails::Spark {
                invoice_details,
                htlc_details,
                ..
            }) => {
                let inv = invoice_details
                    .as_ref()
                    .map(|d| d.invoice.clone())
                    .unwrap_or_default();
                let desc = invoice_details
                    .as_ref()
                    .and_then(|d| d.description.clone())
                    .unwrap_or_default();
                let (hash, preimg) = if let Some(htlc) = htlc_details {
                    (htlc.payment_hash.clone(), htlc.preimage.clone().unwrap_or_default())
                } else {
                    ("".to_string(), "".to_string())
                };
                (inv, hash, preimg, desc)
            }
            _ => continue,
        };

        if p_hash == target_hash {
            return Ok(Transaction {
                type_: match payment.payment_type {
                    PaymentType::Send => "outgoing".to_string(),
                    PaymentType::Receive => "incoming".to_string(),
                },
                invoice,
                preimage,
                payment_hash: p_hash,
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
                external_id: Some(payment.id),
            });
        }
    }

    Err(ApiError::Api {
        reason: format!("Invoice not found for payment hash: {}", target_hash),
    })
}

/// List transactions from Spark SDK
pub async fn list_transactions(
    sdk: Arc<BreezSdk>,
    from: i64,
    limit: i64,
    _search: Option<String>,
) -> Result<Vec<Transaction>, ApiError> {
    let payments = sdk
        .list_payments(ListPaymentsRequest {
            offset: Some(from as u32),
            limit: Some(limit as u32),
            ..Default::default()
        })
        .await
        .map_err(|e| ApiError::Api {
            reason: e.to_string(),
        })?;

    let mut transactions = Vec::new();

    for payment in payments.payments {
        let (invoice, payment_hash, preimage, description) = match &payment.details {
            Some(PaymentDetails::Lightning {
                invoice,
                payment_hash,
                preimage,
                description,
                ..
            }) => (
                invoice.clone(),
                payment_hash.clone(),
                preimage.clone().unwrap_or_default(),
                description.clone().unwrap_or_default(),
            ),
            Some(PaymentDetails::Spark {
                invoice_details,
                htlc_details,
                ..
            }) => {
                let inv = invoice_details
                    .as_ref()
                    .map(|d| d.invoice.clone())
                    .unwrap_or_default();
                let desc = invoice_details
                    .as_ref()
                    .and_then(|d| d.description.clone())
                    .unwrap_or_default();
                let (hash, preimg) = if let Some(htlc) = htlc_details {
                    (htlc.payment_hash.clone(), htlc.preimage.clone().unwrap_or_default())
                } else {
                    ("".to_string(), "".to_string())
                };
                (inv, hash, preimg, desc)
            }
            Some(PaymentDetails::Deposit { tx_id }) => {
                (tx_id.clone(), "".to_string(), "".to_string(), "Deposit".to_string())
            }
            Some(PaymentDetails::Withdraw { tx_id }) => {
                (tx_id.clone(), "".to_string(), "".to_string(), "Withdraw".to_string())
            }
            Some(PaymentDetails::Token { tx_hash, .. }) => {
                (tx_hash.clone(), "".to_string(), "".to_string(), "Token Transfer".to_string())
            }
            None => continue,
        };

        transactions.push(Transaction {
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
            external_id: Some(payment.id),
        });
    }

    Ok(transactions)
}

/// Decode a payment request using the SDK's parse functionality
pub async fn decode(sdk: Arc<BreezSdk>, input: String) -> Result<String, ApiError> {
    // Use the SDK's parse method to decode the input
    match sdk.parse(&input).await {
        Ok(parsed) => {
            // Return a JSON representation of the parsed input
            Ok(format!("{:?}", parsed))
        }
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

/// Poll invoice events
pub async fn poll_invoice_events<F>(
    sdk: Arc<BreezSdk>,
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
            sdk.clone(),
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

/// Handle invoice events with callback trait
pub async fn on_invoice_events(
    sdk: Arc<BreezSdk>,
    params: OnInvoiceEventParams,
    callback: std::sync::Arc<dyn OnInvoiceEventCallback>,
) {
    poll_invoice_events(sdk, params, move |status, tx| match status.as_str() {
        "success" => callback.success(tx),
        "pending" => callback.pending(tx),
        "failure" => callback.failure(tx),
        _ => {}
    })
    .await;
}

/// Extract payment hash from a BOLT11 invoice
fn extract_payment_hash(invoice: &str) -> Option<String> {
    use lightning_invoice::Bolt11Invoice;
    use std::str::FromStr;

    Bolt11Invoice::from_str(invoice)
        .ok()
        .map(|inv| format!("{:x}", inv.payment_hash()))
}
