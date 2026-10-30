//! Wallet-specific fee estimates through Lexe's stable SDK. Sends cannot cap fees.
use std::str::FromStr;

use lexe::{
    types::{
        bitcoin::{Amount, ConfirmationPriority},
        command::{PayOnchainPreflightRequest, PayOnchainPreflightResponse, PayOnchainRequest},
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

fn prepare_local(
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

pub async fn prepare(
    wallet: &LexeWallet,
    params: PrepareOnchainTransactionParams,
    network: &str,
) -> Result<OnchainTransaction, ApiError> {
    prepare_using(params, network, |req| wallet.pay_onchain_preflight(req)).await
}

// This operation only accepts preflight requests. Never invoke it from pay:
// upstream rejects preflight for existing IDs, but send retries must still work.
async fn prepare_using<F, Fut>(
    params: PrepareOnchainTransactionParams,
    network: &str,
    preflight: F,
) -> Result<OnchainTransaction, ApiError>
where
    F: FnOnce(PayOnchainPreflightRequest) -> Fut,
    Fut: std::future::Future<Output = lexe::anyhow::Result<PayOnchainPreflightResponse>>,
{
    let mut transaction = prepare_local(params, network)?;
    let req = request(&transaction, network)?;
    let estimates = preflight(PayOnchainPreflightRequest {
        client_payment_id: req.client_payment_id,
        address: req.address,
        amount: req.amount,
    })
    .await
    // Stable SDK errors are opaque. Do not parse their text or expose payloads
    // that may contain wallet details; insufficient funds is not a distinct code.
    .map_err(|_| ApiError::Api {
        reason: "Lexe on-chain preflight failed; check available funds, spend authorization, node/SDK compatibility, and whether the payment ID was already submitted. No payment was submitted by this preflight.".to_owned(),
    })?;
    let estimate = match priority(&transaction.fee)? {
        ConfirmationPriority::Background => estimates.background_fees,
        ConfirmationPriority::Normal => estimates.normal_fees,
        ConfirmationPriority::High => estimates.high_fees.ok_or_else(|| ApiError::Api {
            reason: "Lexe fast fee estimate is unavailable; select normal or slow".to_owned(),
        })?,
    };
    let fee = sats(estimate)?;
    transaction.fee_sats = Some(fee);
    transaction.total_amount_sats = Some(checked_total(transaction.amount_sats, fee)?);
    Ok(transaction)
}

fn sats(amount: Amount) -> Result<i64, ApiError> {
    if amount.msat() % 1_000 != 0 {
        return Err(invalid(
            "Lexe returned a fractional-satoshi on-chain amount",
        ));
    }
    i64::try_from(amount.msat() / 1_000).map_err(|_| invalid("Lexe amount exceeds integer range"))
}

fn checked_total(amount: i64, fee: i64) -> Result<i64, ApiError> {
    amount
        .checked_add(fee)
        .ok_or_else(|| invalid("Lexe total exceeds integer range"))
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
        total_amount_sats: Some(checked_total(amount, fee)?),
        recipient_amount_sats: Some(amount),
        created_at: Some(payment.created_at.to_i64() / 1_000),
        raw: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn estimates() -> PayOnchainPreflightResponse {
        PayOnchainPreflightResponse {
            background_fees: Amount::from_msat(100_000),
            normal_fees: Amount::from_msat(250_000),
            high_fees: Some(Amount::from_msat(500_000)),
        }
    }

    #[tokio::test]
    async fn preflight_maps_each_priority_and_refresh_preserves_identity() {
        for (speed, expected) in [
            (None, 250),
            (Some(OnchainFeeSpeed::Slow), 100),
            (Some(OnchainFeeSpeed::Normal), 250),
            (Some(OnchainFeeSpeed::Fast), 500),
        ] {
            let mut input = params();
            input.fee = speed.map(|speed| OnchainFeePreference {
                preference_type: OnchainFeePreferenceType::Speed,
                speed: Some(speed),
                ..Default::default()
            });
            for _ in 0..2 {
                let calls = std::cell::Cell::new(0);
                let prepared = prepare_using(input.clone(), "mainnet", |req| {
                    calls.set(calls.get() + 1);
                    assert_eq!(req.client_payment_id.unwrap().to_string(), "ab".repeat(32));
                    assert_eq!(req.amount.msat(), 10_000_000);
                    assert_eq!(req.address.assume_checked().to_string(), input.address);
                    std::future::ready(Ok(estimates()))
                })
                .await
                .unwrap();
                assert_eq!(calls.get(), 1);
                assert_eq!(prepared.fee_sats, Some(expected));
                assert_eq!(prepared.total_amount_sats, Some(10_000 + expected));
                assert_eq!(prepared.recipient_amount_sats, Some(10_000));
                assert_eq!(prepared.fee_limit_supported, Some(false));
                let send = request(&prepared, "mainnet").unwrap();
                assert_eq!(send.personal_note.as_deref(), Some("test payment"));
                assert_eq!(send.client_payment_id.unwrap().to_string(), "ab".repeat(32));
            }
        }
    }

    #[tokio::test]
    async fn missing_fast_estimate_never_substitutes_another_priority() {
        for speed in [
            OnchainFeeSpeed::Slow,
            OnchainFeeSpeed::Normal,
            OnchainFeeSpeed::Fast,
        ] {
            let mut input = params();
            input.fee = Some(OnchainFeePreference {
                preference_type: OnchainFeePreferenceType::Speed,
                speed: Some(speed.clone()),
                ..Default::default()
            });
            let result = prepare_using(input, "mainnet", |_| async {
                let mut fees = estimates();
                fees.high_fees = None;
                Ok(fees)
            })
            .await;
            if speed == OnchainFeeSpeed::Fast {
                assert!(result
                    .unwrap_err()
                    .to_string()
                    .contains("fast fee estimate is unavailable"));
            } else {
                assert!(result.is_ok());
            }
        }
    }

    #[tokio::test]
    async fn invalid_input_does_not_call_preflight_and_errors_do_not_return_preparations() {
        let mut input = params();
        input.amount_sats = -1;
        assert!(prepare_using(input, "mainnet", |_| async {
            panic!("invalid input must not reach the network")
        })
        .await
        .is_err());

        // The stable SDK exposes opaque anyhow errors, not typed funds/version codes.
        for cause in [
            "insufficient funds",
            "unknown endpoint",
            "authentication",
            "timeout",
        ] {
            let calls = std::cell::Cell::new(0);
            let error = prepare_using(params(), "mainnet", |_| {
                calls.set(calls.get() + 1);
                std::future::ready(Err(lexe::anyhow::anyhow!(
                    "{cause}: private wallet payload"
                )))
            })
            .await
            .unwrap_err()
            .to_string();
            assert_eq!(calls.get(), 1);
            assert!(error.contains("preflight failed"));
            assert!(error.contains("No payment was submitted"));
            assert!(!error.contains("private wallet payload"));
        }
    }

    #[tokio::test]
    async fn fee_conversion_is_exact_and_rejects_fractional_satoshis() {
        let result = prepare_using(params(), "mainnet", |_| async {
            let mut fees = estimates();
            fees.normal_fees = Amount::from_msat(1001);
            Ok(fees)
        })
        .await;
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("fractional-satoshi"));
        assert_eq!(
            sats(Amount::from_msat(2_100_000_000_000_000_000)).unwrap(),
            2_100_000_000_000_000
        );
        assert_eq!(checked_total(10_000, 250).unwrap(), 10_250);
        assert!(checked_total(i64::MAX, 1).is_err());
    }

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
        let transaction = prepare_local(params(), "mainnet").unwrap();
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
        let a = prepare_local(input.clone(), "mainnet").unwrap();
        let b = prepare_local(input, "mainnet").unwrap();
        assert_ne!(a.id, b.id);
        assert_eq!(a.id.unwrap().len(), 64);
    }

    #[test]
    fn validates_before_sending() {
        for amount in [0, -1, i64::MAX] {
            let mut input = params();
            input.amount_sats = amount;
            assert!(prepare_local(input, "mainnet").is_err());
        }
        assert!(prepare_local(params(), "testnet3").is_err());
        let mut input = params();
        input.address = "not-an-address".to_owned();
        assert!(prepare_local(input, "mainnet").is_err());
        let mut input = params();
        input.idempotency_key = Some("invalid".to_owned());
        assert!(prepare_local(input, "mainnet").is_err());
        let mut input = params();
        input.fee_payer = Some(OnchainFeePayer::Recipient);
        assert!(prepare_local(input, "mainnet").is_err());
        for note in ["".to_owned(), "a".repeat(201), "🦀".repeat(129)] {
            let mut input = params();
            input.description = Some(note);
            assert!(prepare_local(input, "mainnet").is_err());
        }
        let mut transaction = prepare_local(params(), "mainnet").unwrap();
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
