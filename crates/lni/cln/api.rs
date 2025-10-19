use super::types::{
    Bolt11Resp, Bolt12Resp, ChannelWrapper, FetchInvoiceResponse, InfoResponse, InvoicesResponse,
    ListOffersResponse, PayResponse,
};
use super::ClnConfig;
use crate::cln::types::Invoice;
use crate::types::NodeInfo;
use crate::{
    calculate_fee_msats, ApiError, CreateOfferParams, InvoiceType, Offer, OnInvoiceEventCallback, OnInvoiceEventParams,
    PayInvoiceParams, PayInvoiceResponse, Transaction,
};
use reqwest::header;
use std::time::Duration;
use tokio::time::sleep;

// https://docs.corelightning.org/reference/get_list_methods_resource

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

// decode - bolt11 invoice (lnbc) bolt12 invoice (lni) or bolt12 offer (lno)
pub async fn decode(config: ClnConfig, str: String) -> Result<String, ApiError> {
    let client = clnrest_client(&config);
    let req_url = format!("{}/v1/decode", config.url);
    let response = client
        .post(&req_url)
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({
            "string": str,
        }))
        .send()
        .await
        .map_err(|e| ApiError::Http {
            reason: format!("Failed to decode: {}", e),
        })?;
    // TODO parse JSON response
    let decoded = response.text().await.map_err(|e| ApiError::Http {
        reason: format!("Failed to read decode response: {}", e),
    })?;
    Ok(decoded)
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

pub fn on_invoice_events(
    config: ClnConfig,
    params: OnInvoiceEventParams,
    callback: Box<dyn OnInvoiceEventCallback>,
) {
    tokio::task::spawn(async move {
        poll_invoice_events(config, params, move |status, tx| match status.as_str() {
            "success" => callback.success(tx),
            "pending" => callback.pending(tx),
            "failure" => callback.failure(tx),
            _ => {}
        })
        .await;
    });
}
