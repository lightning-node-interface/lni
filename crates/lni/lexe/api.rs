use std::{str::FromStr, sync::Arc, time::Duration};

use lexe::{
    types::{
        bitcoin::{Amount, Offer as LexeOffer},
        command::{
            CreateInvoiceRequest as LexeCreateInvoiceRequest,
            CreateOfferRequest as LexeCreateOfferRequest,
            PayInvoiceRequest as LexePayInvoiceRequest, PayOfferRequest as LexePayOfferRequest,
        },
        payment::{
            Order, Payment, PaymentDirection, PaymentFilter, PaymentPreimage, PaymentStatus,
        },
    },
    wallet::LexeWallet,
};

use crate::{
    ApiError, CreateInvoiceParams, CreateOfferParams, InvoiceType, ListTransactionsParams,
    LookupInvoiceParams, NodeInfo, Offer, OnInvoiceEventCallback, OnInvoiceEventParams,
    PayInvoiceParams, PayInvoiceResponse, Permissions, Transaction,
};

use super::LexeHumanBitcoinAddress;

const PAYMENT_PAGE_SIZE: usize = 100;

fn api_error(context: &str, error: impl std::fmt::Display) -> ApiError {
    ApiError::Api {
        reason: format!("{context}: {error}"),
    }
}

fn invalid_input(message: impl Into<String>) -> ApiError {
    ApiError::InvalidInput(message.into())
}

fn amount_from_msats(amount_msats: i64, field: &str) -> Result<Amount, ApiError> {
    let amount_msats = u64::try_from(amount_msats)
        .map_err(|_| invalid_input(format!("{field} must be non-negative")))?;
    Ok(Amount::from_msat(amount_msats))
}

fn optional_amount_from_msats(
    amount_msats: Option<i64>,
    field: &str,
) -> Result<Option<Amount>, ApiError> {
    amount_msats
        .map(|amount| amount_from_msats(amount, field))
        .transpose()
}

fn amount_to_msats(amount: Amount, field: &str) -> Result<i64, ApiError> {
    i64::try_from(amount.msat())
        .map_err(|_| api_error(field, "amount exceeds LNI's signed integer range"))
}

fn timestamp_secs(timestamp: lexe::types::util::TimestampMs) -> i64 {
    timestamp.to_i64() / 1_000
}

fn payment_preimage_to_hex(preimage: &PaymentPreimage) -> String {
    hex::encode(AsRef::<[u8]>::as_ref(preimage))
}

fn validate_create_invoice_params(params: &CreateInvoiceParams) -> Result<(), ApiError> {
    if !matches!(params.get_invoice_type(), InvoiceType::Bolt11) {
        return Err(invalid_input(
            "Lexe create_invoice supports only BOLT 11; use create_offer for BOLT 12",
        ));
    }
    if params
        .description_hash
        .as_deref()
        .is_some_and(|v| !v.is_empty())
    {
        return Err(invalid_input(
            "Lexe create_invoice does not support description_hash",
        ));
    }
    if params.offer.is_some()
        || params.r_preimage.is_some()
        || params.is_blinded.unwrap_or(false)
        || params.is_keysend.unwrap_or(false)
        || params.is_amp.unwrap_or(false)
        || params.is_private.unwrap_or(false)
    {
        return Err(invalid_input(
            "Lexe create_invoice received unsupported invoice options",
        ));
    }
    Ok(())
}

fn validate_pay_invoice_params(params: &PayInvoiceParams) -> Result<(), ApiError> {
    if params.fee_limit_msat.is_some() || params.fee_limit_percentage.is_some() {
        return Err(invalid_input(
            "Lexe does not currently expose a pay_invoice fee limit",
        ));
    }
    if params.max_parts.is_some()
        || params.first_hop_pubkey.is_some()
        || params.last_hop_pubkey.is_some()
        || params.allow_self_payment.unwrap_or(false)
        || params.is_amp.unwrap_or(false)
    {
        return Err(invalid_input(
            "Lexe pay_invoice received unsupported routing options",
        ));
    }
    if params.invoice.trim().is_empty() {
        return Err(invalid_input("invoice must not be empty"));
    }
    if params.timeout_seconds.is_some_and(|timeout| timeout <= 0) {
        return Err(invalid_input("timeout_seconds must be positive"));
    }
    Ok(())
}

