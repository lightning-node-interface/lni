use super::types::{
    Bolt11Req, Bolt11Resp, Bolt12Req, InfoResponse, InvoiceResponse, OutgoingPaymentResponse,
    PayResponse, PhoenixPayInvoiceResp,
};
use super::PhoenixdConfig;
use crate::ListTransactionsParams;
use crate::{
    phoenixd::types::GetBalanceResponse, ApiError, CreateOfferParams, InvoiceType, NodeInfo, Offer, OnInvoiceEventCallback,
    OnInvoiceEventParams, OnchainFeePayer, OnchainFeePreference, OnchainFeePreferenceType,
    OnchainTransaction, PayInvoiceParams, PayInvoiceResponse, PayOnchainOptions,
    PayOnchainResponse, PrepareOnchainTransactionParams, Transaction,
};
use lightning_invoice::Bolt11Invoice;
use serde_urlencoded;
use std::str::FromStr;
use std::time::Duration;
use tokio::time::sleep;

// TODO
// list_channels
// get_balance

// https://phoenix.acinq.co/server/api

struct PhoenixdOnchainFeeRequest {
    feerate_sat_byte: i64,
}

fn client(config: &PhoenixdConfig) -> reqwest::Client {
    // Create HTTP client with optional SOCKS5 proxy following LND pattern
    if let Some(proxy_url) = config.socks5_proxy.clone() {
        if !proxy_url.is_empty() {
            let mut client_builder = reqwest::Client::builder();
            if config.accept_invalid_certs.unwrap_or(false) {
                client_builder = client_builder.danger_accept_invalid_certs(true);
            }
            if let Some(timeout) = config.http_timeout {
                client_builder = client_builder.timeout(std::time::Duration::from_secs(timeout as u64));
            }
            
            match reqwest::Proxy::all(&proxy_url) {
                Ok(proxy) => {
                    match client_builder.proxy(proxy).build() {
                        Ok(client) => return client,
                        Err(_) => {} // Fall through to default client creation
                    }
                }
                Err(_) => {} // Fall through to default client creation
            }
        }
    }
    
    // Default client creation
    let mut client_builder = reqwest::ClientBuilder::new();
    if config.accept_invalid_certs.unwrap_or(false) {
        client_builder = client_builder.danger_accept_invalid_certs(true);
    }
    if let Some(timeout) = config.http_timeout {
        client_builder = client_builder.timeout(std::time::Duration::from_secs(timeout as u64));
    }
    client_builder.build().unwrap_or_else(|_| reqwest::Client::new())
}

fn assert_valid_onchain_amount(amount_sats: i64) -> Result<(), ApiError> {
    if amount_sats <= 0 {
        return Err(ApiError::InvalidInput(
            "pay_onchain requires a positive amount_sats".to_string(),
        ));
    }

    Ok(())
}

fn resolve_phoenixd_fee_payer(
    fee_payer: Option<OnchainFeePayer>,
) -> Result<OnchainFeePayer, ApiError> {
    match fee_payer.unwrap_or(OnchainFeePayer::Sender) {
        OnchainFeePayer::Sender => Ok(OnchainFeePayer::Sender),
        OnchainFeePayer::Recipient => Err(ApiError::InvalidInput(
            "Phoenixd pay_onchain only supports sender-paid on-chain fees".to_string(),
        )),
    }
}

fn resolve_phoenixd_fee_request(
    fee: &OnchainFeePreference,
) -> Result<PhoenixdOnchainFeeRequest, ApiError> {
    match fee.preference_type {
        OnchainFeePreferenceType::SatsPerVbyte => {
            let sats_per_vbyte = fee.sats_per_vbyte.ok_or_else(|| {
                ApiError::InvalidInput(
                    "Phoenixd sats_per_vbyte fee preference requires a fee rate".to_string(),
                )
            })?;
            if sats_per_vbyte <= 0.0 {
                return Err(ApiError::InvalidInput(
                    "Phoenixd sats_per_vbyte fee preference requires a positive fee rate".to_string(),
                ));
            }

            Ok(PhoenixdOnchainFeeRequest {
                feerate_sat_byte: sats_per_vbyte.ceil() as i64,
            })
        }
        OnchainFeePreferenceType::Backend => {
            let feerate = fee
                .backend
                .clone()
                .unwrap_or_default()
                .parse::<i64>()
                .map_err(|_| {
                    ApiError::InvalidInput(
                        "Phoenixd backend fee preference must be a positive integer feerateSatByte value".to_string(),
                    )
                })?;
            if feerate <= 0 {
                return Err(ApiError::InvalidInput(
                    "Phoenixd backend fee preference must be a positive integer feerateSatByte value".to_string(),
                ));
            }

            Ok(PhoenixdOnchainFeeRequest {
                feerate_sat_byte: feerate,
            })
        }
        OnchainFeePreferenceType::Default
        | OnchainFeePreferenceType::Speed
        | OnchainFeePreferenceType::TargetConf => Err(ApiError::InvalidInput(
            "Phoenixd pay_onchain requires an explicit sats_per_vbyte fee preference".to_string(),
        )),
    }
}

