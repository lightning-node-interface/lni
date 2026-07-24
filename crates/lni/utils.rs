use lightning::blinded_path::message::BlindedMessagePath;
use lightning::blinded_path::{Direction, IntroductionNode};
use lightning::offers::offer::{Amount, Offer as Bolt12Offer, Quantity};
use lightning_invoice::{Bolt11Invoice, Bolt11InvoiceDescriptionRef};
use serde_json::{json, Map, Value};
use std::error::Error;
use std::str::FromStr;

const MSATS_PER_BTC: i128 = 100_000_000_000;

pub fn parse_btc_to_msats_exact(amount: &str) -> Option<i64> {
    let (whole, fraction) = match amount.split_once('.') {
        Some((whole, fraction)) => (whole, fraction),
        None => (amount, ""),
    };
    if whole.is_empty()
        || !whole.bytes().all(|byte| byte.is_ascii_digit())
        || (!fraction.is_empty() && !fraction.bytes().all(|byte| byte.is_ascii_digit()))
        || amount.ends_with('.')
    {
        return None;
    }

    if fraction.len() > 11 && fraction.as_bytes()[11..].iter().any(|byte| *byte != b'0') {
        return None;
    }

    let whole_msats = whole.parse::<i128>().ok()?.checked_mul(MSATS_PER_BTC)?;
    let significant_fraction = &fraction[..fraction.len().min(11)];
    let fractional_msats = if significant_fraction.is_empty() {
        0
    } else {
        significant_fraction
            .parse::<i128>()
            .ok()?
            .checked_mul(10_i128.checked_pow((11 - significant_fraction.len()) as u32)?)?
    };

    i64::try_from(whole_msats.checked_add(fractional_msats)?).ok()
}

pub fn msats_to_btc(amount_msats: i64) -> String {
    format!("{:.8}", amount_msats as f64 / MSATS_PER_BTC as f64)
}

pub fn format_msats_as_sats(amount_msats: i64) -> String {
    let is_negative = amount_msats < 0;
    let absolute_msats = i128::from(amount_msats).abs();
    let whole_sats = absolute_msats / 1_000;
    let fractional_msats = absolute_msats % 1_000;
    let sign = if is_negative { "-" } else { "" };
    let amount = if fractional_msats == 0 {
        format!("{sign}{whole_sats}")
    } else {
        let fraction = format!("{fractional_msats:03}");
        format!("{sign}{whole_sats}.{}", fraction.trim_end_matches('0'))
    };
    let unit = if absolute_msats == 1_000 {
        "sat"
    } else {
        "sats"
    };

    format!("{amount} {unit}")
}

pub fn calculate_fee_msats(
    bolt11: &str,
    fee_percentage: f64,
    amount_msats: Option<u64>,
) -> Result<u64, Box<dyn Error>> {
    // Decode the BOLT11 invoice
    let invoice = Bolt11Invoice::from_str(bolt11)
        .map_err(|e| format!("Failed to parse BOLT11 invoice: {}", e))?;

    println!("invoice: {:?}", invoice);

    // Get the amount from the invoice
    let invoice_amount_msats = invoice.amount_milli_satoshis().unwrap_or(0);
    println!("invoice amount: {:?}", invoice_amount_msats);

    // Determine the amount to use for fee calculation
    let amount_msats = if invoice_amount_msats == 0 {
        amount_msats.ok_or("Amount in invoice is 0 and no amount_msats provided")?
    } else {
        invoice_amount_msats
    };

    // Calculate the fee
    let fee_msats = (amount_msats as f64 * fee_percentage / 100.0).round() as u64;

    println!(
        "calculated fee_limit_msat {} from percentage, {} for the total amount {}",
        fee_msats, fee_percentage, amount_msats
    );

    Ok(fee_msats)
}

