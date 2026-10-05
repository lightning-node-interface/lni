//! Receive-only LNURL accounts. Settlement requires a preimage for the exact invoice.
use super::*;
use sha2::{Digest, Sha256};

#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
#[derive(Clone)]
pub struct LnurlReceiveInvoice {
    pub invoice: String,
    pub verify_url: Option<String>,
}

#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
#[derive(Clone)]
pub struct LnurlReceiveSettlement {
    pub settled: bool,
    pub preimage: Option<String>,
}

fn receive_error() -> ApiError {
    ApiError::LnurlError("LNURL receive request could not be verified".into())
}

/// Generates an invoice and retains its provider verification endpoint, if supported.
/// Absence of an endpoint means this receive-only account cannot automatically reconcile.
#[cfg_attr(feature = "uniffi", uniffi::export(async_runtime = "tokio"))]
pub async fn create_lnurl_receive_invoice(
    destination: String,
    amount_msats: u64,
) -> Result<LnurlReceiveInvoice, ApiError> {
    let amount = i64::try_from(amount_msats).map_err(|_| receive_error())?;
    if amount <= 0 {
        return Err(receive_error());
    }
    let url = match PaymentDestination::parse(&destination).map_err(|_| receive_error())? {
        PaymentDestination::LightningAddress { user, domain } => {
            lightning_address_to_url(&user, &domain)
        }
        PaymentDestination::LnurlPay(value) => decode_lnurl(&value).map_err(|_| receive_error())?,
        _ => return Err(receive_error()),
    };
    let metadata: LnurlPayResponse = serde_json::from_value(
        fetch_lnurl_json_value(&url)
            .await
            .map_err(|_| receive_error())?,
    )
    .map_err(|_| receive_error())?;
    if metadata.tag != "payRequest"
        || amount < metadata.min_sendable
        || amount > metadata.max_sendable
    {
        return Err(receive_error());
    }
    let callback =
        callback_url_with_amount(&metadata.callback, amount).map_err(|_| receive_error())?;
    let result = fetch_lnurl_json_value(callback.as_str())
        .await
        .map_err(|_| receive_error())?;
    let invoice = result
        .get("pr")
        .and_then(|v| v.as_str())
        .ok_or_else(receive_error)?;
    validate_invoice_amount(invoice, amount).map_err(|_| receive_error())?;
    let parsed = Bolt11Invoice::from_str(invoice).map_err(|_| receive_error())?;
    let expected = Sha256::digest(metadata.metadata.as_bytes());
    match parsed.description() {
        lightning_invoice::Bolt11InvoiceDescriptionRef::Hash(hash)
            if hash.0.to_string() == hex::encode(expected) => {}
        _ => return Err(receive_error()),
    }
    let verify_url = result
        .get("verify")
        .and_then(|v| v.as_str())
        .map(|value| {
            validate_public_https_url(value)
                .map(|url| url.to_string())
                .map_err(|_| receive_error())
        })
        .transpose()?;
    Ok(LnurlReceiveInvoice {
        invoice: invoice.to_owned(),
        verify_url,
    })
}

/// A provider's `settled` flag alone is never accepted as proof.
#[cfg_attr(feature = "uniffi", uniffi::export(async_runtime = "tokio"))]
pub async fn verify_lnurl_receive_invoice(
    invoice: String,
    verify_url: String,
) -> Result<LnurlReceiveSettlement, ApiError> {
    let parsed = Bolt11Invoice::from_str(&invoice).map_err(|_| receive_error())?;
    let response = fetch_lnurl_json_value(&verify_url)
        .await
        .map_err(|_| receive_error())?;
    verify_response(&parsed, response)
}

fn verify_response(
    parsed: &Bolt11Invoice,
    response: serde_json::Value,
) -> Result<LnurlReceiveSettlement, ApiError> {
    handle_lnurl_ok_value(&response, "LNURL verify").map_err(|_| receive_error())?;
    let settled = response
        .get("settled")
        .and_then(|v| v.as_bool())
        .ok_or_else(receive_error)?;
    if !settled {
        return Ok(LnurlReceiveSettlement {
            settled: false,
            preimage: None,
        });
    }
    let preimage = response
        .get("preimage")
        .and_then(|v| v.as_str())
        .ok_or_else(receive_error)?;
    let bytes = hex::decode(preimage).map_err(|_| receive_error())?;
    if bytes.len() != 32 || hex::encode(Sha256::digest(&bytes)) != parsed.payment_hash().to_string()
    {
        return Err(receive_error());
    }
    Ok(LnurlReceiveSettlement {
        settled: true,
        preimage: Some(hex::encode(bytes)),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use lightning::bitcoin::{
        hashes::{sha256, Hash},
        secp256k1::{Secp256k1, SecretKey},
    };
    use lightning_invoice::{Currency, InvoiceBuilder, PaymentSecret};

    #[test]
    fn verification_requires_exact_preimage_not_provider_flag() {
        let secp = Secp256k1::new();
        let key = SecretKey::from_slice(&[1; 32]).unwrap();
        let invoice = InvoiceBuilder::new(Currency::Bitcoin)
            .description("public fixture".into())
            .payment_hash(sha256::Hash::hash(&[7; 32]))
            .payment_secret(PaymentSecret([2; 32]))
            .duration_since_epoch(std::time::Duration::from_secs(1_700_000_000))
            .min_final_cltv_expiry_delta(18)
            .amount_milli_satoshis(1000)
            .build_signed(|hash| secp.sign_ecdsa_recoverable(hash, &key))
            .unwrap();
        assert!(
            verify_response(&invoice, serde_json::json!({"status":"OK","settled":true})).is_err()
        );
        assert!(verify_response(
            &invoice,
            serde_json::json!({"status":"OK","settled":true,"preimage":hex::encode([8;32])})
        )
        .is_err());
        assert!(verify_response(
            &invoice,
            serde_json::json!({"status":"ERROR","settled":true,"preimage":hex::encode([7;32])})
        )
        .is_err());
        assert!(
            verify_response(
                &invoice,
                serde_json::json!({"status":"OK","settled":true,"preimage":hex::encode([7;32])})
            )
            .unwrap()
            .settled
        );
        assert!(
            !verify_response(&invoice, serde_json::json!({"status":"OK","settled":false}))
                .unwrap()
                .settled
        );
    }
}