fn list_transactions_window(params: &ListTransactionsParams) -> Result<(usize, usize), ApiError> {
    let from =
        usize::try_from(params.from).map_err(|_| invalid_input("from must be non-negative"))?;
    let limit =
        usize::try_from(params.limit).map_err(|_| invalid_input("limit must be non-negative"))?;
    if limit > PAYMENT_PAGE_SIZE {
        return Err(invalid_input(format!(
            "limit must not exceed {PAYMENT_PAGE_SIZE}"
        )));
    }
    Ok((from, limit))
}

fn payment_selector_is_empty(payment_hash: Option<&str>, search: Option<&str>) -> bool {
    payment_hash.is_none_or(str::is_empty) && search.is_none_or(str::is_empty)
}

fn lni_transaction_type(direction: PaymentDirection) -> Option<&'static str> {
    match direction {
        PaymentDirection::Inbound => Some("incoming"),
        PaymentDirection::Outbound => Some("outgoing"),
        // Lexe info payments are balance-neutral journal entries, not
        // incoming or outgoing Lightning transactions.
        PaymentDirection::Info => None,
    }
}

fn payment_to_transaction(payment: &Payment) -> Result<Transaction, ApiError> {
    let invoice = payment
        .invoice
        .as_deref()
        .map(ToString::to_string)
        .unwrap_or_default();
    let description = payment
        .invoice
        .as_deref()
        .and_then(|invoice| invoice.description_str())
        .unwrap_or_default()
        .to_owned();

    let type_ = lni_transaction_type(payment.direction).ok_or_else(|| {
        api_error(
            "Lexe payment",
            "info journal entry is not an LNI transaction",
        )
    })?;

    Ok(Transaction {
        type_: type_.to_owned(),
        invoice,
        description,
        description_hash: String::new(),
        preimage: payment
            .preimage
            .as_ref()
            .map(payment_preimage_to_hex)
            .unwrap_or_default(),
        payment_hash: payment
            .hash
            .as_ref()
            .map(ToString::to_string)
            .unwrap_or_default(),
        amount_msats: payment
            .amount
            .map(|amount| amount_to_msats(amount, "payment amount"))
            .transpose()?
            .unwrap_or_default(),
        fees_paid: amount_to_msats(payment.fees, "payment fees")?,
        created_at: timestamp_secs(payment.created_at),
        expires_at: payment.expires_at.map(timestamp_secs).unwrap_or_default(),
        settled_at: payment
            .finalized_at
            .filter(|_| payment.status == PaymentStatus::Completed)
            .map(timestamp_secs)
            .unwrap_or_default(),
        payer_note: payment
            .message
            .clone()
            .or_else(|| payment.personal_note.clone()),
        external_id: Some(payment.index.to_string()),
    })
}

fn completed_payment_response(payment: Payment) -> Result<PayInvoiceResponse, ApiError> {
    match payment.status {
        PaymentStatus::Failed => {
            return Err(ApiError::Nwc {
                code: "PAYMENT_FAILED".to_owned(),
                message: payment.status_msg,
            });
        }
        PaymentStatus::Pending => {
            return Err(api_error(
                "Lexe payment",
                "payment returned before reaching a terminal state",
            ));
        }
        PaymentStatus::Completed => {}
    }

    let payment_hash = payment
        .hash
        .map(|hash| hash.to_string())
        .ok_or_else(|| api_error("Lexe payment", "completed payment has no payment hash"))?;
    let preimage = payment
        .preimage
        .map(|preimage| payment_preimage_to_hex(&preimage))
        .ok_or_else(|| api_error("Lexe payment", "completed payment has no preimage"))?;

    Ok(PayInvoiceResponse {
        payment_hash,
        preimage,
        fee_msats: amount_to_msats(payment.fees, "payment fees")?,
    })
}