pub fn decode_bolt11(str: String) -> Result<String, crate::ApiError> {
    let invoice = Bolt11Invoice::from_str(&str).map_err(|e| {
        crate::ApiError::InvalidInput(format!("Failed to parse BOLT11 invoice: {}", e))
    })?;

    let mut decoded = Map::new();
    decoded.insert("paymentRequest".to_string(), json!(str));
    decoded.insert("type".to_string(), json!("bolt11_invoice"));

    if let Some(amount_msats) = invoice.amount_milli_satoshis() {
        decoded.insert("amount".to_string(), json!(amount_msats.to_string()));
        decoded.insert("amountMsats".to_string(), json!(amount_msats));
    }

    let timestamp = invoice.duration_since_epoch().as_secs();
    let expiry = invoice.expiry_time().as_secs();
    decoded.insert("timestamp".to_string(), json!(timestamp));
    decoded.insert("expiresAt".to_string(), json!(timestamp + expiry));
    decoded.insert(
        "payment_hash".to_string(),
        json!(invoice.payment_hash().to_string()),
    );

    match invoice.description() {
        Bolt11InvoiceDescriptionRef::Direct(description) => {
            decoded.insert("description".to_string(), json!(description.to_string()));
        }
        Bolt11InvoiceDescriptionRef::Hash(hash) => {
            decoded.insert("description_hash".to_string(), json!(hash.0.to_string()));
        }
    }

    decoded.insert(
        "payment_secret".to_string(),
        json!(hex::encode(invoice.payment_secret().0)),
    );
    decoded.insert("expiry".to_string(), json!(expiry));
    decoded.insert(
        "min_final_cltv_expiry".to_string(),
        json!(invoice.min_final_cltv_expiry_delta()),
    );

    if let Some(payee) = invoice.payee_pub_key() {
        decoded.insert("payee_node_key".to_string(), json!(payee.to_string()));
        decoded.insert("payeeNodeKey".to_string(), json!(payee.to_string()));
    }

    let route_hints: Vec<String> = invoice
        .route_hints()
        .into_iter()
        .map(|hint| format!("{:?}", hint))
        .collect();
    decoded.insert("route_hints".to_string(), json!(route_hints));

    serde_json::to_string(&Value::Object(decoded)).map_err(crate::ApiError::from)
}

pub fn decode_offer(str: String) -> Result<String, crate::ApiError> {
    let offer = Bolt12Offer::from_str(&str).map_err(|e| {
        crate::ApiError::InvalidInput(format!("Failed to parse BOLT12 offer: {:?}", e))
    })?;

    let mut sections = vec![json!({
        "name": "offer",
        "value": str,
    })];

    let chains: Vec<String> = offer
        .chains()
        .into_iter()
        .map(|chain| format!("{:?}", chain))
        .collect();
    if !chains.is_empty() {
        sections.push(json!({
            "name": "chains",
            "value": chains,
        }));
    }

    let metadata = offer.metadata().map(hex::encode);
    if let Some(metadata) = &metadata {
        sections.push(json!({
            "name": "metadata",
            "value": metadata,
        }));
    }

    let (currency, amount, amount_msats) = match offer.amount() {
        Some(Amount::Bitcoin { amount_msats }) => {
            (None, Some(amount_msats.to_string()), Some(amount_msats))
        }
        Some(Amount::Currency {
            iso4217_code,
            amount,
        }) => (
            Some(iso4217_code.to_string()),
            Some(amount.to_string()),
            None,
        ),
        None => (None, None, None),
    };
    if let Some(currency) = &currency {
        sections.push(json!({
            "name": "currency",
            "value": currency,
        }));
    }
    if let Some(amount) = &amount {
        sections.push(json!({
            "name": "amount",
            "value": amount,
        }));
    }

    let description = offer
        .description()
        .map(|description| description.to_string());
    if let Some(description) = &description {
        sections.push(json!({
            "name": "description",
            "value": description,
        }));
    }

    sections.push(json!({
        "name": "features",
        "value": format!("{:?}", offer.offer_features()),
    }));

    let absolute_expiry = offer.absolute_expiry().map(|expiry| expiry.as_secs());
    if let Some(absolute_expiry) = absolute_expiry {
        sections.push(json!({
            "name": "absolute_expiry",
            "value": absolute_expiry,
        }));
    }

    let paths = normalize_blinded_paths(offer.paths());
    if !paths.is_empty() {
        sections.push(json!({
            "name": "paths",
            "value": paths,
        }));
    }

    let issuer = offer.issuer().map(|issuer| issuer.to_string());
    if let Some(issuer) = &issuer {
        sections.push(json!({
            "name": "issuer",
            "value": issuer,
        }));
    }

    let quantity_max = match offer.supported_quantity() {
        Quantity::One => json!(1),
        Quantity::Unbounded => json!("unbounded"),
        Quantity::Bounded(quantity) => json!(quantity.get()),
    };
    sections.push(json!({
        "name": "quantity_max",
        "value": quantity_max.clone(),
    }));

    let issuer_signing_pubkey = offer
        .issuer_signing_pubkey()
        .map(|pubkey| pubkey.to_string());
    if let Some(issuer_signing_pubkey) = &issuer_signing_pubkey {
        sections.push(json!({
            "name": "issuer_id",
            "value": issuer_signing_pubkey,
        }));
    }

    serde_json::to_string(&json!({
        "offer": str,
        "prefix": "lno",
        "type": "bolt12_offer",
        "id": hex::encode(offer.id().0),
        "sections": sections,
        "chains": chains,
        "metadata": metadata,
        "currency": currency,
        "amount": amount,
        "amountMsats": amount_msats,
        "description": description,
        "features": format!("{:?}", offer.offer_features()),
        "absoluteExpiry": absolute_expiry,
        "paths": paths,
        "issuer": issuer,
        "quantityMax": quantity_max,
        "issuerSigningPubkey": issuer_signing_pubkey,
    }))
    .map_err(crate::ApiError::from)
}

