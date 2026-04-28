use regex::Regex;

const NWC_METHOD_PERMISSIONS: &[&str] = &[
    "get_info",
    "get_balance",
    "make_invoice",
    "pay_invoice",
    "lookup_invoice",
    "list_transactions",
];

const CLN_METHOD_PERMISSIONS: &[&str] = &[
    "getinfo",
    "listfunds",
    "invoice",
    "pay",
    "offer",
    "fetchinvoice",
    "listoffers",
    "listinvoices",
    "decode",
];

const STRIKE_SCOPE_PERMISSIONS: &[(&str, &[&str])] = &[
    ("get_info", &["partner.balances.read"]),
    ("create_invoice", &["partner.receive-request.create"]),
    (
        "pay_invoice",
        &[
            "partner.payment-quote.lightning.create",
            "partner.payment-quote.execute",
        ],
    ),
    ("lookup_invoice", &["partner.receive-request.read"]),
    (
        "list_transactions",
        &["partner.receive-request.read", "partner.payment.read"],
    ),
    ("on_invoice_events", &["partner.receive-request.read"]),
];

const BLINK_SCOPE_PERMISSIONS: &[(&str, &[&str])] = &[
    ("get_info", &["read"]),
    ("create_invoice", &["receive"]),
    ("pay_invoice", &["write"]),
    ("lookup_invoice", &["read"]),
    ("list_transactions", &["read"]),
    ("on_invoice_events", &["read"]),
];

pub fn nwc_method_permissions() -> Vec<String> {
    normalize_permissions(
        NWC_METHOD_PERMISSIONS
            .iter()
            .map(|permission| (*permission).to_string()),
    )
}

pub fn normalize_permissions<I>(permissions: I) -> Vec<String>
where
    I: IntoIterator<Item = String>,
{
    let mut values: Vec<String> = permissions
        .into_iter()
        .map(|permission| permission.trim().to_string())
        .filter(|permission| !permission.is_empty())
        .collect();
    values.sort();
    values.dedup();
    values
}

pub fn parse_cln_rune_permissions(rune: &str) -> Vec<String> {
    let decoded = match base64::decode_config(pad_base64_url(rune), base64::URL_SAFE) {
        Ok(bytes) => bytes,
        Err(_) => {
            return normalize_permissions(
                CLN_METHOD_PERMISSIONS
                    .iter()
                    .map(|permission| (*permission).to_string()),
            )
        }
    };
    let text = String::from_utf8_lossy(&decoded);
    let matcher = Regex::new(r"method(?:=|\^)[A-Za-z0-9_.:-]+").expect("valid rune method regex");
    let matches: Vec<&str> = matcher
        .find_iter(&text)
        .map(|match_| match_.as_str())
        .collect();

    if matches.is_empty() {
        return normalize_permissions(
            CLN_METHOD_PERMISSIONS
                .iter()
                .map(|permission| (*permission).to_string()),
        );
    }

    let mut expanded = Vec::new();
    for permission in matches {
        if let Some(method) = permission.strip_prefix("method=") {
            expanded.push(method.to_string());
            continue;
        }

        if let Some(prefix) = permission.strip_prefix("method^") {
            let mut matched = false;
            for method in CLN_METHOD_PERMISSIONS {
                if method.starts_with(prefix) {
                    expanded.push((*method).to_string());
                    matched = true;
                }
            }
            if !matched {
                expanded.push(format!("{prefix}*"));
            }
        }
    }

    normalize_permissions(expanded)
}

pub fn parse_lnd_macaroon_permissions(bytes: &[u8]) -> Vec<String> {
    let text = String::from_utf8_lossy(bytes);
    let matcher = Regex::new(r"[a-z][a-z0-9_-]*:(?:read|write|generate)")
        .expect("valid lnd macaroon permission regex");
    normalize_permissions(
        matcher
            .find_iter(&text)
            .map(|match_| match_.as_str().to_string()),
    )
}

pub fn parse_strike_oauth_permissions(access_token: &str) -> Option<Vec<String>> {
    let payload = decode_jwt_payload(access_token)?;
    let scopes = read_scope_values(&payload);
    let mut permissions = vec!["decode".to_string()];

    for (permission, required_scopes) in STRIKE_SCOPE_PERMISSIONS {
        if required_scopes.iter().all(|scope| scopes.iter().any(|item| item == scope)) {
            permissions.push((*permission).to_string());
        }
    }

    Some(normalize_permissions(permissions))
}

