use crate::ApiError;
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct ProviderErrorInfo {
    pub code: Option<String>,
    pub status: Option<u16>,
    pub message: Option<String>,
}

pub fn nwc_error(code: &str, message: impl Into<String>) -> ApiError {
    ApiError::Nwc {
        code: code.to_string(),
        message: message.into(),
    }
}

fn string_field(value: &Value, key: &str) -> Option<String> {
    value.get(key)?.as_str().map(ToString::to_string)
}

fn u16_field(value: &Value, key: &str) -> Option<u16> {
    let field = value.get(key)?;
    field
        .as_u64()
        .and_then(|value| u16::try_from(value).ok())
        .or_else(|| field.as_str().and_then(|value| value.parse::<u16>().ok()))
}

pub fn provider_info_from_body(status: Option<u16>, body: &str) -> ProviderErrorInfo {
    let Ok(value) = serde_json::from_str::<Value>(body) else {
        return ProviderErrorInfo {
            code: None,
            status,
            message: if body.is_empty() {
                None
            } else {
                Some(body.to_string())
            },
        };
    };

    let source = value
        .get("error")
        .filter(|error| error.is_object())
        .or_else(|| value.get("data").filter(|data| data.is_object()))
        .unwrap_or(&value);

    ProviderErrorInfo {
        code: string_field(source, "code")
            .or_else(|| string_field(&value, "code"))
            .or_else(|| u16_field(source, "code").map(|code| code.to_string()))
            .or_else(|| u16_field(&value, "code").map(|code| code.to_string())),
        status: u16_field(source, "status")
            .or_else(|| u16_field(&value, "status"))
            .or(status),
        message: string_field(source, "message")
            .or_else(|| string_field(source, "reason"))
            .or_else(|| string_field(&value, "message"))
            .or_else(|| string_field(&value, "reason")),
    }
}

pub fn map_http_status(status: Option<u16>) -> Option<&'static str> {
    match status {
        Some(401) => Some("UNAUTHORIZED"),
        Some(403) => Some("RESTRICTED"),
        Some(404) => Some("NOT_FOUND"),
        Some(429) => Some("RATE_LIMITED"),
        Some(500..=599) => Some("INTERNAL"),
        _ => None,
    }
}

pub fn map_provider_message(message: Option<&str>) -> Option<&'static str> {
    let normalized = message?.to_lowercase();

    if normalized.contains("rate limit") || normalized.contains("too many requests") {
        return Some("RATE_LIMITED");
    }
    if normalized.contains("unauthorized")
        || normalized.contains("unauthenticated")
        || normalized.contains("not authorized")
    {
        return Some("UNAUTHORIZED");
    }
    if normalized.contains("permission denied")
        || normalized.contains("forbidden")
        || normalized.contains("scope")
    {
        return Some("RESTRICTED");
    }
    if normalized.contains("insufficient")
        || normalized.contains("balance too low")
        || normalized.contains("not enough funds")
    {
        return Some("INSUFFICIENT_BALANCE");
    }
    if normalized.contains("quota")
        || normalized.contains("limit exceeded")
        || normalized.contains("amount too high")
    {
        return Some("QUOTA_EXCEEDED");
    }
    if normalized.contains("invalid invoice")
        || normalized.contains("invoice expired")
        || normalized.contains("expired invoice")
        || normalized.contains("no route")
        || normalized.contains("route not found")
        || normalized.contains("payment failed")
        || normalized.contains("recipient")
    {
        return Some("PAYMENT_FAILED");
    }
    if normalized.contains("not found") {
        return Some("NOT_FOUND");
    }

    None
}

pub fn provider_error_from_response<F>(
    provider: &str,
    operation: &str,
    status: reqwest::StatusCode,
    body: String,
    map_provider_code: F,
) -> ApiError
where
    F: Fn(&ProviderErrorInfo) -> Option<&'static str>,
{
    let info = provider_info_from_body(Some(status.as_u16()), &body);
    let code = map_provider_code(&info)
        .or_else(|| map_provider_message(info.message.as_deref()))
        .or_else(|| map_http_status(info.status))
        .unwrap_or("OTHER");
    let message = info.message.unwrap_or_else(|| {
        format!(
            "{} {} request failed: HTTP {} - {}",
            provider, operation, status, body
        )
    });

    nwc_error(code, message)
}

pub fn transport_error(provider: &str, operation: &str, error: reqwest::Error) -> ApiError {
    nwc_error(
        "INTERNAL",
        format!("{} {} request failed: {}", provider, operation, error),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_http_status_fallbacks_to_nwc_codes() {
        let error = provider_error_from_response(
            "speed",
            "get_info",
            reqwest::StatusCode::UNAUTHORIZED,
            "bad key".to_string(),
            |_| None,
        );

        match error {
            ApiError::Nwc { code, message } => {
                assert_eq!(code, "UNAUTHORIZED");
                assert_eq!(message, "bad key");
            }
            other => panic!("expected NWC error, got {:?}", other),
        }
    }

    #[test]
    fn maps_provider_numeric_codes_before_status_fallbacks() {
        let error = provider_error_from_response(
            "cln",
            "pay_invoice",
            reqwest::StatusCode::INTERNAL_SERVER_ERROR,
            r#"{"error":{"code":205,"message":"Unable to find a route"}}"#.to_string(),
            |info| match info
                .code
                .as_deref()
                .and_then(|code| code.parse::<i64>().ok())
            {
                Some(205) => Some("PAYMENT_FAILED"),
                _ => None,
            },
        );

        match error {
            ApiError::Nwc { code, message } => {
                assert_eq!(code, "PAYMENT_FAILED");
                assert_eq!(message, "Unable to find a route");
            }
            other => panic!("expected NWC error, got {:?}", other),
        }
    }

    #[test]
    fn maps_provider_messages_to_insufficient_balance() {
        let error = provider_error_from_response(
            "phoenixd",
            "pay_invoice",
            reqwest::StatusCode::BAD_REQUEST,
            r#"{"message":"Insufficient balance"}"#.to_string(),
            |_| None,
        );

        match error {
            ApiError::Nwc { code, message } => {
                assert_eq!(code, "INSUFFICIENT_BALANCE");
                assert_eq!(message, "Insufficient balance");
            }
            other => panic!("expected NWC error, got {:?}", other),
        }
    }
}