fn payment_matches(
    payment: &Payment,
    payment_hash: Option<&str>,
    search: Option<&str>,
    created_after: Option<i64>,
    created_before: Option<i64>,
) -> bool {
    let created_at = timestamp_secs(payment.created_at);
    if created_after.is_some_and(|after| created_at < after)
        || created_before.is_some_and(|before| created_at > before)
    {
        return false;
    }

    if let Some(expected_hash) = payment_hash.filter(|hash| !hash.is_empty()) {
        let matches_hash = payment
            .hash
            .as_ref()
            .is_some_and(|hash| hash.to_string().eq_ignore_ascii_case(expected_hash));
        if !matches_hash {
            return false;
        }
    }

    let Some(search) = search.filter(|search| !search.is_empty()) else {
        return true;
    };
    let search = search.to_ascii_lowercase();
    let contains = |value: Option<&str>| {
        value.is_some_and(|value| value.to_ascii_lowercase().contains(&search))
    };

    contains(payment.message.as_deref())
        || contains(payment.personal_note.as_deref())
        || contains(Some(&payment.status_msg))
        || contains(
            payment
                .invoice
                .as_deref()
                .and_then(|invoice| invoice.description_str()),
        )
        || payment
            .invoice
            .as_deref()
            .is_some_and(|invoice| invoice.to_string().to_ascii_lowercase().contains(&search))
        || payment
            .hash
            .as_ref()
            .is_some_and(|hash| hash.to_string().to_ascii_lowercase().contains(&search))
}

async fn matching_payments(
    wallet: &LexeWallet,
    payment_hash: Option<&str>,
    search: Option<&str>,
    created_after: Option<i64>,
    created_before: Option<i64>,
    skip: usize,
    limit: usize,
) -> Result<Vec<Payment>, ApiError> {
    if limit == 0 {
        return Ok(Vec::new());
    }

    wallet
        .sync_payments()
        .await
        .map_err(|error| api_error("Failed to sync Lexe payments", error))?;

    let mut after = None;
    let mut skipped = 0usize;
    let mut matches = Vec::with_capacity(limit.min(PAYMENT_PAGE_SIZE));

    loop {
        let page = wallet
            .list_payments(
                &PaymentFilter::All,
                Some(Order::Desc),
                Some(PAYMENT_PAGE_SIZE),
                after.as_ref(),
            )
            .map_err(|error| api_error("Failed to list Lexe payments", error))?;
        let next_index = page.next_index;

        for payment in page.payments {
            if lni_transaction_type(payment.direction).is_none() {
                continue;
            }
            if !payment_matches(
                &payment,
                payment_hash,
                search,
                created_after,
                created_before,
            ) {
                continue;
            }
            if skipped < skip {
                skipped += 1;
                continue;
            }
            matches.push(payment);
            if matches.len() == limit {
                return Ok(matches);
            }
        }

        match next_index {
            Some(next) if Some(next) != after => after = Some(next),
            _ => return Ok(matches),
        }
    }
}

fn indeterminate_payment_error(detail: impl std::fmt::Display) -> ApiError {
    ApiError::Nwc {
        code: "PAYMENT_INDETERMINATE".to_owned(),
        message: format!(
            "Lexe payment timed out and its outcome is indeterminate ({detail}); \
             call lookup_invoice with the invoice payment hash before retrying"
        ),
    }
}

async fn reconcile_timed_out_invoice_payment(
    wallet: &LexeWallet,
    payment_hash: &str,
) -> Result<PayInvoiceResponse, ApiError> {
    let payments = matching_payments(wallet, Some(payment_hash), None, None, None, 0, 1)
        .await
        .map_err(|error| indeterminate_payment_error(format!("reconciliation failed: {error}")))?;

    let Some(payment) = payments.into_iter().next() else {
        return Err(indeterminate_payment_error(
            "the payment was not found during reconciliation",
        ));
    };

    if payment.status == PaymentStatus::Pending {
        return Err(indeterminate_payment_error("the payment is still pending"));
    }

    completed_payment_response(payment)
}

