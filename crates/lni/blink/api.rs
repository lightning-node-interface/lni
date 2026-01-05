use std::str::FromStr;
use std::time::Duration;

use lightning_invoice::Bolt11Invoice;

use super::types::*;
use super::BlinkConfig;
use crate::types::NodeInfo;
use crate::{
    ApiError, CreateInvoiceParams, InvoiceType, Offer, OnInvoiceEventCallback, OnInvoiceEventParams,
    PayInvoiceParams, PayInvoiceResponse, Transaction,
};
use reqwest::header;

// Docs: https://dev.blink.sv/

fn client(config: &BlinkConfig) -> reqwest::Client {
    let mut headers = reqwest::header::HeaderMap::new();
    
    match header::HeaderValue::from_str(&config.api_key) {
        Ok(api_key_header) => headers.insert("X-API-KEY", api_key_header),
        Err(_) => {
            eprintln!("Failed to create API key header");
            return reqwest::ClientBuilder::new()
                .default_headers(headers)
                .build()
                .unwrap_or_else(|_| reqwest::Client::new());
        }
    };
    
    headers.insert(
        "Content-Type",
        header::HeaderValue::from_static("application/json"),
    );

    // Create HTTP client with optional SOCKS5 proxy following Strike pattern
    if let Some(proxy_url) = config.socks5_proxy.clone() {
        if !proxy_url.is_empty() {
            // Accept invalid certificates when using SOCKS5 proxy
            let client_builder = reqwest::Client::builder()
                .default_headers(headers.clone())
                .danger_accept_invalid_certs(true);
            
            match reqwest::Proxy::all(&proxy_url) {
                Ok(proxy) => {
                    let mut builder = client_builder.proxy(proxy);
                    if config.http_timeout.is_some() {
                        builder = builder.timeout(std::time::Duration::from_secs(
                            config.http_timeout.unwrap_or_default() as u64,
                        ));
                    }
                    match builder.build() {
                        Ok(client) => return client,
                        Err(_) => {} // Fall through to default client creation
                    }
                }
                Err(_) => {} // Fall through to default client creation
            }
        }
    }
    
    // Default client creation
    let mut client_builder = reqwest::Client::builder().default_headers(headers);
    if config.accept_invalid_certs.unwrap_or(false) {
        client_builder = client_builder.danger_accept_invalid_certs(true);
    }
    if config.http_timeout.is_some() {
        client_builder = client_builder.timeout(std::time::Duration::from_secs(
            config.http_timeout.unwrap_or_default() as u64,
        ));
    }
    client_builder.build().unwrap_or_else(|_| reqwest::Client::new())
}

async fn execute_graphql_query<T>(
    config: &BlinkConfig,
    query: &str,
    variables: Option<serde_json::Value>,
) -> Result<T, ApiError>
where
    T: for<'de> serde::Deserialize<'de>,
{
    let client = client(config);
    let request = GraphQLRequest {
        query: query.to_string(),
        variables,
    };

    let response = client
        .post(config.base_url.as_deref().unwrap_or("https://api.blink.sv/graphql"))
        .json(&request)
        .send()
        .await
        .map_err(|e| ApiError::Http {
            reason: e.to_string(),
        })?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response.text().await.unwrap_or_default();
        return Err(ApiError::Http {
            reason: format!("HTTP {} - {}", status, error_text),
        });
    }

    let response_text = response.text().await.unwrap();
    let graphql_response: GraphQLResponse<T> = serde_json::from_str(&response_text)
        .map_err(|e| ApiError::Json {
            reason: format!("Failed to parse GraphQL response: {} - Response: {}", e, response_text),
        })?;

    if let Some(errors) = graphql_response.errors {
        return Err(ApiError::Api {
            reason: format!(
                "GraphQL errors: {}",
                errors
                    .iter()
                    .map(|e| e.message.clone())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        });
    }

    graphql_response.data.ok_or_else(|| ApiError::Json {
        reason: "No data in GraphQL response".to_string(),
    })
}

async fn get_btc_wallet_id(config: &BlinkConfig) -> Result<String, ApiError> {
    let query = r#"
        query Me {
            me {
                defaultAccount {
                    wallets {
                        id
                        walletCurrency
                        balance
                    }
                }
            }
        }
    "#;

    let response: MeQuery = execute_graphql_query(config, query, None).await?;
    
    let btc_wallet = response
        .me
        .default_account
        .wallets
        .into_iter()
        .find(|w| w.wallet_currency == "BTC")
        .ok_or_else(|| ApiError::Api {
            reason: "No BTC wallet found in account".to_string(),
        })?;

    Ok(btc_wallet.id)
}

