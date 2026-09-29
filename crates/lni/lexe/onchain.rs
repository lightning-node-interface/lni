//! Lexe does not expose fee estimation or fee caps. Preparation is local only.
use std::str::FromStr;

use lexe::{
    types::{
        bitcoin::{Amount, ConfirmationPriority},
        command::PayOnchainRequest,
        payment::{ClientPaymentId, Payment, PaymentStatus},
    },
    wallet::LexeWallet,
};

use crate::{
    ApiError, OnchainFeePayer, OnchainFeePreference, OnchainFeePreferenceType, OnchainFeeSpeed,
    OnchainTransaction, PayOnchainOptions, PayOnchainResponse, PrepareOnchainTransactionParams,
};

fn invalid(message: impl Into<String>) -> ApiError {
    ApiError::InvalidInput(message.into())
}

fn priority(fee: &OnchainFeePreference) -> Result<ConfirmationPriority, ApiError> {
    match (&fee.preference_type, &fee.speed) {
        (OnchainFeePreferenceType::Default, _) => Ok(ConfirmationPriority::Normal),
        (OnchainFeePreferenceType::Speed, Some(OnchainFeeSpeed::Fast)) => {
            Ok(ConfirmationPriority::High)
        }
        (OnchainFeePreferenceType::Speed, Some(OnchainFeeSpeed::Normal)) => {
            Ok(ConfirmationPriority::Normal)
        }
        (OnchainFeePreferenceType::Speed, Some(OnchainFeeSpeed::Slow)) => {
            Ok(ConfirmationPriority::Background)
        }
        _ => Err(invalid(
            "Lexe supports only default or fast/normal/slow on-chain fee speeds",
        )),
    }
}

fn request(transaction: &OnchainTransaction, network: &str) -> Result<PayOnchainRequest, ApiError> {
    if transaction.amount_sats <= 0 || transaction.amount_sats > 2_100_000_000_000_000 {
        return Err(invalid(
            "amount_sats must be positive and at most 21 million BTC",
        ));
    }
    if transaction.fee_payer != OnchainFeePayer::Sender {
        return Err(invalid("Lexe supports only sender-paid on-chain fees"));
    }
    let note: Option<String> = transaction
        .raw
        .as_deref()
        .map(serde_json::from_str)
        .transpose()
        .map_err(|_| invalid("Invalid Lexe prepared personal note"))?
        .flatten();
    if let Some(note) = &note {
        if note.is_empty() || note.chars().count() > 200 || note.len() > 512 {
            return Err(invalid(
                "description must be non-empty and at most 200 characters / 512 UTF-8 bytes",
            ));
        }
    }
    let req = PayOnchainRequest {
        address: transaction
            .address
            .parse()
            .map_err(|_| invalid("Invalid Bitcoin address"))?,
        amount: Amount::from_msat(transaction.amount_sats as u64 * 1_000),
        priority: Some(priority(&transaction.fee)?),
        client_payment_id: Some(
            ClientPaymentId::from_str(
                transaction
                    .id
                    .as_deref()
                    .ok_or_else(|| invalid("Lexe pay_onchain requires a prepared payment id"))?,
            )
            .map_err(|_| invalid("idempotency key must be a 64-character hex string"))?,
        ),
        personal_note: note,
    };
    // Infer the SDK's bitcoin::Network type, avoiding a second bitcoin dependency.
    let bitcoin_network = match network {
        "mainnet" => "bitcoin",
        "testnet3" => "testnet",
        _ => return Err(invalid("Unsupported Lexe network")),
    }
    .parse()
    .map_err(|_| invalid("Invalid Bitcoin network"))?;
    req.address
        .clone()
        .require_network(bitcoin_network)
        .map_err(|_| invalid("Bitcoin address does not match the Lexe wallet network"))?;
    Ok(req)
}

pub fn prepare(
    params: PrepareOnchainTransactionParams,
    network: &str,
) -> Result<OnchainTransaction, ApiError> {
    let transaction = OnchainTransaction {
        fee_limit_supported: Some(false),
        id: Some(
            params
                .idempotency_key
                .unwrap_or_else(|| ClientPaymentId::generate().to_string()),
        ),
        address: params.address,
        amount_sats: params.amount_sats,
        fee_sats: None,
        total_amount_sats: None,
        recipient_amount_sats: Some(params.amount_sats),
        fee_payer: params.fee_payer.unwrap_or_default(),
        fee: params.fee.unwrap_or_default(),
        expires_at: None,
        estimated_delivery_seconds: None,
        raw: Some(
            serde_json::to_string(&params.description)
                .map_err(|_| invalid("Invalid description"))?,
        ),
    };
    request(&transaction, network)?;
    Ok(transaction)
}