fn assert_onchain_fee_guardrail(
    transaction: &OnchainTransaction,
    options: PayOnchainOptions,
) -> Result<(), ApiError> {
    if options.dangerously_disable_fee_guardrail {
        return Ok(());
    }

    let guardrail = options.fee_guardrail.unwrap_or_default();
    let max_fee_sats = guardrail
        .max_fee_sats
        .unwrap_or(crate::types::DEFAULT_ONCHAIN_MAX_FEE_SATS);
    let max_fee_percent = guardrail
        .max_fee_percent
        .unwrap_or(crate::types::DEFAULT_ONCHAIN_MAX_FEE_PERCENT);
    let fee_sats = transaction.fee_sats.ok_or_else(|| {
        ApiError::InvalidInput(
            "Cannot pay on-chain transaction because fee_sats is unknown. Re-prepare the transaction or pass dangerously_disable_fee_guardrail: true.".to_string(),
        )
    })?;
    if fee_sats < 0 {
        return Err(ApiError::InvalidInput(
            "Cannot pay on-chain transaction because fee_sats is invalid".to_string(),
        ));
    }
    if transaction.amount_sats <= 0 {
        return Err(ApiError::InvalidInput(
            "Cannot pay on-chain transaction because amount_sats is invalid".to_string(),
        ));
    }

    let max_fee_by_percent =
        ((transaction.amount_sats as f64) * max_fee_percent / 100.0).floor() as i64;
    let max_allowed_fee = std::cmp::min(max_fee_sats, max_fee_by_percent);
    if fee_sats > max_allowed_fee {
        return Err(ApiError::InvalidInput(format!(
            "Cannot pay on-chain transaction because fee_sats {} exceeds guardrail {} sats",
            fee_sats, max_allowed_fee
        )));
    }

    Ok(())
}

fn is_txid(value: &str) -> bool {
    let trimmed = value.trim();
    trimmed.len() == 64 && trimmed.chars().all(|c| c.is_ascii_hexdigit())
}

pub async fn get_info(config: PhoenixdConfig) -> Result<NodeInfo, ApiError> {
    let info_url = format!("{}/getinfo", config.url);
    let client = client(&config);

    let response = client
        .get(&info_url)
        .basic_auth("", Some(config.password.clone()))
        .send()
        .await
        .map_err(|e| ApiError::Http {
            reason: e.to_string(),
        })?;
    let response_text = response.text().await.map_err(|e| ApiError::Http {
        reason: e.to_string(),
    })?;
    // println!("get node info response: {}", response_text);

    // Get balance info as well
    let balance_url = format!("{}/getbalance", config.url);
    let balance_response = client
        .get(&balance_url)
        .basic_auth("", Some(config.password.clone()))
        .send()
        .await
        .map_err(|e| ApiError::Http {
            reason: e.to_string(),
        })?;
    let balance_response_text = balance_response
        .text()
        .await
        .map_err(|e| ApiError::Http {
            reason: e.to_string(),
        })?;
    // println!("balance_response: {}", balance_response_text);

    // Now process the results in  context
    let info: InfoResponse = serde_json::from_str(&response_text)?;
    let balance: GetBalanceResponse = serde_json::from_str(&balance_response_text)?;

    let node_info = NodeInfo {
        alias: "Phoenixd".to_string(),
        pubkey: info.node_id,
        network: "bitcoin".to_string(),
        block_height: 0,
        block_hash: "".to_string(),
        send_balance_msat: info.channels.first().map_or(0, |c| c.balance_sat * 1000),
        receive_balance_msat: info
            .channels
            .first()
            .map_or(0, |c| c.inbound_liquidity_sat * 1000),
        fee_credit_balance_msat: balance.fee_credit_sat * 1000,
        ..Default::default()
    };
    Ok(node_info)
}