pub async fn get_info(config: &BlinkConfig) -> Result<NodeInfo, ApiError> {
    let query = r#"
        query Me {
            me {
                defaultAccount {
                    wallets {
                        id
                        walletCurrency
                        balance
                    }
                }
            }
        }
    "#;

    let response: MeQuery = execute_graphql_query(config, query, None).await?;
    
    let btc_wallet = response
        .me
        .default_account
        .wallets
        .iter()
        .find(|w| w.wallet_currency == "BTC");

    let balance_sats = btc_wallet.map(|w| w.balance).unwrap_or(0);
    let balance_msats = balance_sats * 1000;

    Ok(NodeInfo {
        alias: "Blink Node".to_string(),
        color: "".to_string(),
        pubkey: "".to_string(),
        network: "mainnet".to_string(),
        block_height: 0,
        block_hash: "".to_string(),
        send_balance_msat: balance_msats,
        receive_balance_msat: balance_msats,
        fee_credit_balance_msat: 0,
        unsettled_send_balance_msat: 0,
        unsettled_receive_balance_msat: 0,
        pending_open_send_balance: 0,
        pending_open_receive_balance: 0,
    })
}

pub async fn create_invoice(
    config: &BlinkConfig,
    invoice_params: CreateInvoiceParams,
) -> Result<Transaction, ApiError> {
    match invoice_params.get_invoice_type() {
        InvoiceType::Bolt11 => {
            let wallet_id = get_btc_wallet_id(config).await?;
            
            let amount_sats = invoice_params.amount_msats.unwrap_or(0) / 1000;
            
            let query = r#"
                mutation LnInvoiceCreate($input: LnInvoiceCreateInput!) {
                    lnInvoiceCreate(input: $input) {
                        invoice {
                            paymentRequest
                            paymentHash
                            paymentSecret
                            satoshis
                        }
                        errors {
                            message
                        }
                    }
                }
            "#;

            let variables = serde_json::json!({
                "input": {
                    "amount": amount_sats.to_string(),
                    "walletId": wallet_id,
                    "memo": invoice_params.description
                }
            });

            let response: LnInvoiceCreateResponse = execute_graphql_query(config, query, Some(variables)).await?;

            if let Some(errors) = &response.ln_invoice_create.errors {
                if !errors.is_empty() {
                    return Err(ApiError::Api {
                        reason: format!(
                            "Invoice creation errors: {}",
                            errors
                                .iter()
                                .map(|e| e.message.clone())
                                .collect::<Vec<_>>()
                                .join(", ")
                        ),
                    });
                }
            }

            let invoice = response.ln_invoice_create.invoice.ok_or_else(|| ApiError::Json {
                reason: "No invoice data in response".to_string(),
            })?;

            // Parse the BOLT11 invoice to get expiry
            let expires_at = match Bolt11Invoice::from_str(&invoice.payment_request) {
                Ok(bolt11) => bolt11.expires_at()
                    .map(|duration| duration.as_secs() as i64)
                    .unwrap_or(0),
                Err(_) => 0,
            };

            Ok(Transaction {
                type_: "incoming".to_string(),
                invoice: invoice.payment_request,
                preimage: "".to_string(),
                payment_hash: invoice.payment_hash,
                amount_msats: invoice.satoshis * 1000,
                fees_paid: 0,
                created_at: chrono::Utc::now().timestamp(),
                expires_at,
                settled_at: 0,
                description: invoice_params.description.unwrap_or_default(),
                description_hash: invoice_params.description_hash.unwrap_or_default(),
                payer_note: Some("".to_string()),
                external_id: Some("".to_string()),
            })
        }
        InvoiceType::Bolt12 => Err(ApiError::Json {
            reason: "Bolt12 not implemented for Blink".to_string(),
        }),
    }
}