fn validate_execution(options: &PayOnchainOptions) -> Result<(), ApiError> {
    if options.fee_guardrail.is_some() {
        return Err(invalid("Lexe does not support a maximum network fee; omit fee_guardrail to use provider-determined fees."));
    }
    Ok(())
}

pub async fn pay(
    wallet: &LexeWallet,
    transaction: OnchainTransaction,
    options: PayOnchainOptions,
    network: &str,
) -> Result<PayOnchainResponse, ApiError> {
    let req = request(&transaction, network)?;
    // Never trust caller-provided fee_sats: Lexe cannot enforce that fee.
    validate_execution(&options)?;
    let payment = wallet.pay_onchain(req).await.map_err(|_| ApiError::Api {
        reason: "Lexe on-chain send failed or its outcome is unknown; reconcile payment history or retry with the same prepared payment id".to_owned(),
    })?;
    response(&payment, transaction.address)
}

fn response(payment: &Payment, address: String) -> Result<PayOnchainResponse, ApiError> {
    let sats = |amount: Amount| -> Result<i64, ApiError> {
        if amount.msat() % 1_000 != 0 {
            return Err(invalid(
                "Lexe returned a fractional-satoshi on-chain amount",
            ));
        }
        i64::try_from(amount.msat() / 1_000)
            .map_err(|_| invalid("Lexe amount exceeds integer range"))
    };
    let amount = sats(
        payment
            .amount
            .ok_or_else(|| invalid("Lexe payment amount is missing"))?,
    )?;
    let fee = sats(payment.fees)?;
    Ok(PayOnchainResponse {
        payment_id: Some(payment.index.to_string()),
        txid: payment.txid.map(|id| id.to_string()),
        state: match payment.status {
            PaymentStatus::Pending => "pending",
            PaymentStatus::Completed => "completed",
            PaymentStatus::Failed => "failed",
        }
        .to_owned(),
        // An idempotent retry can return an earlier payment. Prefer its actual
        // destination over the caller's request when Lexe supplies it.
        address: payment
            .address
            .as_ref()
            .map(|address| address.assume_checked_ref().to_string())
            .unwrap_or(address),
        amount_sats: amount,
        fee_sats: Some(fee),
        total_amount_sats: Some(
            amount
                .checked_add(fee)
                .ok_or_else(|| invalid("Lexe total exceeds integer range"))?,
        ),
        recipient_amount_sats: Some(amount),
        created_at: Some(payment.created_at.to_i64() / 1_000),
        raw: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params() -> PrepareOnchainTransactionParams {
        PrepareOnchainTransactionParams {
            address: "1BoatSLRHtKNngkdXEeobR76b53LETtpyT".to_owned(),
            amount_sats: 10_000,
            fee: None,
            fee_payer: None,
            description: Some("test payment".to_owned()),
            idempotency_key: Some("ab".repeat(32)),
        }
    }

    #[test]
    fn preparation_preserves_id_and_note_without_inventing_a_fee() {
        let transaction = prepare(params(), "mainnet").unwrap();
        assert_eq!(transaction.id.as_deref(), Some("ab".repeat(32).as_str()));
        assert_eq!(transaction.fee_sats, None);
        assert_eq!(transaction.fee_limit_supported, Some(false));
        assert_eq!(transaction.total_amount_sats, None);
        let first = request(&transaction, "mainnet").unwrap();
        let retry = request(&transaction, "mainnet").unwrap();
        assert_eq!(first.client_payment_id, retry.client_payment_id);
        assert_eq!(first.personal_note.as_deref(), Some("test payment"));
        assert_eq!(first.amount.msat(), 10_000_000);
    }

    #[test]
    fn fresh_preparations_have_distinct_ids() {
        let mut input = params();
        input.idempotency_key = None;
        let a = prepare(input.clone(), "mainnet").unwrap();
        let b = prepare(input, "mainnet").unwrap();
        assert_ne!(a.id, b.id);
        assert_eq!(a.id.unwrap().len(), 64);
    }

    #[test]
    fn validates_before_sending() {
        for amount in [0, -1, i64::MAX] {
            let mut input = params();
            input.amount_sats = amount;
            assert!(prepare(input, "mainnet").is_err());
        }
        assert!(prepare(params(), "testnet3").is_err());
        let mut input = params();
        input.address = "not-an-address".to_owned();
        assert!(prepare(input, "mainnet").is_err());
        let mut input = params();
        input.idempotency_key = Some("invalid".to_owned());
        assert!(prepare(input, "mainnet").is_err());
        let mut input = params();
        input.fee_payer = Some(OnchainFeePayer::Recipient);
        assert!(prepare(input, "mainnet").is_err());
        for note in ["".to_owned(), "a".repeat(201), "🦀".repeat(129)] {
            let mut input = params();
            input.description = Some(note);
            assert!(prepare(input, "mainnet").is_err());
        }
        let mut transaction = prepare(params(), "mainnet").unwrap();
        transaction.id = None;
        assert!(request(&transaction, "mainnet").is_err());
    }

    #[test]
    fn maps_supported_priorities_and_rejects_unavailable_fee_controls() {
        for (speed, expected) in [
            (OnchainFeeSpeed::Fast, ConfirmationPriority::High),
            (OnchainFeeSpeed::Normal, ConfirmationPriority::Normal),
            (OnchainFeeSpeed::Slow, ConfirmationPriority::Background),
        ] {
            let fee = OnchainFeePreference {
                preference_type: OnchainFeePreferenceType::Speed,
                speed: Some(speed),
                ..Default::default()
            };
            assert_eq!(priority(&fee).unwrap(), expected);
        }
        for preference_type in [
            OnchainFeePreferenceType::TargetConf,
            OnchainFeePreferenceType::SatsPerVbyte,
            OnchainFeePreferenceType::Backend,
        ] {
            assert!(priority(&OnchainFeePreference {
                preference_type,
                ..Default::default()
            })
            .is_err());
        }
        assert!(priority(&OnchainFeePreference {
            preference_type: OnchainFeePreferenceType::Speed,
            speed: Some(OnchainFeeSpeed::Free),
            ..Default::default()
        })
        .is_err());
    }

    #[test]
    fn provider_fees_are_default_but_explicit_limits_are_rejected() {
        assert!(validate_execution(&PayOnchainOptions::default()).is_ok());
        assert!(validate_execution(&PayOnchainOptions {
            fee_guardrail: Some(crate::OnchainFeeGuardrail {
                max_fee_sats: Some(i64::MAX),
                max_fee_percent: None
            }),
            ..Default::default()
        })
        .is_err());
        assert!(validate_execution(&PayOnchainOptions {
            dangerously_disable_fee_guardrail: true,
            ..Default::default()
        })
        .is_ok());
    }

    #[test]
    fn dangerous_flag_cannot_silently_discard_explicit_limits() {
        for fee_guardrail in [
            crate::OnchainFeeGuardrail {
                max_fee_sats: Some(500),
                max_fee_percent: None,
            },
            crate::OnchainFeeGuardrail {
                max_fee_sats: None,
                max_fee_percent: Some(5.0),
            },
            crate::OnchainFeeGuardrail {
                max_fee_sats: None,
                max_fee_percent: None,
            },
        ] {
            assert!(validate_execution(&PayOnchainOptions {
                fee_guardrail: Some(fee_guardrail),
                dangerously_disable_fee_guardrail: true,
            })
            .is_err());
        }
    }

    #[test]
    fn maps_pending_completed_and_failed_sends() {
        for status in ["pending", "completed", "failed"] {
            let payment: Payment = serde_json::from_value(serde_json::json!({
                "index": format!("0000001700000000000-os_{}", "ab".repeat(32)),
                "rail": "onchain", "kind": "onchain", "direction": "outbound",
                "amount": 10000, "fees": 250, "status": status,
                "status_msg": "test status", "txid": "cd".repeat(32),
                "address": params().address,
                "created_at": 1700000000000i64, "updated_at": 1700000000000i64
            }))
            .unwrap();
            let result = response(&payment, "different retry destination".to_owned()).unwrap();
            assert_eq!(result.address, params().address);
            assert_eq!(result.state, status);
            assert_eq!(result.amount_sats, 10_000);
            assert_eq!(result.fee_sats, Some(250));
            assert_eq!(result.total_amount_sats, Some(10_250));
            assert_eq!(result.txid, Some("cd".repeat(32)));
            assert_eq!(result.created_at, Some(1_700_000_000));
        }
    }
}