pub async fn get_info(wallet: &LexeWallet, network: &str) -> Result<NodeInfo, ApiError> {
    let info = wallet
        .node_info()
        .await
        .map_err(|error| api_error("Failed to get Lexe node info", error))?;

    Ok(NodeInfo {
        alias: "Lexe".to_owned(),
        color: String::new(),
        pubkey: info.node_pk.to_string(),
        network: network.to_owned(),
        block_height: 0,
        block_hash: String::new(),
        send_balance_msat: amount_to_msats(
            info.lightning_sendable_balance,
            "Lexe sendable balance",
        )?,
        receive_balance_msat: 0,
        fee_credit_balance_msat: 0,
        unsettled_send_balance_msat: 0,
        unsettled_receive_balance_msat: 0,
        pending_open_send_balance: 0,
        pending_open_receive_balance: 0,
    })
}

pub async fn get_human_bitcoin_address(
    wallet: &LexeWallet,
) -> Result<LexeHumanBitcoinAddress, ApiError> {
    let response = wallet
        .get_human_bitcoin_address()
        .await
        .map_err(|error| api_error("Failed to get Lexe Human Bitcoin Address", error))?;

    Ok(LexeHumanBitcoinAddress {
        human_bitcoin_address: response.human_bitcoin_address,
        lightning_address: response.lightning_address,
        offer: response.offer.to_string(),
        updatable: response.updatable,
    })
}

pub async fn create_invoice(
    wallet: &LexeWallet,
    params: CreateInvoiceParams,
) -> Result<Transaction, ApiError> {
    validate_create_invoice_params(&params)?;
    let expiration_secs = params
        .expiry
        .map(|expiry| {
            u32::try_from(expiry)
                .map_err(|_| invalid_input("expiry must fit in a positive u32 number of seconds"))
        })
        .transpose()?;

    let response = wallet
        .create_invoice(LexeCreateInvoiceRequest {
            expiration_secs,
            amount: optional_amount_from_msats(params.amount_msats, "amount_msats")?,
            description: params.description,
            personal_note: None,
            partner_pk: None,
            partner_prop_fee: None,
            partner_base_fee: None,
        })
        .await
        .map_err(|error| api_error("Failed to create Lexe invoice", error))?;

    Ok(Transaction {
        type_: "incoming".to_owned(),
        invoice: response.invoice.to_string(),
        description: response.description.unwrap_or_default(),
        description_hash: String::new(),
        preimage: String::new(),
        payment_hash: response.payment_hash.to_string(),
        amount_msats: response
            .amount
            .map(|amount| amount_to_msats(amount, "invoice amount"))
            .transpose()?
            .unwrap_or_default(),
        fees_paid: 0,
        created_at: timestamp_secs(response.created_at),
        expires_at: timestamp_secs(response.expires_at),
        settled_at: 0,
        payer_note: None,
        external_id: Some(response.index.to_string()),
    })
}

pub async fn pay_invoice(
    wallet: &LexeWallet,
    params: PayInvoiceParams,
) -> Result<PayInvoiceResponse, ApiError> {
    validate_pay_invoice_params(&params)?;
    let invoice = lexe::types::bitcoin::Invoice::from_str(&params.invoice)
        .map_err(|error| invalid_input(format!("Invalid BOLT 11 invoice: {error}")))?;
    let payment_hash = invoice.payment_hash().to_string();
    let request = LexePayInvoiceRequest {
        invoice,
        fallback_amount: optional_amount_from_msats(params.amount_msats, "amount_msats")?,
        personal_note: None,
    };

    let payment = if let Some(timeout_secs) = params.timeout_seconds {
        match tokio::time::timeout(
            Duration::from_secs(timeout_secs as u64),
            wallet.pay_invoice(request),
        )
        .await
        {
            Ok(payment) => payment,
            Err(_) => {
                return reconcile_timed_out_invoice_payment(wallet, &payment_hash).await;
            }
        }
    } else {
        wallet.pay_invoice(request).await
    }
    .map_err(|error| api_error("Failed to pay Lexe invoice", error))?;

    completed_payment_response(payment)
}