pub async fn create_invoice(
    config: PhoenixdConfig,
    invoice_type: InvoiceType,
    amount_msats: Option<i64>,
    description: Option<String>,
    description_hash: Option<String>,
    expiry: Option<i64>,
) -> Result<Transaction, ApiError> {
    let client = client(&config);
    match invoice_type {
        InvoiceType::Bolt11 => {
            let req_url = format!("{}/createinvoice", config.url);

            let bolt11_req = Bolt11Req {
                description: description.clone(),
                amount_sat: amount_msats.unwrap_or_default() / 1000,
                expiry_seconds: expiry.unwrap_or(3600),
                external_id: None, // TODO
                webhook_url: None, // TODO
            };

            let response = client
                .post(&req_url)
                .basic_auth("", Some(config.password.clone()))
                .form(&bolt11_req)
                .send()
                .await
                .map_err(|e| ApiError::Http {
                    reason: e.to_string(),
                })?;

            // println!("Status: {}", response.status());

            let invoice_str = response.text().await.map_err(|e| ApiError::Http {
                reason: e.to_string(),
            })?;
            let invoice_str = invoice_str.as_str();
            // println!("Bolt11 {}", &invoice_str.to_string());

            let bolt11_resp: Bolt11Resp =
                serde_json::from_str(&invoice_str).map_err(|e| crate::ApiError::Json {
                    reason: e.to_string(),
                })?;

            Ok(Transaction {
                type_: "incoming".to_string(),
                invoice: bolt11_resp.serialized,
                preimage: "".to_string(),
                payment_hash: bolt11_resp.payment_hash,
                amount_msats: amount_msats.unwrap_or(0),
                fees_paid: 0,
                created_at: 0,
                expires_at: expiry.unwrap_or(3600),
                settled_at: 0,
                description: description.unwrap_or_default(),
                description_hash: description_hash.unwrap_or_default(),
                payer_note: Some("".to_string()),
                external_id: Some("".to_string()),
            })
        }
        InvoiceType::Bolt12 => {
            let req_url = format!("{}/createoffer", config.url);

            let bolt12_req = Bolt12Req {
                description: description.clone(),
                amount_sat: amount_msats.map(|a| a / 1000),
            };

            let response = client
                .post(&req_url)
                .basic_auth("", Some(config.password.clone()))
                .form(&bolt12_req)
                .send()
                .await
                .map_err(|e| ApiError::Http {
                    reason: e.to_string(),
                })?;

            // println!("Status: {}", response.status());

            let invoice_str = response.text().await.map_err(|e| ApiError::Http {
                reason: e.to_string(),
            })?;
            let invoice_str = invoice_str.as_str();
            // println!("Bolt12 {}", &invoice_str.to_string());

            Ok(Transaction {
                type_: "incoming".to_string(),
                invoice: invoice_str.to_string(),
                preimage: "".to_string(),
                payment_hash: "".to_string(),
                amount_msats: amount_msats.unwrap_or(0),
                fees_paid: 0,
                created_at: 0,
                expires_at: expiry.unwrap_or(3600),
                settled_at: 0,
                description: description.unwrap_or_default(),
                description_hash: description_hash.unwrap_or_default(),
                payer_note: Some("".to_string()),
                external_id: Some("".to_string()),
            })
        }
    }
}