fn normalize_blinded_paths(paths: &[BlindedMessagePath]) -> Vec<serde_json::Value> {
    paths
        .iter()
        .map(|path| {
            let introduction_node = match path.introduction_node() {
                IntroductionNode::NodeId(node_id) => json!({
                    "type": "node_id",
                    "nodeId": node_id.to_string(),
                }),
                IntroductionNode::DirectedShortChannelId(direction, short_channel_id) => {
                    let direction = match direction {
                        Direction::NodeOne => "node_one",
                        Direction::NodeTwo => "node_two",
                    };
                    json!({
                        "type": "directed_short_channel_id",
                        "direction": direction,
                        "shortChannelId": short_channel_id.to_string(),
                    })
                }
            };

            let blinded_hops: Vec<serde_json::Value> = path
                .blinded_hops()
                .iter()
                .map(|hop| {
                    json!({
                        "blindedNodeId": hop.blinded_node_id.to_string(),
                        "encryptedPayload": hex::encode(&hop.encrypted_payload),
                    })
                })
                .collect();

            json!({
                "introductionNode": introduction_node,
                "blindingPoint": path.blinding_point().to_string(),
                "blindedHops": blinded_hops,
            })
        })
        .collect()
}

#[cfg(test)]
mod decode_tests {
    const BOLT11: &str = "lnbc2500u1pvjluezsp5zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zygspp5qqqsyqcyq5rqwzqfqqqsyqcyq5rqwzqfqqqsyqcyq5rqwzqfqypqdq5xysxxatsyp3k7enxv4jsxqzpu9qrsgquk0rl77nj30yxdy8j9vdx85fkpmdla2087ne0xh8nhedh8w27kyke0lp53ut353s06fv3qfegext0eh0ymjpf39tuven09sam30g4vgpfna3rh";
    const BOLT12_OFFER: &str =
        "lno1pgx9getnwss8vetrw3hhyuckyypwa3eyt44h6txtxquqh7lz5djge4afgfjn7k4rgrkuag0jsd5xvxg";
    const BOLT12_OFFER_WITH_CURRENCY_AMOUNT: &str = "lno1qcp4256ypqpzwyq2p32x2um5ypmx2cm5dae8x93pqthvwfzadd7jejes8q9lhc4rvjxd022zv5l44g6qah82ru5rdpnpj";
    const BOLT12_OFFER_WITH_PATH: &str = "lno1pgx9getnwss8vetrw3hhyucs5ypjgef743p5fzqq9nqxh0ah7y87rzv3ud0eleps9kl2d5348hq2k8qzqgpqyqszqgpqyqszqgpqyqszqgpqyqszqgpqyqszqgpqyqszqgpqyqszqgpqyqszqgpqyqszqgpqyqszqgpqyqszqgpqyqszqgpqyqszqgqpqqqqqqqqqqqqqqqqqqqqqqqqqqqzqgpqyqszqgpqyqszqgpqyqszqgpqyqszqgpqyqszqgpqyqszqgpqqzq3zyg3zyg3zyg3vggzamrjghtt05kvkvpcp0a79gmy3nt6jsn98ad2xs8de6sl9qmgvcvs";