pub fn parse_blink_token_permissions(token: &str) -> Option<Vec<String>> {
    let payload = decode_jwt_payload(token)?;
    let scopes: Vec<String> = read_scope_values(&payload)
        .into_iter()
        .map(|scope| scope.to_lowercase())
        .collect();
    let mut permissions = vec!["decode".to_string()];

    for (permission, required_scopes) in BLINK_SCOPE_PERMISSIONS {
        if required_scopes.iter().all(|scope| scopes.iter().any(|item| item == scope)) {
            permissions.push((*permission).to_string());
        }
    }

    Some(normalize_permissions(permissions))
}

fn decode_jwt_payload(access_token: &str) -> Option<serde_json::Value> {
    let mut parts = access_token.split('.');
    let _header = parts.next()?;
    let payload = parts.next()?;
    let _signature = parts.next()?;
    let decoded = base64::decode_config(pad_base64_url(payload), base64::URL_SAFE).ok()?;
    serde_json::from_slice(&decoded).ok()
}

fn read_scope_values(payload: &serde_json::Value) -> Vec<String> {
    let mut scopes = Vec::new();

    for key in ["scope", "scp", "scopes"] {
        match payload.get(key) {
            Some(serde_json::Value::String(value)) => {
                scopes.extend(value.split_whitespace().map(|scope| scope.to_string()));
            }
            Some(serde_json::Value::Array(values)) => {
                scopes.extend(values.iter().filter_map(|value| value.as_str().map(|scope| scope.to_string())));
            }
            _ => {}
        }
    }

    normalize_permissions(scopes)
}

fn pad_base64_url(input: &str) -> String {
    let normalized = input.trim();
    let padding = (4 - (normalized.len() % 4)) % 4;
    format!("{}{}", normalized, "=".repeat(padding))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expands_cln_method_prefix_restrictions() {
        let rune = base64::encode_config("unique-id&method^list|method=getinfo", base64::URL_SAFE);

        assert_eq!(
            parse_cln_rune_permissions(&rune),
            vec![
                "getinfo".to_string(),
                "listfunds".to_string(),
                "listinvoices".to_string(),
                "listoffers".to_string(),
            ]
        );
    }

    #[test]
    fn parses_lnd_permission_pairs_from_macaroon_bytes() {
        assert_eq!(
            parse_lnd_macaroon_permissions(b"info:read invoices:write offchain:read"),
            vec![
                "info:read".to_string(),
                "invoices:write".to_string(),
                "offchain:read".to_string(),
            ]
        );
    }

    #[test]
    fn maps_strike_oauth_scopes_to_lni_permissions() {
        let header = base64_url_json(&serde_json::json!({ "alg": "none" }));
        let payload = base64_url_json(&serde_json::json!({
            "scope": "openid partner.balances.read partner.receive-request.create partner.receive-request.read partner.payment-quote.lightning.create partner.payment-quote.execute"
        }));
        let access_token = format!("{header}.{payload}.signature");

        assert_eq!(
            parse_strike_oauth_permissions(&access_token),
            Some(vec![
                "create_invoice".to_string(),
                "decode".to_string(),
                "get_info".to_string(),
                "lookup_invoice".to_string(),
                "on_invoice_events".to_string(),
                "pay_invoice".to_string(),
            ])
        );
    }

    #[test]
    fn rejects_opaque_strike_api_keys() {
        assert_eq!(parse_strike_oauth_permissions("sk_test_opaque"), None);
    }

    #[test]
    fn maps_blink_scopes_to_lni_permissions() {
        let header = base64_url_json(&serde_json::json!({ "alg": "none" }));
        let payload = base64_url_json(&serde_json::json!({ "scope": "Read Receive Write" }));
        let token = format!("{header}.{payload}.signature");

        assert_eq!(
            parse_blink_token_permissions(&token),
            Some(vec![
                "create_invoice".to_string(),
                "decode".to_string(),
                "get_info".to_string(),
                "list_transactions".to_string(),
                "lookup_invoice".to_string(),
                "on_invoice_events".to_string(),
                "pay_invoice".to_string(),
            ])
        );
    }

    #[test]
    fn rejects_opaque_blink_api_keys() {
        assert_eq!(parse_blink_token_permissions("blink_opaque"), None);
    }

    fn base64_url_json(value: &serde_json::Value) -> String {
        base64::encode_config(value.to_string(), base64::URL_SAFE).trim_end_matches('=').to_string()
    }
}