pub async fn pay_invoice(
    config: PhoenixdConfig,
    invoice_params: PayInvoiceParams,
) -> Result<PayInvoiceResponse, ApiError> {
    let client = client(&config);
    let req_url = format!("{}/payinvoice", config.url);
    let mut params = vec![];
    if invoice_params.amount_msats.is_some() {
        params.push((
            "amountSat",
            Some((invoice_params.amount_msats.unwrap_or_default() / 1000).to_string()),
        ));
    }
    params.push(("invoice", Some(invoice_params.invoice.to_string())));
    let response = client
        .post(&req_url)
        .basic_auth("", Some(config.password.clone()))
        .form(&params)
        .send()
        .await
        .map_err(|e| ApiError::Http {
            reason: e.to_string(),
        })?;
    // println!("Status: {}", response.status());
    let response_text = response.text().await.map_err(|e| ApiError::Http {
        reason: e.to_string(),
    })?;
    let pay_invoice_resp: PhoenixPayInvoiceResp =
        serde_json::from_str(&response_text).map_err(|e| ApiError::Json {
            reason: format!("Failed to parse pay_invoice response: {}", e),
        })?;

    Ok(PayInvoiceResponse {
        payment_hash: pay_invoice_resp.payment_hash,
        preimage: pay_invoice_resp.preimage,
        fee_msats: pay_invoice_resp.routing_fee_sat * 1000,
    })
}

pub async fn prepare_onchain_transaction(
    _config: PhoenixdConfig,
    params: PrepareOnchainTransactionParams,
) -> Result<OnchainTransaction, ApiError> {
    assert_valid_onchain_amount(params.amount_sats)?;

    let fee = params.fee.clone().unwrap_or_default();
    let fee_payer = resolve_phoenixd_fee_payer(params.fee_payer.clone())?;
    let fee_request = resolve_phoenixd_fee_request(&fee)?;

    Ok(OnchainTransaction {
        id: None,
        address: params.address.clone(),
        amount_sats: params.amount_sats,
        fee_sats: None,
        total_amount_sats: None,
        recipient_amount_sats: Some(params.amount_sats),
        fee_payer,
        fee,
        expires_at: None,
        estimated_delivery_seconds: None,
        raw: Some(
            serde_json::json!({
                "sendRequest": {
                    "address": params.address,
                    "amountSat": params.amount_sats,
                    "feerateSatByte": fee_request.feerate_sat_byte,
                },
                "description": params.description,
            })
            .to_string(),
        ),
    })
}

pub async fn pay_onchain(
    config: PhoenixdConfig,
    transaction: OnchainTransaction,
) -> Result<PayOnchainResponse, ApiError> {
    pay_onchain_with_options(config, transaction, PayOnchainOptions::default()).await
}

pub async fn pay_onchain_with_options(
    config: PhoenixdConfig,
    transaction: OnchainTransaction,
    options: PayOnchainOptions,
) -> Result<PayOnchainResponse, ApiError> {
    assert_valid_onchain_amount(transaction.amount_sats)?;
    let _fee_payer = resolve_phoenixd_fee_payer(Some(transaction.fee_payer.clone()))?;
    let fee_request = resolve_phoenixd_fee_request(&transaction.fee)?;
    assert_onchain_fee_guardrail(&transaction, options)?;

    let client = client(&config);
    let req_url = format!("{}/sendtoaddress", config.url);
    let response = client
        .post(&req_url)
        .basic_auth("", Some(config.password.clone()))
        .form(&[
            ("address", transaction.address.clone()),
            ("amountSat", transaction.amount_sats.to_string()),
            ("feerateSatByte", fee_request.feerate_sat_byte.to_string()),
        ])
        .send()
        .await
        .map_err(|e| ApiError::Http {
            reason: format!("Failed to broadcast Phoenixd on-chain transaction: {}", e),
        })?;
    if !response.status().is_success() {
        let status = response.status();
        let error_text = response.text().await.unwrap_or_default();
        return Err(ApiError::Http {
            reason: format!(
                "Failed to broadcast Phoenixd on-chain transaction: {} - {}",
                status, error_text
            ),
        });
    }

    let response_text = response.text().await.unwrap_or_default();
    let txid = response_text.trim().to_string();
    if !is_txid(&txid) {
        return Err(ApiError::Api {
            reason: if txid.is_empty() {
                "Phoenixd sendtoaddress did not return a txid".to_string()
            } else {
                txid
            },
        });
    }

    Ok(PayOnchainResponse {
        payment_id: None,
        txid: Some(txid),
        state: "pending".to_string(),
        address: transaction.address,
        amount_sats: transaction.amount_sats,
        fee_sats: transaction.fee_sats,
        total_amount_sats: transaction.total_amount_sats,
        recipient_amount_sats: transaction.recipient_amount_sats.or(Some(transaction.amount_sats)),
        created_at: None,
        raw: Some(response_text),
    })
}

