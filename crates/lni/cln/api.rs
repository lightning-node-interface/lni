use super::types::{
    Bolt11Resp, Bolt12Resp, ChannelWrapper, FetchInvoiceResponse, InfoResponse, InvoicesResponse,
    ListOffersResponse, PayResponse,
};
use super::ClnConfig;
use crate::cln::types::Invoice;
use crate::types::NodeInfo;
use crate::{
    calculate_fee_msats, ApiError, CreateOfferParams, InvoiceType, Offer, OnInvoiceEventCallback, OnInvoiceEventParams,
    OnchainFeePayer, OnchainFeePreference, OnchainFeePreferenceType, OnchainFeeSpeed,
    OnchainTransaction, PayInvoiceParams, PayInvoiceResponse, PayOnchainOptions,
    PayOnchainResponse, PrepareOnchainTransactionParams, Transaction,
};
use reqwest::header;
use serde::Deserialize;
use serde_json::json;
use std::time::Duration;
use tokio::time::sleep;

// https://docs.corelightning.org/reference/get_list_methods_resource

#[derive(Debug, Deserialize)]
struct TxPrepareResponse {
    psbt: Option<String>,
    unsigned_tx: Option<String>,
    txid: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TxSendResponse {
    txid: Option<String>,
}

#[derive(Clone)]
struct ClnOnchainFeeRequest {
    feerate: Option<String>,
}

fn clnrest_client(config: &ClnConfig) -> reqwest::Client {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert("Rune", header::HeaderValue::from_str(&config.rune).unwrap());

    // Create HTTP client with optional SOCKS5 proxy following LND pattern
    if let Some(proxy_url) = config.socks5_proxy.clone() {
        if !proxy_url.is_empty() {
            let mut client_builder = reqwest::Client::builder().default_headers(headers.clone());
            if config.accept_invalid_certs.unwrap_or(false) {
                client_builder = client_builder.danger_accept_invalid_certs(true);
            }
            if let Some(timeout) = config.http_timeout {
                client_builder =
                    client_builder.timeout(std::time::Duration::from_secs(timeout as u64));
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
    let mut client_builder = reqwest::ClientBuilder::new().default_headers(headers);
    if config.accept_invalid_certs.unwrap_or(false) {
        client_builder = client_builder.danger_accept_invalid_certs(true);
    }
    if let Some(timeout) = config.http_timeout {
        client_builder = client_builder.timeout(std::time::Duration::from_secs(timeout as u64));
    }
    client_builder
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
}

fn assert_valid_onchain_amount(amount_sats: i64) -> Result<(), ApiError> {
    if amount_sats <= 0 {
        return Err(ApiError::InvalidInput(
            "pay_onchain requires a positive amount_sats".to_string(),
        ));
    }

    Ok(())
}

fn default_onchain_fee() -> OnchainFeePreference {
    OnchainFeePreference {
        preference_type: OnchainFeePreferenceType::Speed,
        speed: Some(OnchainFeeSpeed::Normal),
        target_conf: None,
        sats_per_vbyte: None,
        backend: None,
    }
}

fn resolve_cln_fee_payer(fee_payer: Option<OnchainFeePayer>) -> Result<OnchainFeePayer, ApiError> {
    match fee_payer.unwrap_or(OnchainFeePayer::Sender) {
        OnchainFeePayer::Sender => Ok(OnchainFeePayer::Sender),
        OnchainFeePayer::Recipient => Err(ApiError::InvalidInput(
            "CLN pay_onchain only supports sender-paid on-chain fees".to_string(),
        )),
    }
}

fn resolve_cln_fee_request(fee: &OnchainFeePreference) -> Result<ClnOnchainFeeRequest, ApiError> {
    match fee.preference_type {
        OnchainFeePreferenceType::Default => Ok(ClnOnchainFeeRequest {
            feerate: Some("normal".to_string()),
        }),
        OnchainFeePreferenceType::Speed => match fee.speed.clone().unwrap_or(OnchainFeeSpeed::Normal) {
            OnchainFeeSpeed::Fast => Ok(ClnOnchainFeeRequest {
                feerate: Some("urgent".to_string()),
            }),
            OnchainFeeSpeed::Normal => Ok(ClnOnchainFeeRequest {
                feerate: Some("normal".to_string()),
            }),
            OnchainFeeSpeed::Slow => Ok(ClnOnchainFeeRequest {
                feerate: Some("slow".to_string()),
            }),
            OnchainFeeSpeed::Free => Err(ApiError::InvalidInput(
                "CLN pay_onchain does not support free on-chain fee speed".to_string(),
            )),
        },
        OnchainFeePreferenceType::SatsPerVbyte => {
            let sats_per_vbyte = fee.sats_per_vbyte.ok_or_else(|| {
                ApiError::InvalidInput(
                    "CLN sats_per_vbyte fee preference requires a fee rate".to_string(),
                )
            })?;
            if sats_per_vbyte <= 0.0 {
                return Err(ApiError::InvalidInput(
                    "CLN sats_per_vbyte fee preference requires a positive fee rate".to_string(),
                ));
            }

            Ok(ClnOnchainFeeRequest {
                feerate: Some(format!("{}perkb", (sats_per_vbyte * 1000.0).ceil() as i64)),
            })
        }
        OnchainFeePreferenceType::Backend => {
            let feerate = fee.backend.clone().unwrap_or_default();
            if feerate.trim().is_empty() {
                return Err(ApiError::InvalidInput(
                    "CLN backend fee preference requires a feerate value".to_string(),
                ));
            }

            Ok(ClnOnchainFeeRequest {
                feerate: Some(feerate),
            })
        }
        OnchainFeePreferenceType::TargetConf => Err(ApiError::InvalidInput(
            "CLN pay_onchain does not support target-confirmation fee preferences".to_string(),
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

fn read_u64_le(bytes: &[u8], offset: usize) -> Result<u64, ApiError> {
    if offset + 8 > bytes.len() {
        return Err(ApiError::Json {
            reason: "Unexpected end of uint64".to_string(),
        });
    }

    Ok(u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap()))
}

fn read_u32_le(bytes: &[u8], offset: usize) -> Result<u32, ApiError> {
    if offset + 4 > bytes.len() {
        return Err(ApiError::Json {
            reason: "Unexpected end of uint32".to_string(),
        });
    }

    Ok(u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap()))
}

fn read_compact_size(bytes: &[u8], offset: usize) -> Result<(usize, usize), ApiError> {
    let first = *bytes.get(offset).ok_or_else(|| ApiError::Json {
        reason: "Unexpected end of compact size".to_string(),
    })?;
    if first < 0xfd {
        return Ok((first as usize, offset + 1));
    }
    if first == 0xfd {
        if offset + 3 > bytes.len() {
            return Err(ApiError::Json {
                reason: "Unexpected end of compact size".to_string(),
            });
        }
        return Ok((
            u16::from_le_bytes(bytes[offset + 1..offset + 3].try_into().unwrap()) as usize,
            offset + 3,
        ));
    }
    if first == 0xfe {
        return Ok((read_u32_le(bytes, offset + 1)? as usize, offset + 5));
    }

    let value = read_u64_le(bytes, offset + 1)?;
    if value > usize::MAX as u64 {
        return Err(ApiError::Json {
            reason: "Compact size is too large".to_string(),
        });
    }
    Ok((value as usize, offset + 9))
}

#[derive(Default)]
struct ParsedTransaction {
    input_count: usize,
    prevout_vouts: Vec<usize>,
    output_total_sats: i64,
    outputs: Vec<i64>,
}

fn parse_transaction(bytes: &[u8]) -> Result<ParsedTransaction, ApiError> {
    let mut offset = 4;
    let (mut input_count, next) = read_compact_size(bytes, offset)?;
    offset = next;
    if input_count == 0 && offset < bytes.len() {
        offset += 1;
        let input_count_info = read_compact_size(bytes, offset)?;
        input_count = input_count_info.0;
        offset = input_count_info.1;
    }

    let mut prevout_vouts = Vec::new();
    for _ in 0..input_count {
        offset += 32;
        let vout = read_u32_le(bytes, offset)? as usize;
        offset += 4;
        let (script_len, next) = read_compact_size(bytes, offset)?;
        offset = next + script_len + 4;
        if offset > bytes.len() {
            return Err(ApiError::Json {
                reason: "Unexpected end of transaction input".to_string(),
            });
        }
        prevout_vouts.push(vout);
    }

    let (output_count, next) = read_compact_size(bytes, offset)?;
    offset = next;
    let mut outputs = Vec::new();
    let mut output_total_sats = 0;
    for _ in 0..output_count {
        let amount_sats = read_u64_le(bytes, offset)? as i64;
        offset += 8;
        let (script_len, next) = read_compact_size(bytes, offset)?;
        offset = next + script_len;
        if offset > bytes.len() {
            return Err(ApiError::Json {
                reason: "Unexpected end of transaction output".to_string(),
            });
        }
        outputs.push(amount_sats);
        output_total_sats += amount_sats;
    }

    Ok(ParsedTransaction {
        input_count,
        prevout_vouts,
        output_total_sats,
        outputs,
    })
}

fn parse_psbt_map(bytes: &[u8], mut offset: usize) -> Result<(Vec<(Vec<u8>, Vec<u8>)>, usize), ApiError> {
    let mut entries = Vec::new();
    while offset < bytes.len() {
        let (key_len, next) = read_compact_size(bytes, offset)?;
        offset = next;
        if key_len == 0 {
            return Ok((entries, offset));
        }
        if offset + key_len > bytes.len() {
            return Err(ApiError::Json {
                reason: "Unexpected end of PSBT key".to_string(),
            });
        }
        let key = bytes[offset..offset + key_len].to_vec();
        offset += key_len;
        let (value_len, next) = read_compact_size(bytes, offset)?;
        offset = next;
        if offset + value_len > bytes.len() {
            return Err(ApiError::Json {
                reason: "Unexpected end of PSBT value".to_string(),
            });
        }
        let value = bytes[offset..offset + value_len].to_vec();
        offset += value_len;
        entries.push((key, value));
    }

    Err(ApiError::Json {
        reason: "Unterminated PSBT map".to_string(),
    })
}

fn parse_psbt_fee_sats(psbt: Option<&String>) -> Option<i64> {
    let psbt = psbt?;
    let bytes = base64::decode(psbt).ok()?;
    if bytes.len() < 5 || &bytes[0..5] != b"psbt\xff" {
        return None;
    }

    let (global_entries, mut offset) = parse_psbt_map(&bytes, 5).ok()?;
    let unsigned_tx = global_entries
        .iter()
        .find(|(key, _)| key.first() == Some(&0x00))
        .map(|(_, value)| value.clone())?;
    let tx = parse_transaction(&unsigned_tx).ok()?;
    let mut input_total_sats = 0;

    for index in 0..tx.input_count {
        let (input_entries, next) = parse_psbt_map(&bytes, offset).ok()?;
        offset = next;

        if let Some((_, witness_utxo)) = input_entries
            .iter()
            .find(|(key, _)| key.first() == Some(&0x01))
        {
            input_total_sats += read_u64_le(witness_utxo, 0).ok()? as i64;
            continue;
        }

        if let Some((_, non_witness_utxo)) = input_entries
            .iter()
            .find(|(key, _)| key.first() == Some(&0x00))
        {
            let prev_tx = parse_transaction(non_witness_utxo).ok()?;
            let vout = *tx.prevout_vouts.get(index)?;
            if let Some(amount_sats) = prev_tx.outputs.get(vout) {
                input_total_sats += amount_sats;
            }
        }
    }

    let fee_sats = input_total_sats - tx.output_total_sats;
    if input_total_sats > 0 && fee_sats >= 0 {
        Some(fee_sats)
    } else {
        None
    }
}

pub async fn get_info(config: ClnConfig) -> Result<NodeInfo, ApiError> {
    let req_url = format!("{}/v1/getinfo", config.url);
    let client = clnrest_client(&config);
    let response = client
        .post(&req_url)
        .header("Content-Type", "application/json")
        .send()
        .await
        .map_err(|e| ApiError::Http {
            reason: format!("Failed to get node info: {}", e),
        })?;
    let response_text = response.text().await.map_err(|e| ApiError::Http {
        reason: format!("Failed to read node info response: {}", e),
    })?;
    let info: InfoResponse = serde_json::from_str(&response_text)?;

    // https://github.com/ZeusLN/zeus/blob/master/backends/CoreLightningRequestHandler.ts#L28
    let funds_url = format!("{}/v1/listfunds", config.url);
    let funds_response = client
        .post(&funds_url)
        .header("Content-Type", "application/json")
        .send()
        .await
        .map_err(|e| ApiError::Http {
            reason: format!("Failed to get funds info: {}", e),
        })?;
    let funds_response_text = funds_response.text().await.map_err(|e| ApiError::Http {
        reason: format!("Failed to read funds response: {}", e),
    })?;
    let channels: ChannelWrapper = serde_json::from_str(&funds_response_text)?;

    let mut local_balance: i64 = 0;
    let mut remote_balance: i64 = 0;
    let mut unsettled_send_balance_msat: i64 = 0;
    let mut unsettled_receive_balance_msat: i64 = 0;
    let mut pending_open_send_balance: i64 = 0;
    let mut pending_open_receive_balance: i64 = 0;
    // rules and states here https://docs.corelightning.org/reference/listfunds
    for channel in channels.channels.iter() {
        if channel.state == "CHANNELD_NORMAL" && channel.connected {
            // Active channels
            local_balance += channel.our_amount_msat;
            remote_balance += channel.amount_msat - channel.our_amount_msat;
        } else if channel.state == "CHANNELD_NORMAL" && !channel.connected {
            // Unsettled channels (previously inactive)
            unsettled_send_balance_msat += channel.our_amount_msat;
            unsettled_receive_balance_msat += channel.amount_msat - channel.our_amount_msat;
        } else if channel.state == "CHANNELD_AWAITING_LOCKIN"
            || channel.state == "DUALOPEND_AWAITING_LOCKIN"
            || channel.state == "DUALOPEND_OPEN_INIT"
            || channel.state == "DUALOPEND_OPEN_COMMITTED"
            || channel.state == "DUALOPEND_OPEN_COMMIT_READY"
            || channel.state == "OPENINGD"
        {
            // Pending open channels
            pending_open_send_balance += channel.our_amount_msat;
            pending_open_receive_balance += channel.amount_msat - channel.our_amount_msat;
        }
    }

    let node_info = NodeInfo {
        alias: info.alias,
        color: info.color,
        pubkey: info.id,
        network: info.network,
        block_height: info.blockheight,
        block_hash: "".to_string(),
        send_balance_msat: local_balance,
        receive_balance_msat: remote_balance,
        unsettled_send_balance_msat,
        unsettled_receive_balance_msat,
        pending_open_send_balance,
        pending_open_receive_balance,
        ..Default::default()
    };
    Ok(node_info)
}

// invoice - amount_msat label description expiry fallbacks preimage exposeprivatechannels cltv
pub async fn create_invoice(
    config: ClnConfig,
    invoice_type: InvoiceType,
    amount_msats: Option<i64>,
    offer: Option<String>,
    description: Option<String>, // public memo for bolt11, private? payer_note for bolt12
    description_hash: Option<String>,
    expiry: Option<i64>,
) -> Result<Transaction, ApiError> {
    let client = clnrest_client(&config);
    let amount_msat_str: String = amount_msats.map_or("any".to_string(), |amt| amt.to_string());
    let mut params: Vec<(&str, Option<String>)> = vec![];
    params.push((
        "description",
        Some(description.clone().unwrap_or("".to_string())),
    ));
    params.push(("amount_msat", Some(amount_msat_str.clone())));
    params.push(("expiry", expiry.map(|e| e.to_string())));
    params.push((
        "label",
        Some(format!("lni.{}", rand::random::<u32>()).into()),
    ));
    match invoice_type {
        InvoiceType::Bolt11 => {
            let req_url = format!("{}/v1/invoice", config.url);
            let response = client
                .post(&req_url)
                .header("Content-Type", "application/json")
                .json(&serde_json::json!(params
                    .into_iter()
                    .filter_map(|(k, v)| v.map(|v| (k, v.to_string())))
                    .collect::<serde_json::Value>()))
                .send()
                .await
                .map_err(|e| ApiError::Http {
                    reason: format!("Failed to create invoice: {}", e),
                })?;

            let invoice_str = response.text().await.map_err(|e| ApiError::Http {
                reason: format!("Failed to read invoice response: {}", e),
            })?;
            let invoice_str = invoice_str.as_str();
            let bolt11_resp: Bolt11Resp =
                serde_json::from_str(&invoice_str).map_err(|e| crate::ApiError::Json {
                    reason: e.to_string(),
                })?;

            Ok(Transaction {
                type_: "incoming".to_string(),
                invoice: bolt11_resp.bolt11,
                preimage: "".to_string(),
                payment_hash: bolt11_resp.payment_hash,
                amount_msats: amount_msats.unwrap_or(0),
                fees_paid: 0,
                created_at: 0,
                expires_at: expiry.unwrap_or(3600),
                settled_at: 0,
                description: description.clone().unwrap_or_default(),
                description_hash: description_hash.unwrap_or_default(),
                payer_note: Some("".to_string()),
                external_id: Some("".to_string()),
            })
        }
        InvoiceType::Bolt12 => {
            if offer.is_none() {
                return Err(ApiError::Json {
                    reason: "Offer cannot be empty".to_string(),
                });
            }
            let fetch_invoice_resp = fetch_invoice_from_offer(
                &config,
                offer.clone().unwrap(),
                amount_msats.unwrap_or(0), // TODO make this optional if the lno already has amount in it
                Some(description.clone().unwrap_or_default()),
            )
            .await?;
            Ok(Transaction {
                type_: "incoming".to_string(),
                invoice: fetch_invoice_resp.invoice,
                preimage: "".to_string(),
                payment_hash: "".to_string(),
                amount_msats: amount_msats.unwrap_or(0),
                fees_paid: 0,
                created_at: 0,
                expires_at: expiry.unwrap_or_default(),
                settled_at: 0,
                description: description.clone().unwrap_or_default(),
                description_hash: description_hash.unwrap_or_default(),
                payer_note: Some("".to_string()),
                external_id: Some("".to_string()),
            })
        }
    }
}

pub async fn pay_invoice(
    config: ClnConfig,
    invoice_params: PayInvoiceParams,
) -> Result<PayInvoiceResponse, ApiError> {
    let client = clnrest_client(&config);
    let pay_url = format!("{}/v1/pay", config.url);

    let mut params: Vec<(&str, Option<serde_json::Value>)> = vec![];
    params.push((
        "bolt11",
        Some(serde_json::Value::String(
            invoice_params.invoice.to_string(),
        )),
    ));
    invoice_params.amount_msats.map(|amt| {
        params.push((
            "amount_msat",
            Some(serde_json::Value::String(amt.to_string())),
        ))
    });

    // calculate fee limit
    if invoice_params.fee_limit_msat.is_some() && invoice_params.fee_limit_percentage.is_some() {
        return Err(ApiError::Json {
            reason: "Cannot set both fee_limit_msat and fee_limit_percentage".to_string(),
        });
    }
    invoice_params.fee_limit_msat.map(|amt| {
        params.push(("maxfee", Some(serde_json::Value::String(amt.to_string()))));
    });
    invoice_params.fee_limit_percentage.map(|fee_percentage| {
        let fee_msats = calculate_fee_msats(
            invoice_params.invoice.as_str(),
            fee_percentage,
            invoice_params.amount_msats.map(|v| v as u64),
        )
        .unwrap();
        params.push((
            "maxfee",
            Some(serde_json::Value::String(fee_msats.to_string())),
        ));
    });
    invoice_params.timeout_seconds.map(|timeout| {
        params.push((
            "retry_for",
            Some(serde_json::Value::String(timeout.to_string())),
        ))
    });

    let params_json: serde_json::Value = params
        .into_iter()
        .filter_map(|(k, v)| v.map(|v| (k.to_string(), v)))
        .collect::<serde_json::Map<String, _>>()
        .into();

    let pay_response = client
        .post(&pay_url)
        .header("Content-Type", "application/json")
        .json(&params_json)
        .send()
        .await
        .map_err(|e| ApiError::Http {
            reason: format!("Failed to pay invoice: {}", e),
        })?;
    let pay_response_text = pay_response.text().await.map_err(|e| ApiError::Http {
        reason: format!("Failed to read pay response: {}", e),
    })?;
    let pay_response_text = pay_response_text.as_str();
    let pay_resp: PayResponse = match serde_json::from_str(&pay_response_text) {
        Ok(resp) => resp,
        Err(_e) => {
            return Err(ApiError::Json {
                reason: pay_response_text.to_string(),
            })
        }
    };

    Ok(PayInvoiceResponse {
        payment_hash: pay_resp.payment_hash,
        preimage: pay_resp.payment_preimage,
        fee_msats: pay_resp.amount_sent_msat - pay_resp.amount_msat,
    })
}

pub async fn prepare_onchain_transaction(
    config: ClnConfig,
    params: PrepareOnchainTransactionParams,
) -> Result<OnchainTransaction, ApiError> {
    assert_valid_onchain_amount(params.amount_sats)?;

    let fee = params.fee.clone().unwrap_or_else(default_onchain_fee);
    let fee_payer = resolve_cln_fee_payer(params.fee_payer.clone())?;
    let fee_request = resolve_cln_fee_request(&fee)?;
    let client = clnrest_client(&config);
    let txprepare_url = format!("{}/v1/txprepare", config.url);
    let mut output = serde_json::Map::new();
    output.insert(
        params.address.clone(),
        json!(format!("{}sat", params.amount_sats)),
    );
    let mut body = json!({
        "outputs": [output],
    });
    if let Some(feerate) = fee_request.feerate.clone() {
        body["feerate"] = json!(feerate);
    }

    let response = client
        .post(&txprepare_url)
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| ApiError::Http {
            reason: format!("Failed to prepare CLN on-chain transaction: {}", e),
        })?;
    if !response.status().is_success() {
        let status = response.status();
        let error_text = response.text().await.unwrap_or_default();
        return Err(ApiError::Http {
            reason: format!(
                "Failed to prepare CLN on-chain transaction: {} - {}",
                status, error_text
            ),
        });
    }

    let response_text = response.text().await.unwrap_or_default();
    let txprepare: TxPrepareResponse = serde_json::from_str(&response_text)?;
    let fee_sats = parse_psbt_fee_sats(txprepare.psbt.as_ref());

    Ok(OnchainTransaction {
        id: txprepare.txid.clone(),
        address: params.address,
        amount_sats: params.amount_sats,
        fee_sats,
        total_amount_sats: fee_sats.map(|fee_sats| params.amount_sats + fee_sats),
        recipient_amount_sats: Some(params.amount_sats),
        fee_payer,
        fee,
        expires_at: None,
        estimated_delivery_seconds: None,
        raw: Some(
            json!({
                "txPrepare": {
                    "psbt": txprepare.psbt,
                    "unsigned_tx": txprepare.unsigned_tx,
                    "txid": txprepare.txid,
                },
                "txSendRequest": {
                    "txid": txprepare.txid,
                },
                "feeRequest": {
                    "feerate": fee_request.feerate,
                },
                "description": params.description,
            })
            .to_string(),
        ),
    })
}

pub async fn pay_onchain(
    config: ClnConfig,
    transaction: OnchainTransaction,
) -> Result<PayOnchainResponse, ApiError> {
    pay_onchain_with_options(config, transaction, PayOnchainOptions::default()).await
}

pub async fn pay_onchain_with_options(
    config: ClnConfig,
    transaction: OnchainTransaction,
    options: PayOnchainOptions,
) -> Result<PayOnchainResponse, ApiError> {
    assert_valid_onchain_amount(transaction.amount_sats)?;
    let _fee_payer = resolve_cln_fee_payer(Some(transaction.fee_payer.clone()))?;
    let _fee_request = resolve_cln_fee_request(&transaction.fee)?;
    assert_onchain_fee_guardrail(&transaction, options)?;
    let txid = transaction.id.clone().ok_or_else(|| {
        ApiError::InvalidInput(
            "CLN pay_onchain requires a transaction id from prepare_onchain_transaction"
                .to_string(),
        )
    })?;

    let client = clnrest_client(&config);
    let txsend_url = format!("{}/v1/txsend", config.url);
    let response = client
        .post(&txsend_url)
        .header("Content-Type", "application/json")
        .json(&json!({ "txid": txid }))
        .send()
        .await
        .map_err(|e| ApiError::Http {
            reason: format!("Failed to broadcast CLN on-chain transaction: {}", e),
        })?;
    if !response.status().is_success() {
        let status = response.status();
        let error_text = response.text().await.unwrap_or_default();
        return Err(ApiError::Http {
            reason: format!(
                "Failed to broadcast CLN on-chain transaction: {} - {}",
                status, error_text
            ),
        });
    }

    let response_text = response.text().await.unwrap_or_default();
    let txsend: TxSendResponse = serde_json::from_str(&response_text)?;
    let response_txid = txsend.txid.clone().or(transaction.id.clone());

    Ok(PayOnchainResponse {
        payment_id: transaction.id,
        txid: response_txid.clone(),
        state: if response_txid.is_some() {
            "pending".to_string()
        } else {
            "failed".to_string()
        },
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

// get the one with the offer_id or label or get the first offer in the list
pub async fn get_offer(config: ClnConfig, search: Option<String>) -> Result<Offer, ApiError> {
    let offers = list_offers(config, search.clone()).await?;
    if offers.is_empty() {
        return Ok(Offer {
            offer_id: "".to_string(),
            bolt12: "".to_string(),
            label: None,
            active: None,
            single_use: None,
            used: None,
            amount_msats: None,
        });
    }
    Ok(offers.first().unwrap().clone())
}

pub async fn list_offers(
    config: ClnConfig,
    search: Option<String>,
) -> Result<Vec<Offer>, ApiError> {
    let client = clnrest_client(&config);
    let req_url = format!("{}/v1/listoffers", config.url);
    let mut params = vec![];
    if let Some(search) = search {
        params.push(("offer_id", Some(search)))
    }
    let response = client
        .post(&req_url)
        .header("Content-Type", "application/json")
        .json(&serde_json::json!(params
            .into_iter()
            .filter_map(|(k, v)| v.map(|v| (k, v)))
            .collect::<serde_json::Value>()))
        .send()
        .await
        .map_err(|e| ApiError::Http {
            reason: format!("Failed to list offers: {}", e),
        })?;
    let offers = response.text().await.map_err(|e| ApiError::Http {
        reason: format!("Failed to read offers response: {}", e),
    })?;
    let offers_str = offers.as_str();
    let offers_list: ListOffersResponse =
        serde_json::from_str(&offers_str).map_err(|e| crate::ApiError::Json {
            reason: e.to_string(),
        })?;
    Ok(offers_list.offers)
}

// Create a BOLT12 offer and return Offer
// https://docs.corelightning.org/reference/offer
pub async fn create_offer(
    config: ClnConfig,
    params: CreateOfferParams,
) -> Result<Offer, ApiError> {
    let client = clnrest_client(&config);
    let req_url = format!("{}/v1/offer", config.url);
    
    let mut json_params = serde_json::Map::new();
    
    // Handle amount - if not specified, create a reusable offer with "any" amount
    if let Some(amount_msats) = params.amount_msats {
        json_params.insert("amount".to_string(), serde_json::json!(format!("{}msat", amount_msats)));
    } else {
        json_params.insert("amount".to_string(), serde_json::json!("any"));
    }
    
    // Add description if provided
    if let Some(description) = params.description.clone() {
        json_params.insert("description".to_string(), serde_json::json!(description));
    }
    
    let response = client
        .post(&req_url)
        .header("Content-Type", "application/json")
        .json(&json_params)
        .send()
        .await
        .map_err(|e| ApiError::Http {
            reason: format!("Failed to create offer: {}", e),
        })?;
        
    let offer_str = response.text().await.map_err(|e| ApiError::Http {
        reason: format!("Failed to read offer response: {}", e),
    })?;
    
    let bolt12resp: Bolt12Resp =
        serde_json::from_str(&offer_str).map_err(|e| crate::ApiError::Json {
            reason: e.to_string(),
        })?;
    
    Ok(Offer {
        offer_id: bolt12resp.offer_id.unwrap_or_default(),
        bolt12: bolt12resp.bolt12,
        label: params.description.clone(),
        active: Some(bolt12resp.active),
        single_use: Some(bolt12resp.single_use),
        used: Some(bolt12resp.used),
        amount_msats: params.amount_msats,
    })
}

async fn fetch_invoice_from_offer(
    config: &ClnConfig,
    offer: String,
    amount_msats: i64, // TODO make optional if the lno already has amount in it
    payer_note: Option<String>,
) -> Result<FetchInvoiceResponse, ApiError> {
    let fetch_invoice_url = format!("{}/v1/fetchinvoice", config.url);
    let client = clnrest_client(&config);
    let response = client
        .post(&fetch_invoice_url)
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({
            "offer": offer,
            "amount_msat": amount_msats,
            "payer_note": payer_note,
            "timeout": 60,
        }))
        .send()
        .await
        .map_err(|e| ApiError::Http {
            reason: format!("Failed to fetch invoice: {}", e),
        })?;
    let response_text = response.text().await.map_err(|e| ApiError::Http {
        reason: format!("Failed to read fetch invoice response: {}", e),
    })?;
    let response_text = response_text.as_str();
    let fetch_invoice_resp: FetchInvoiceResponse = match serde_json::from_str(&response_text) {
        Ok(resp) => resp,
        Err(_e) => {
            return Err(ApiError::Json {
                reason: response_text.to_string(),
            })
        }
    };
    Ok(fetch_invoice_resp)
}

pub async fn pay_offer(
    config: ClnConfig,
    offer: String,
    amount_msats: i64,
    payer_note: Option<String>,
) -> Result<PayInvoiceResponse, ApiError> {
    let client = clnrest_client(&config);
    let fetch_invoice_resp =
        fetch_invoice_from_offer(&config, offer.clone(), amount_msats, payer_note.clone()).await?;
    if fetch_invoice_resp.invoice.is_empty() {
        return Err(ApiError::Json {
            reason: "Missing BOLT 12 invoice".to_string(),
        });
    }

    // now pay the bolt 12 invoice lni
    let pay_url = format!("{}/v1/pay", config.url);
    let pay_response = client
        .post(&pay_url)
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({
            "bolt11": fetch_invoice_resp.invoice.to_string(),
            "maxfeepercent": 1, // TODO read from config
            "retry_for": 60,
        }))
        .send()
        .await
        .map_err(|e| ApiError::Http {
            reason: format!("Failed to pay offer: {}", e),
        })?;
    let pay_response_text = pay_response.text().await.map_err(|e| ApiError::Http {
        reason: format!("Failed to read pay offer response: {}", e),
    })?;
    let pay_response_text = pay_response_text.as_str();
    let pay_resp: PayResponse = match serde_json::from_str(&pay_response_text) {
        Ok(resp) => resp,
        Err(_e) => {
            return Err(ApiError::Json {
                reason: pay_response_text.to_string(),
            })
        }
    };

    Ok(PayInvoiceResponse {
        payment_hash: pay_resp.payment_hash,
        preimage: pay_resp.payment_preimage,
        fee_msats: pay_resp.amount_sent_msat - pay_resp.amount_msat,
    })
}

// Looks up invoice by payment_hash or search field, or returns latest invoice
pub async fn lookup_invoice(
    config: ClnConfig,
    payment_hash: Option<String>,
    from: Option<i64>,
    limit: Option<i64>,
    search: Option<String>,
) -> Result<Transaction, ApiError> {
    match lookup_invoices(&config, payment_hash, from, limit, search).await {
        Ok(transactions) => {
            if let Some(tx) = transactions.first() {
                Ok(tx.clone())
            } else {
                Err(ApiError::Api {
                    reason: "No matching invoice found".to_string(),
                })
            }
        }
        Err(e) => Err(e),
    }
}

async fn lookup_invoices(
    config: &ClnConfig,
    payment_hash: Option<String>,
    from: Option<i64>,
    limit: Option<i64>,
    search: Option<String>,
) -> Result<Vec<Transaction>, ApiError> {
    let client = clnrest_client(config);

    if search.is_some() {
        let list_invoices_url = format!("{}/v1/sql", config.url);
        let sql = format!(
            "SELECT label, bolt11, bolt12, payment_hash, amount_msat, status, amount_received_msat, paid_at, payment_preimage, description, local_offer_id, invreq_payer_note, expires_at FROM invoices"
        );
        let where_clause = if search.is_some() {
            format!(
                "WHERE description = '{}' or invreq_payer_note ='{}' or payment_hash = '{}' ORDER BY created_index DESC LIMIT {}",
                search.clone().unwrap(),
                search.clone().unwrap(),
                search.clone().unwrap(),
                limit.unwrap_or(150),
            )
        } else {
            format!("ORDER BY created_index DESC LIMIT {}", limit.unwrap_or(150),)
        };

        dbg!(format!("{} {}", sql, where_clause));
        let response = client
            .post(&list_invoices_url)
            .header("Content-Type", "application/json")
            .json(&serde_json::json!({
                "query": format!("{} {}", sql, where_clause),
            }))
            .send()
            .await
            .map_err(|e| ApiError::Http {
                reason: format!("Failed to query invoices: {}", e),
            })?;
        let response_text = response.text().await.map_err(|e| ApiError::Http {
            reason: format!("Failed to read invoices response: {}", e),
        })?;
        let response_text = response_text.as_str();
        dbg!(&response_text);

        if response_text.len() > 25 {
            // i.e not blank resp like "[rows: []]"
            // Parse the SQL response into InvoicesResponse
            #[derive(serde::Deserialize)]
            struct SqlResponse {
                rows: Vec<Vec<serde_json::Value>>,
            }
            // Map SQL row indices to InvoicesResponse fields
            let sql_resp: SqlResponse = serde_json::from_str(response_text).unwrap();

            let mut invoices = Vec::new();
            for row in sql_resp.rows {
                invoices.push(Invoice {
                    label: row
                        .get(0)
                        .and_then(|v| v.as_str())
                        .unwrap_or_default()
                        .to_string(),
                    bolt11: row.get(1).and_then(|v| v.as_str()).map(|s| s.to_string()),
                    bolt12: row.get(2).and_then(|v| v.as_str()).map(|s| s.to_string()),
                    payment_hash: row
                        .get(3)
                        .and_then(|v| v.as_str())
                        .unwrap_or_default()
                        .to_string(),
                    amount_msat: Some(row.get(4).and_then(|v| v.as_i64()).unwrap_or(0)),
                    status: row
                        .get(5)
                        .and_then(|v| v.as_str())
                        .unwrap_or_default()
                        .to_string(),
                    amount_received_msat: row.get(6).and_then(|v| v.as_i64()),
                    paid_at: row.get(7).and_then(|v| v.as_i64()),
                    payment_preimage: row.get(8).and_then(|v| v.as_str()).map(|s| s.to_string()),
                    description: row.get(9).and_then(|v| v.as_str()).map(|s| s.to_string()),
                    local_offer_id: row.get(10).and_then(|v| v.as_str()).map(|s| s.to_string()),
                    invreq_payer_note: row.get(11).and_then(|v| v.as_str()).map(|s| s.to_string()),
                    expires_at: row.get(12).and_then(|v| v.as_i64()).unwrap_or(0),
                    pay_index: None,
                    created_index: 0,
                    updated_index: None,
                    paid_outpoint: None,
                });
            }
            let incoming_payments = InvoicesResponse { invoices };
            let mut transactions: Vec<Transaction> = incoming_payments
                .invoices
                .into_iter()
                .map(|inv| Transaction {
                    type_: "incoming".to_string(),
                    invoice: inv
                        .bolt11
                        .clone()
                        .unwrap_or_else(|| inv.bolt12.clone().unwrap_or_default()),
                    preimage: inv.payment_preimage.unwrap_or_default(),
                    payment_hash: inv.payment_hash,
                    amount_msats: inv.amount_received_msat.unwrap_or(0),
                    fees_paid: 0,
                    created_at: 0, // TODO: parse if available
                    expires_at: inv.expires_at,
                    settled_at: inv.paid_at.unwrap_or(0),
                    description: inv.description.unwrap_or_default(),
                    description_hash: "".to_string(),
                    payer_note: Some(inv.invreq_payer_note.unwrap_or_default()),
                    external_id: Some(inv.label),
                })
                .collect();
            transactions.sort_by(|a, b| b.created_at.cmp(&a.created_at));
            return Ok(transactions);
        }
    }

    let list_invoices_url = format!("{}/v1/listinvoices", config.url);
    // 1) Build query for incoming transactions
    let mut params: Vec<(&str, Option<String>)> = vec![];
    if let Some(from_value) = from {
        params.push(("start", Some(from_value.to_string())));
        params.push(("index", Some("created".to_string())));
    }
    if let Some(limit_value) = limit {
        params.push(("limit", Some(limit_value.to_string())));
    }
    let pay_hash = if payment_hash.is_some() {
        payment_hash.clone()
    } else if search.is_some() {
        search.clone()
    } else {
        None
    };
    if let Some(payment_hash_value) = pay_hash {
        params.push(("payment_hash", Some(payment_hash_value)));
    }

    // Fetch incoming transactions
    let response = client
        .post(&list_invoices_url)
        .header("Content-Type", "application/json")
        //.json(&serde_json::json!(params))
        .json(&serde_json::json!(params
            .into_iter()
            .filter_map(|(k, v)| v.map(|v| (k, v)))
            .collect::<serde_json::Value>()))
        .send()
        .await
        .map_err(|e| ApiError::Http {
            reason: format!("Failed to list invoices: {}", e),
        })?;
    let response_text = response.text().await.map_err(|e| ApiError::Http {
        reason: format!("Failed to read list invoices response: {}", e),
    })?;
    let response_text = response_text.as_str();
    let incoming_payments: InvoicesResponse =
        serde_json::from_str(&response_text).map_err(|e| ApiError::Json {
            reason: e.to_string(),
        })?;

    // Convert incoming payments into "incoming" Transaction
    let mut transactions: Vec<Transaction> = incoming_payments
        .invoices
        .into_iter()
        .map(|inv| {
            Transaction {
                type_: "incoming".to_string(),
                invoice: inv.bolt11.unwrap_or_else(|| inv.bolt12.unwrap_or_default()),
                preimage: inv.payment_preimage.unwrap_or("".to_string()),
                payment_hash: inv.payment_hash,
                amount_msats: inv.amount_received_msat.unwrap_or(0),
                fees_paid: 0,
                created_at: 0, // TODO
                expires_at: inv.expires_at,
                settled_at: inv.paid_at.unwrap_or(0),
                description: inv.description.unwrap_or("".to_string()),
                description_hash: "".to_string(),
                payer_note: Some(inv.invreq_payer_note.unwrap_or("".to_string())),
                external_id: Some(inv.label),
            }
        })
        .collect();

    // Sort by created date descending
    transactions.sort_by(|a, b| b.created_at.cmp(&a.created_at));

    Ok(transactions)
}

pub async fn list_transactions(
    config: ClnConfig,
    from: i64,
    limit: i64,
    search: Option<String>,
) -> Result<Vec<Transaction>, ApiError> {
    match lookup_invoices(&config, None, Some(from), Some(limit), search).await {
        Ok(transactions) => Ok(transactions),
        Err(e) => Err(e),
    }
}

// Core logic shared by both implementations
pub async fn poll_invoice_events<F>(
    config: ClnConfig,
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
    config: ClnConfig,
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