pub async fn create_offer(
    wallet: &LexeWallet,
    params: CreateOfferParams,
) -> Result<Offer, ApiError> {
    let response = wallet
        .create_offer(LexeCreateOfferRequest {
            description: params.description.clone(),
            min_amount: optional_amount_from_msats(params.amount_msats, "amount_msats")?,
            expiration_secs: None,
        })
        .await
        .map_err(|error| api_error("Failed to create Lexe offer", error))?;
    let offer = response.offer;

    Ok(Offer {
        offer_id: offer.id().to_string(),
        bolt12: offer.to_string(),
        label: params.description,
        active: Some(!offer.is_expired()),
        single_use: Some(false),
        used: None,
        amount_msats: offer
            .min_amount()
            .map(|amount| amount_to_msats(amount, "offer amount"))
            .transpose()?,
    })
}

pub async fn pay_offer(
    wallet: &LexeWallet,
    offer: String,
    amount_msats: i64,
    payer_note: Option<String>,
) -> Result<PayInvoiceResponse, ApiError> {
    let offer = LexeOffer::from_str(&offer)
        .map_err(|error| invalid_input(format!("Invalid BOLT 12 offer: {error}")))?;
    let payment = wallet
        .pay_offer(LexePayOfferRequest {
            offer,
            amount: amount_from_msats(amount_msats, "amount_msats")?,
            message: payer_note,
            personal_note: None,
        })
        .await
        .map_err(|error| api_error("Failed to pay Lexe offer", error))?;

    completed_payment_response(payment)
}

pub async fn list_transactions(
    wallet: &LexeWallet,
    params: ListTransactionsParams,
) -> Result<Vec<Transaction>, ApiError> {
    let (from, limit) = list_transactions_window(&params)?;
    let payments = matching_payments(
        wallet,
        params.payment_hash.as_deref(),
        params.search.as_deref(),
        params.created_after,
        params.created_before,
        from,
        limit,
    )
    .await?;

    payments.iter().map(payment_to_transaction).collect()
}

pub async fn lookup_invoice(
    wallet: &LexeWallet,
    params: LookupInvoiceParams,
) -> Result<Transaction, ApiError> {
    if payment_selector_is_empty(params.payment_hash.as_deref(), params.search.as_deref()) {
        return Err(invalid_input(
            "lookup_invoice requires payment_hash or search",
        ));
    }

    matching_payments(
        wallet,
        params.payment_hash.as_deref(),
        params.search.as_deref(),
        None,
        None,
        0,
        1,
    )
    .await?
    .first()
    .map(payment_to_transaction)
    .transpose()?
    .ok_or_else(|| ApiError::Nwc {
        code: "NOT_FOUND".to_owned(),
        message: "Lexe payment not found".to_owned(),
    })
}

pub async fn on_invoice_events(
    wallet: Arc<LexeWallet>,
    params: OnInvoiceEventParams,
    callback: Arc<dyn OnInvoiceEventCallback>,
) {
    if payment_selector_is_empty(params.payment_hash.as_deref(), params.search.as_deref()) {
        callback.failure(None);
        return;
    }

    if params.polling_delay_sec <= 0 || params.max_polling_sec <= 0 {
        callback.failure(None);
        return;
    }

    let started = tokio::time::Instant::now();
    let max_polling = Duration::from_secs(params.max_polling_sec as u64);
    let polling_delay = Duration::from_secs(params.polling_delay_sec as u64);
    let mut last_transaction = None;

    while started.elapsed() < max_polling {
        match matching_payments(
            &wallet,
            params.payment_hash.as_deref(),
            params.search.as_deref(),
            None,
            None,
            0,
            1,
        )
        .await
        {
            Ok(payments) => {
                if let Some(payment) = payments.first() {
                    match payment_to_transaction(payment) {
                        Ok(transaction) => {
                            last_transaction = Some(transaction);
                            match payment.status {
                                PaymentStatus::Completed => {
                                    callback.success(last_transaction);
                                    return;
                                }
                                PaymentStatus::Failed => {
                                    callback.failure(last_transaction);
                                    return;
                                }
                                PaymentStatus::Pending => {
                                    callback.pending(last_transaction.take());
                                }
                            }
                        }
                        Err(_) => {
                            callback.failure(None);
                            return;
                        }
                    }
                }
            }
            Err(_) => {
                callback.failure(last_transaction);
                return;
            }
        }

        tokio::time::sleep(polling_delay).await;
    }

    callback.failure(last_transaction);
}