pub fn decode(str: String) -> Result<String, ApiError> {
    crate::utils::decode_bolt11(str)
}

pub fn decode_offer(offer: String) -> Result<String, ApiError> {
    crate::utils::decode_offer(offer)
}

// Create a new BOLT12 offer
// https://phoenix.acinq.co/server/api#create-bolt12-offer
pub async fn create_offer(
    config: PhoenixdConfig,
    params: CreateOfferParams,
) -> Result<Offer, ApiError> {
    let req_url = format!("{}/createoffer", config.url);
    let client = client(&config);

    // Always use form data with optional fields
    let bolt12_req = Bolt12Req {
        description: params.description.clone(),
        amount_sat: params.amount_msats.map(|a| a / 1000),
    };

    let response = client
        .post(&req_url)
        .basic_auth("", Some(config.password.clone()))
        .form(&bolt12_req)
        .send()
        .await
        .map_err(|e| ApiError::Http {
            reason: e.to_string(),
        })?;

    let offer_str = response.text().await.map_err(|e| ApiError::Http {
        reason: e.to_string(),
    })?;

    Ok(Offer {
        offer_id: "".to_string(),
        bolt12: offer_str.trim().to_string(),
        label: params.description.clone(),
        active: Some(true),
        single_use: Some(false),
        used: Some(false),
        amount_msats: params.amount_msats,
    })
}

// Get latest BOLT12 offer
pub async fn get_offer(config: PhoenixdConfig) -> Result<Offer, ApiError> {
    let req_url = format!("{}/getoffer", config.url);
    let client = client(&config);
    let response = client
        .get(&req_url)
        .basic_auth("", Some(config.password.clone()))
        .send()
        .await
        .map_err(|e| ApiError::Http {
            reason: e.to_string(),
        })?;
    let offer_str = response.text().await.map_err(|e| ApiError::Http {
        reason: e.to_string(),
    })?;
    Ok(Offer {
        offer_id: "".to_string(),
        bolt12: offer_str.to_string(),
        label: None,
        active: None,
        single_use: None,
        used: None,
        amount_msats: None,
    })
}

pub async fn pay_offer(
    config: PhoenixdConfig,
    offer: String,
    amount_msats: i64,
    payer_note: Option<String>,
) -> Result<PayInvoiceResponse, ApiError> {
    let req_url = format!("{}/payoffer", config.url);
    let client = client(&config);
    let response = client
        .post(&req_url)
        .basic_auth("", Some(config.password.clone()))
        .form(&[
            ("amountSat", (amount_msats / 1000).to_string()),
            ("offer", offer),
            ("message", payer_note.unwrap_or_default()),
        ])
        .send()
        .await
        .map_err(|e| ApiError::Http {
            reason: e.to_string(),
        })?;
    let response_text = response.text().await.map_err(|e| ApiError::Http {
        reason: e.to_string(),
    })?;
    let response_text = response_text.as_str();
    let pay_resp: PayResponse = match serde_json::from_str(&response_text) {
        Ok(resp) => resp,
        Err(_e) => {
            return Err(ApiError::Json {
                reason: response_text.to_string(),
            })
        }
    };
    Ok(PayInvoiceResponse {
        payment_hash: pay_resp.payment_hash,
        preimage: pay_resp.preimage,
        fee_msats: pay_resp.routing_fee_sat * 1000,
    })
}

// TODO implement list_offers, currently just one is returned by Phoenixd
pub async fn list_offers(
    _config: PhoenixdConfig,
    _search: Option<String>,
) -> Result<Vec<Offer>, ApiError> {
    Ok(vec![])
}