pub async fn pay_invoice(
    config: &BlinkConfig,
    invoice_params: PayInvoiceParams,
) -> Result<PayInvoiceResponse, ApiError> {
    let wallet_id = get_btc_wallet_id(config).await?;

    // First probe the fee
    let fee_probe_query = r#"
        mutation lnInvoiceFeeProbe($input: LnInvoiceFeeProbeInput!) {
            lnInvoiceFeeProbe(input: $input) {
                errors {
                    message
                }
                amount
            }
        }
    "#;

    let fee_probe_variables = serde_json::json!({
        "input": {
            "paymentRequest": invoice_params.invoice,
            "walletId": wallet_id
        }
    });

    let fee_response: LnInvoiceFeeProbeResponse = execute_graphql_query(config, fee_probe_query, Some(fee_probe_variables)).await?;

    let fee_msats = if let Some(errors) = &fee_response.ln_invoice_fee_probe.errors {
        if !errors.is_empty() {
            return Err(ApiError::Api {
                reason: format!(
                    "Fee probe errors: {}",
                    errors
                        .iter()
                        .map(|e| e.message.clone())
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            });
        } else {
            fee_response.ln_invoice_fee_probe.amount.unwrap_or(0) * 1000
        }
    } else {
        fee_response.ln_invoice_fee_probe.amount.unwrap_or(0) * 1000
    };

    // Now send the payment
    let payment_query = r#"
        mutation LnInvoicePaymentSend($input: LnInvoicePaymentInput!) {
            lnInvoicePaymentSend(input: $input) {
                status
                errors {
                    message
                    path
                    code
                }
            }
        }
    "#;

    let payment_variables = serde_json::json!({
        "input": {
            "paymentRequest": invoice_params.invoice,
            "walletId": wallet_id
        }
    });

    let payment_response: LnInvoicePaymentSendResponse = execute_graphql_query(config, payment_query, Some(payment_variables)).await?;

    if let Some(errors) = &payment_response.ln_invoice_payment_send.errors {
        if !errors.is_empty() {
            return Err(ApiError::Api {
                reason: format!(
                    "Payment errors: {}",
                    errors
                        .iter()
                        .map(|e| e.message.clone())
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            });
        }
    }

    if payment_response.ln_invoice_payment_send.status != "SUCCESS" {
        return Err(ApiError::Api {
            reason: format!(
                "Payment failed with status: {}",
                payment_response.ln_invoice_payment_send.status
            ),
        });
    }

    // Extract payment hash from the BOLT11 invoice
    let payment_hash = match Bolt11Invoice::from_str(&invoice_params.invoice) {
        Ok(invoice) => format!("{:x}", invoice.payment_hash()),
        Err(_) => "".to_string(),
    };

    Ok(PayInvoiceResponse {
        payment_hash,
        preimage: "".to_string(), // Blink doesn't expose preimage
        fee_msats,
    })
}

pub async fn decode(_config: &BlinkConfig, str: String) -> Result<String, ApiError> {
    // Blink doesn't have a decode endpoint, return raw string
    Ok(str)
}

pub async fn get_offer(_config: &BlinkConfig, _search: Option<String>) -> Result<Offer, ApiError> {
    Err(ApiError::Json {
        reason: "Bolt12 not implemented for Blink".to_string(),
    })
}

pub async fn list_offers(
    _config: &BlinkConfig,
    _search: Option<String>,
) -> Result<Vec<Offer>, ApiError> {
    Err(ApiError::Json {
        reason: "Bolt12 not implemented for Blink".to_string(),
    })
}

pub async fn create_offer(
    _config: &BlinkConfig,
    _amount_msats: Option<i64>,
    _description: Option<String>,
    _expiry: Option<i64>,
) -> Result<Transaction, ApiError> {
    Err(ApiError::Json {
        reason: "Bolt12 not implemented for Blink".to_string(),
    })
}

pub async fn fetch_invoice_from_offer(
    _config: &BlinkConfig,
    _offer: String,
    _amount_msats: i64,
    _payer_note: Option<String>,
) -> Result<crate::cln::types::FetchInvoiceResponse, ApiError> {
    Err(ApiError::Json {
        reason: "Bolt12 not implemented for Blink".to_string(),
    })
}

pub async fn pay_offer(
    _config: &BlinkConfig,
    _offer: String,
    _amount_msats: i64,
    _payer_note: Option<String>,
) -> Result<PayInvoiceResponse, ApiError> {
    Err(ApiError::Json {
        reason: "Bolt12 not implemented for Blink".to_string(),
    })
}

pub async fn lookup_invoice(
    config: &BlinkConfig,
    payment_hash: Option<String>,
    from: Option<i64>,
    limit: Option<i64>,
    search: Option<String>,
) -> Result<Transaction, ApiError> {
    let target_payment_hash = payment_hash.unwrap_or_default();
    
    // Get transactions and look for the specific payment hash, using parameters or defaults
    let transactions = list_transactions(
        config, 
        from.unwrap_or(0), 
        limit.unwrap_or(100), 
        search
    ).await?;
    
    let transaction = transactions
        .into_iter()
        .find(|t| t.payment_hash == target_payment_hash)
        .ok_or_else(|| ApiError::Json {
            reason: format!("Transaction not found for payment hash: {}", target_payment_hash),
        })?;

    Ok(transaction)
}

pub async fn list_transactions(
    config: &BlinkConfig,
    from: i64,
    limit: i64,
    search: Option<String>,
) -> Result<Vec<Transaction>, ApiError> {
    let query = r#"
        query TransactionsQuery($first: Int, $last: Int, $after: String, $before: String) {
            me {
                defaultAccount {
                    transactions(first: $first, last: $last, after: $after, before: $before) {
                        edges {
                            cursor
                            node {
                                id
                                createdAt
                                direction
                                status
                                memo
                                settlementAmount
                                settlementCurrency
                                settlementFee
                                settlementDisplayAmount
                                settlementDisplayCurrency
                                settlementDisplayFee
                                settlementPrice {
                                    base
                                    offset
                                    currencyUnit
                                }
                                initiationVia {
                                    __typename
                                    ... on InitiationViaLn {
                                        paymentHash
                                    }
                                }
                                settlementVia {
                                    __typename
                                    ... on SettlementViaLn {
                                        preImage
                                    }
                                }
                            }
                        }
                        pageInfo {
                            hasNextPage
                            hasPreviousPage
                            startCursor
                            endCursor
                        }
                    }
                }
            }
        }
    "#;

    // Simple approach: map limit directly to $first, handle from with client-side skip
    // This is cleaner than trying to convert integer offsets to opaque cursors
    let variables = serde_json::json!({
        "first": (from + limit) as i32,  // Fetch enough to skip 'from' records
        "last": serde_json::Value::Null,
        "after": serde_json::Value::Null,
        "before": serde_json::Value::Null
    });

    let response: TransactionsQuery = execute_graphql_query(config, query, Some(variables)).await?;
    
    let mut all_transactions = Vec::new();
    
    for edge in response.me.default_account.transactions.edges {
        let node = edge.node;
        
        // Extract Lightning-specific information
        let payment_hash = match node.initiation_via {
            Some(InitiationVia::InitiationViaLn { payment_hash }) => payment_hash,
            _ => "".to_string(),
        };

        let preimage = match node.settlement_via {
            Some(SettlementVia::SettlementViaLn { pre_image }) => pre_image.unwrap_or_default(),
            _ => "".to_string(),
        };

        // Handle amount conversion based on settlement currency
        let (amount_msats, fees_paid) = if let Some(currency) = &node.settlement_currency {
            if currency == "BTC" {
                // BTC amounts are in satoshis, convert to millisatoshis
                let amount = (node.settlement_amount.unwrap_or(0).abs()) * 1000;
                let fees = (node.settlement_fee.unwrap_or(0).abs()) * 1000;
                (amount, fees)
            } else if currency == "USD" {
                // USD amounts - for now return 0 as we can't meaningfully convert to satoshis
                // without current exchange rate data
                (0, 0)
            } else {
                // Other currencies
                (0, 0)
            }
        } else {
            // No settlement currency available
            (0, 0)
        };
        
        // Use the timestamp directly since it's already a Unix timestamp
        let created_at = node.created_at;

        let settled_at = if node.status == "SUCCESS" {
            created_at
        } else {
            0
        };

        all_transactions.push(Transaction {
            type_: if node.direction == "SEND" { "outgoing" } else { "incoming" }.to_string(),
            invoice: "".to_string(), // Not available from this query
            preimage,
            payment_hash,
            amount_msats,
            fees_paid,
            created_at,
            expires_at: 0, // Not available from this query
            settled_at,
            description: node.memo.unwrap_or_default(),
            description_hash: "".to_string(),
            payer_note: Some("".to_string()),
            external_id: Some(node.id),
        });
    }

    // Apply client-side search filtering if search term is provided
    if let Some(search_term) = search {
        let search_lower = search_term.to_lowercase();
        all_transactions.retain(|tx| {
            tx.description.to_lowercase().contains(&search_lower) ||
            tx.payment_hash.to_lowercase().contains(&search_lower) ||
            tx.preimage.to_lowercase().contains(&search_lower)
        });
    }

    // Apply client-side pagination: skip 'from' records and take 'limit' records
    let skip_count = from as usize;
    let take_count = limit as usize;
    
    if skip_count < all_transactions.len() {
        let end_index = std::cmp::min(skip_count + take_count, all_transactions.len());
        all_transactions = all_transactions[skip_count..end_index].to_vec();
    } else {
        all_transactions.clear();
    }

    Ok(all_transactions)
}

// Core logic shared by both implementations  
pub async fn poll_invoice_events<F>(config: &BlinkConfig, params: OnInvoiceEventParams, mut callback: F)
where
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
            config,
            params.payment_hash.clone(),
            None,
            None,
            params.search.clone(),
        ).await {
            Ok(transaction) => {
                if transaction.settled_at > 0 {
                    ("success".to_string(), Some(transaction))
                } else {
                    ("pending".to_string(), Some(transaction))
                }
            }
            Err(_) => ("error".to_string(), None),
        };

        callback(status.clone(), transaction.clone());

        if status == "success" || status == "failure" {
            break;
        }

        tokio::time::sleep(Duration::from_secs(params.polling_delay_sec as u64)).await;
    }
}

pub async fn on_invoice_events(
    config: BlinkConfig,
    params: OnInvoiceEventParams,
    callback: std::sync::Arc<dyn OnInvoiceEventCallback>,
) {
    poll_invoice_events(&config, params, move |status, tx| match status.as_str() {
        "success" => callback.success(tx),
        "pending" => callback.pending(tx),
        "failure" | _ => callback.failure(tx),
    }).await;
}