pub fn permissions() -> Permissions {
    Permissions {
        get_info: true,
        create_invoice: true,
        pay_invoice: true,
        create_offer: true,
        get_offer: false,
        list_offers: false,
        pay_offer: true,
        lookup_invoice: true,
        list_transactions: true,
        decode: true,
        on_invoice_events: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn negative_amount_is_rejected() {
        let error = amount_from_msats(-1, "amount_msats").unwrap_err();
        assert!(matches!(error, ApiError::InvalidInput(_)));
    }

    #[test]
    fn unsupported_fee_limit_is_rejected() {
        let params = PayInvoiceParams {
            invoice: "lnbc1example".to_owned(),
            fee_limit_msat: Some(1_000),
            ..Default::default()
        };
        let error = validate_pay_invoice_params(&params).unwrap_err();
        assert!(matches!(error, ApiError::InvalidInput(_)));
    }

    #[test]
    fn explicitly_disabled_routing_options_are_accepted() {
        let params = PayInvoiceParams {
            invoice: "lnbc1example".to_owned(),
            allow_self_payment: Some(false),
            is_amp: Some(false),
            ..Default::default()
        };

        validate_pay_invoice_params(&params).unwrap();
    }

    #[test]
    fn enabled_routing_options_are_rejected() {
        for params in [
            PayInvoiceParams {
                invoice: "lnbc1example".to_owned(),
                allow_self_payment: Some(true),
                ..Default::default()
            },
            PayInvoiceParams {
                invoice: "lnbc1example".to_owned(),
                is_amp: Some(true),
                ..Default::default()
            },
        ] {
            let error = validate_pay_invoice_params(&params).unwrap_err();
            assert!(matches!(error, ApiError::InvalidInput(_)));
        }
    }

    #[test]
    fn transaction_limit_is_bounded() {
        let params = |limit| ListTransactionsParams {
            from: 0,
            limit,
            payment_hash: None,
            search: None,
            created_after: None,
            created_before: None,
        };

        assert_eq!(
            list_transactions_window(&params(PAYMENT_PAGE_SIZE as i64)).unwrap(),
            (0, PAYMENT_PAGE_SIZE)
        );
        let error = list_transactions_window(&params(PAYMENT_PAGE_SIZE as i64 + 1)).unwrap_err();
        assert!(matches!(error, ApiError::InvalidInput(_)));
    }

    #[test]
    fn payment_selector_requires_a_nonempty_value() {
        assert!(payment_selector_is_empty(None, None));
        assert!(payment_selector_is_empty(Some(""), Some("")));
        assert!(!payment_selector_is_empty(Some("payment-hash"), None));
        assert!(!payment_selector_is_empty(None, Some("search")));
    }

    #[test]
    fn balance_neutral_info_payments_are_not_lni_transactions() {
        assert_eq!(
            lni_transaction_type(PaymentDirection::Inbound),
            Some("incoming")
        );
        assert_eq!(
            lni_transaction_type(PaymentDirection::Outbound),
            Some("outgoing")
        );
        assert_eq!(lni_transaction_type(PaymentDirection::Info), None);
    }

    #[test]
    fn indeterminate_payment_error_requires_lookup_before_retry() {
        let error = indeterminate_payment_error("payment is still pending");
        let ApiError::Nwc { code, message } = error else {
            panic!("expected a structured LNI error");
        };
        assert_eq!(code, "PAYMENT_INDETERMINATE");
        assert!(message.contains("indeterminate"));
        assert!(message.contains("lookup_invoice"));
        assert!(message.contains("before retrying"));
    }

    #[test]
    fn payment_preimage_is_encoded_instead_of_display_redacted() {
        let expected = "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f";
        let preimage = expected.parse::<PaymentPreimage>().unwrap();

        assert_eq!(preimage.to_string(), "..");
        assert_eq!(payment_preimage_to_hex(&preimage), expected);
    }
}