pub async fn lookup_invoice(
    config: PhoenixdConfig,
    payment_hash: Option<String>,
    _from: Option<i64>,
    _limit: Option<i64>,
    _search: Option<String>,
) -> Result<Transaction, ApiError> {
    let url = format!("{}/payments/incoming/{}", config.url, payment_hash.unwrap());
    let client = client(&config);
    let response = client
        .get(&url)
        .basic_auth("", Some(config.password.clone()))
        .send()
        .await
        .map_err(|e| ApiError::Http {
            reason: e.to_string(),
        })?;
    let response_text = response.text().await.map_err(|e| ApiError::Http {
        reason: e.to_string(),
    })?;
    let response_text = response_text.as_str();
    dbg!(response_text);
    let inv: InvoiceResponse = serde_json::from_str(&response_text)?;

    let settled_at = if inv.completed_at.is_some() && inv.is_paid {
            (inv.completed_at.unwrap_or(0) / 1000) as i64
        } else {
            0
        };

    // Determine the amount: use received_sat if paid, otherwise decode from invoice
    let amount_msats = if inv.received_sat > 0 {
        inv.received_sat * 1000
    } else if let Some(invoice_str) = &inv.invoice {
        // Try to decode the invoice to get the amount
        match Bolt11Invoice::from_str(invoice_str) {
            Ok(decoded_invoice) => {
                decoded_invoice.amount_milli_satoshis().unwrap_or(0) as i64
            }
            Err(_) => 0
        }
    } else {
        0
    };

    let txn = Transaction {
        type_: "incoming".to_string(),
        invoice: inv.invoice.unwrap_or_default(),
        preimage: inv.preimage,
        payment_hash: inv.payment_hash,
        amount_msats,
        fees_paid: inv.fees * 1000,
        created_at: inv.created_at,
        expires_at: 0, // TODO
        settled_at,
        description: inv.description.unwrap_or_default(),
        description_hash: "".to_string(), // TODO
        payer_note: Some(inv.payer_note.unwrap_or("".to_string())),
        external_id: Some(inv.external_id.unwrap_or("".to_string())),
    };
    Ok(txn)
}