    #[test]
    fn parses_btc_amounts_to_msats_exactly() {
        assert_eq!(super::parse_btc_to_msats_exact("0.00000000999"), Some(999));
        assert_eq!(
            super::parse_btc_to_msats_exact("0.00000001001"),
            Some(1_001)
        );
        assert_eq!(
            super::parse_btc_to_msats_exact("1.00000000000"),
            Some(100_000_000_000)
        );
        assert_eq!(super::parse_btc_to_msats_exact("0.000000000001"), None);
        assert_eq!(super::parse_btc_to_msats_exact("not-an-amount"), None);
        assert_eq!(super::msats_to_btc(1_000), "0.00000001");
        assert_eq!(super::format_msats_as_sats(0), "0 sats");
        assert_eq!(super::format_msats_as_sats(1_000), "1 sat");
        assert_eq!(super::format_msats_as_sats(1_001), "1.001 sats");
        assert_eq!(super::format_msats_as_sats(4_000), "4 sats");
    }

    #[test]
    fn decodes_bolt11_as_json() {
        let decoded = super::decode_bolt11(BOLT11.to_string()).unwrap();
        let value: serde_json::Value = serde_json::from_str(&decoded).unwrap();
        assert_eq!(value["paymentRequest"], BOLT11);
        assert_eq!(value["type"], "bolt11_invoice");
        assert_eq!(
            value["payment_hash"],
            "0001020304050607080900010203040506070809000102030405060708090102"
        );
        assert_eq!(value["description"], "1 cup coffee");
        assert_eq!(value["amount"], "250000000");
        assert_eq!(value["amountMsats"], 250000000);
        assert!(value["sections"].is_null());
    }

    #[test]
    fn decodes_bolt12_offer_as_json() {
        let decoded = super::decode_offer(BOLT12_OFFER.to_string()).unwrap();
        let value: serde_json::Value = serde_json::from_str(&decoded).unwrap();
        assert_eq!(value["offer"], BOLT12_OFFER);
        assert_eq!(value["type"], "bolt12_offer");
        assert!(
            value["issuerSigningPubkey"]
                .as_str()
                .unwrap_or_default()
                .len()
                > 0
        );
    }

    #[test]
    fn decodes_bolt12_currency_amount_without_msats() {
        let decoded = super::decode_offer(BOLT12_OFFER_WITH_CURRENCY_AMOUNT.to_string()).unwrap();
        let value: serde_json::Value = serde_json::from_str(&decoded).unwrap();
        assert_eq!(value["currency"], "USD");
        assert_eq!(value["amount"], "10000");
        assert!(value["amountMsats"].is_null());
    }

    #[test]
    fn decodes_bolt12_offer_blinded_paths_as_json() {
        let decoded = super::decode_offer(BOLT12_OFFER_WITH_PATH.to_string()).unwrap();
        let value: serde_json::Value = serde_json::from_str(&decoded).unwrap();
        let paths = value["paths"].as_array().unwrap();
        assert!(!paths.is_empty());
        assert_eq!(paths[0]["introductionNode"]["type"], "node_id");
        assert!(paths[0]["blindingPoint"].as_str().unwrap_or_default().len() > 0);
        assert!(!paths[0]["blindedHops"].as_array().unwrap().is_empty());
        assert!(value["sections"].as_array().unwrap().iter().any(|section| {
            section["name"] == "paths" && section["value"].as_array().is_some()
        }));
    }
}
