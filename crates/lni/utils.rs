use lightning::blinded_path::message::BlindedMessagePath;
use lightning::blinded_path::{Direction, IntroductionNode};
use lightning::offers::offer::{Amount, Offer as Bolt12Offer, Quantity};
use lightning_invoice::{Bolt11Invoice, Bolt11InvoiceDescriptionRef};
use serde_json::json;
use std::error::Error;
use std::str::FromStr;

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
    let invoice = Bolt11Invoice::from_str(&str)
        .map_err(|e| crate::ApiError::InvalidInput(format!("Failed to parse BOLT11 invoice: {}", e)))?;

    let mut sections = vec![json!({
        "name": "paymentRequest",
        "value": str,
    })];

    if let Some(amount_msats) = invoice.amount_milli_satoshis() {
        sections.push(json!({
            "name": "amount",
            "value": amount_msats.to_string(),
        }));
    }

    sections.push(json!({
        "name": "timestamp",
        "value": invoice.duration_since_epoch().as_secs(),
    }));
    sections.push(json!({
        "name": "payment_hash",
        "value": invoice.payment_hash().to_string(),
    }));

    match invoice.description() {
        Bolt11InvoiceDescriptionRef::Direct(description) => sections.push(json!({
            "name": "description",
            "value": description.to_string(),
        })),
        Bolt11InvoiceDescriptionRef::Hash(hash) => sections.push(json!({
            "name": "description_hash",
            "value": hash.0.to_string(),
        })),
    }

    sections.push(json!({
        "name": "payment_secret",
        "value": hex::encode(invoice.payment_secret().0),
    }));
    sections.push(json!({
        "name": "expiry",
        "value": invoice.expiry_time().as_secs(),
    }));
    sections.push(json!({
        "name": "min_final_cltv_expiry",
        "value": invoice.min_final_cltv_expiry_delta(),
    }));

    if let Some(payee) = invoice.payee_pub_key() {
        sections.push(json!({
            "name": "payee_pub_key",
            "value": payee.to_string(),
        }));
    }

    let route_hints: Vec<String> = invoice
        .route_hints()
        .into_iter()
        .map(|hint| format!("{:?}", hint))
        .collect();

    serde_json::to_string(&json!({
        "paymentRequest": str,
        "sections": sections,
        "expiry": invoice.expiry_time().as_secs(),
        "route_hints": route_hints,
    }))
    .map_err(crate::ApiError::from)
}

pub fn decode_offer(str: String) -> Result<String, crate::ApiError> {
    let offer = Bolt12Offer::from_str(&str)
        .map_err(|e| crate::ApiError::InvalidInput(format!("Failed to parse BOLT12 offer: {:?}", e)))?;

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
        Some(Amount::Bitcoin { amount_msats }) => (None, Some(amount_msats.to_string()), Some(amount_msats)),
        Some(Amount::Currency { iso4217_code, amount }) => (
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

    let description = offer.description().map(|description| description.to_string());
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

    let issuer_signing_pubkey = offer.issuer_signing_pubkey().map(|pubkey| pubkey.to_string());
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
    const BOLT12_OFFER: &str = "lno1pgx9getnwss8vetrw3hhyuckyypwa3eyt44h6txtxquqh7lz5djge4afgfjn7k4rgrkuag0jsd5xvxg";
    const BOLT12_OFFER_WITH_PATH: &str = "lno1pgx9getnwss8vetrw3hhyucs5ypjgef743p5fzqq9nqxh0ah7y87rzv3ud0eleps9kl2d5348hq2k8qzqgpqyqszqgpqyqszqgpqyqszqgpqyqszqgpqyqszqgpqyqszqgpqyqszqgpqyqszqgpqyqszqgpqyqszqgpqyqszqgpqyqszqgpqyqszqgqpqqqqqqqqqqqqqqqqqqqqqqqqqqqzqgpqyqszqgpqyqszqgpqyqszqgpqyqszqgpqyqszqgpqyqszqgpqqzq3zyg3zyg3zyg3vggzamrjghtt05kvkvpcp0a79gmy3nt6jsn98ad2xs8de6sl9qmgvcvs";

    #[test]
    fn decodes_bolt11_as_json() {
        let decoded = super::decode_bolt11(BOLT11.to_string()).unwrap();
        let value: serde_json::Value = serde_json::from_str(&decoded).unwrap();
        assert_eq!(value["paymentRequest"], BOLT11);
        assert!(value["sections"].as_array().unwrap().iter().any(|section| {
            section["name"] == "payment_hash"
        }));
    }

    #[test]
    fn decodes_bolt12_offer_as_json() {
        let decoded = super::decode_offer(BOLT12_OFFER.to_string()).unwrap();
        let value: serde_json::Value = serde_json::from_str(&decoded).unwrap();
        assert_eq!(value["offer"], BOLT12_OFFER);
        assert_eq!(value["type"], "bolt12_offer");
        assert!(value["issuerSigningPubkey"].as_str().unwrap_or_default().len() > 0);
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