pub async fn list_transactions(
    config: PhoenixdConfig,
    params: ListTransactionsParams,
) -> Result<Vec<Transaction>, ApiError> {
    let client = client(&config);

    // 1) Build query for incoming transactions
    let mut incoming_params = vec![];
    if params.from != 0 {
        incoming_params.push(("from", (params.from * 1000).to_string()));
    }
    if params.limit != 0 {
        incoming_params.push(("limit", params.limit.to_string()));
    }
    incoming_params.push(("all", "false".to_string())); // do not return payments that have failed

    // Build the final incoming URL with query
    let incoming_query = serde_urlencoded::to_string(&incoming_params).unwrap();
    let incoming_url = format!("{}/payments/incoming?{}", config.url, incoming_query);

    // Fetch incoming transactions
    let incoming_resp = client
        .get(&incoming_url)
        .basic_auth("", Some(config.password.clone()))
        .send()
        .await
        .map_err(|e| ApiError::Http {
            reason: e.to_string(),
        })?;
    let incoming_text = incoming_resp
        .text()
        .await
        .map_err(|e| ApiError::Http {
            reason: e.to_string(),
        })?;
    let incoming_text = incoming_text.as_str();
    let incoming_payments: Vec<InvoiceResponse> = serde_json::from_str(&incoming_text).unwrap();

    let mut transactions: Vec<Transaction> = vec![];

    for inc_payment in incoming_payments {
        let settled_at = if inc_payment.completed_at.unwrap_or(0) != 0 && inc_payment.is_paid {
            Some((inc_payment.completed_at.unwrap_or(0) / 1000) as i64)
        } else {
            None
        };
        if let Some(ref search) = params.search {
            let hash_match = inc_payment.payment_hash == *search;
            let note_match = inc_payment
                .payer_note
                .as_ref()
                .map_or(false, |note| note == search);
            // Only include if search matches payment_hash or payer_note
            if !(hash_match || note_match) {
                continue;
            }
        }
        if params.payment_hash.is_some()
            && inc_payment.payment_hash != params.payment_hash.clone().unwrap()
        {
            continue;
        }
        transactions.push(Transaction {
            type_: "incoming".to_string(),
            invoice: "".to_string(), // TODO
            preimage: inc_payment.preimage,
            payment_hash: inc_payment.payment_hash,
            amount_msats: inc_payment.received_sat * 1000,
            fees_paid: inc_payment.fees * 1000,
            created_at: (inc_payment.created_at / 1000) as i64,
            expires_at: 0, // TODO
            settled_at: settled_at.unwrap_or(0),
            description: "".to_string(),
            description_hash: "".to_string(),
            payer_note: Some(inc_payment.payer_note.unwrap_or("".to_string())),
            external_id: Some(inc_payment.external_id.unwrap_or("".to_string())),
        });
    }

    // 2) Build query for outgoing transactions
    let mut outgoing_params = vec![];
    if params.from != 0 {
        outgoing_params.push(("from", (params.from * 1000).to_string()));
    }
    if params.limit != 0 {
        outgoing_params.push(("limit", params.limit.to_string()));
    }
    outgoing_params.push(("all", "false".to_string())); // do not return payments that have failed

    // Build the final outgoing URL with query
    let outgoing_query = serde_urlencoded::to_string(&outgoing_params).unwrap();
    let outgoing_url = format!("{}/payments/outgoing?{}", config.url, outgoing_query);

    // Fetch outgoing transactions
    let outgoing_resp = client
        .get(&outgoing_url)
        .basic_auth("", Some(config.password.clone()))
        .send()
        .await
        .map_err(|e| ApiError::Http {
            reason: e.to_string(),
        })?;
    let outgoing_text = outgoing_resp
        .text()
        .await
        .map_err(|e| ApiError::Http {
            reason: e.to_string(),
        })?;
    let outgoing_text = outgoing_text.as_str();
    let outgoing_payments: Vec<OutgoingPaymentResponse> =
        serde_json::from_str(&outgoing_text).unwrap();

    // Convert outgoing payments into "outgoing" Transaction
    for payment in outgoing_payments {
        let settled_at = if payment.completed_at != 0 {
            Some((payment.completed_at / 1000) as i64)
        } else {
            None
        };
        if let Some(ref search) = params.search {
            let hash_match = payment.payment_hash == Some(search.clone());
            let note_match = payment
                .payer_note
                .as_ref()
                .map_or(false, |note| note == search);
            // Only include if search matches payment_hash or payer_note
            if !(hash_match || note_match) {
                continue;
            }
        }
        if params.payment_hash.is_some()
            && payment.payment_hash.is_some()
            && payment.payment_hash.clone().unwrap() != params.payment_hash.clone().unwrap()
        {
            continue;
        }
        transactions.push(Transaction {
            type_: "outgoing".to_string(),
            invoice: "".to_string(), // TODO
            preimage: payment.preimage.unwrap_or("".to_string()),
            payment_hash: payment.payment_hash.unwrap_or("".to_string()),
            amount_msats: payment.sent * 1000,
            fees_paid: payment.fees * 1000,
            created_at: (payment.created_at / 1000) as i64,
            expires_at: 0, // TODO
            settled_at: settled_at.unwrap_or(0),
            description: "".to_string(),
            description_hash: "".to_string(),
            payer_note: Some(payment.payer_note.unwrap_or("".to_string())),
            external_id: Some(payment.external_id.unwrap_or("".to_string())),
        });
    }

    // Sort by created date descending
    transactions.sort_by(|a, b| b.created_at.cmp(&a.created_at));

    Ok(transactions)
}

// Core logic shared by both implementations
pub async fn poll_invoice_events<F>(
    config: PhoenixdConfig,
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

        let (status, transaction) = match list_transactions(
            config.clone(),
            ListTransactionsParams {
                from: 0,
                limit: 2500, // TODO remove hardcoded limit
                payment_hash: params.payment_hash.clone(),
                search: params.search.clone(),
                created_after: None,
                created_before: None,
            },
        ).await {
            Ok(transactions) => {
                if transactions.is_empty() {
                    ("pending".to_string(), None)
                } else {
                    let transaction = transactions[0].clone();
                    if transaction.settled_at > 0 {
                        ("settled".to_string(), Some(transaction))
                    } else {
                        ("pending".to_string(), Some(transaction))
                    }
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
                // break;
            }
            _ => {
                callback("pending".to_string(), transaction);
            }
        }

        sleep(Duration::from_secs(params.polling_delay_sec as u64)).await;
    }
}

pub async fn on_invoice_events(
    config: PhoenixdConfig,
    params: OnInvoiceEventParams,
    callback: std::sync::Arc<dyn OnInvoiceEventCallback>,
) {
    poll_invoice_events(config, params, move |status, tx| match status.as_str() {
        "success" => callback.success(tx),
        "pending" => callback.pending(tx),
        "failure" => callback.failure(tx),
        _ => {}
    }).await;
}
