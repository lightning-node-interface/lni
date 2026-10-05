//! Bounded NWA response discovery on fixed Alby relays through the pinned dial gate.
use crate::ApiError;
use nwc::prelude::*;
use std::time::Duration;
const RELAYS: [&str; 2] = ["wss://relay.getalby.com/v1", "wss://relay2.getalby.com/v1"];
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
#[derive(Clone)]
pub struct AuthorizationResponse {
    pub wallet_public_key: String,
    pub relays: Vec<String>,
}
fn invalid() -> ApiError {
    ApiError::InvalidInput("Invalid wallet authorization".into())
}
fn admit(
    event: &Event,
    app: &PublicKey,
    created_at: u64,
    now: u64,
) -> Option<AuthorizationResponse> {
    if event.kind != Kind::Custom(13194)
        || event.created_at.as_u64() < created_at.saturating_sub(30)
        || event.created_at.as_u64() > now.saturating_add(30)
        || event.verify().is_err()
        || event.content.len() > 8192
        || event.tags.len() > 32
    {
        return None;
    }
    let tags: Vec<_> = event
        .tags
        .iter()
        .filter(|tag| tag.as_slice().first().is_some_and(|t| t == "p"))
        .collect();
    if tags.len() != 1
        || tags[0].as_slice().get(1).map(String::as_str) != Some(app.to_hex().as_str())
    {
        return None;
    }
    // Fixed relays are safe fallback; recommended endpoints still pass the MDK
    // policy before use and the same pinned dial gate when connecting the wallet.
    let relays = tags[0]
        .as_slice()
        .get(2)
        .filter(|r| !r.is_empty())
        .map(|r| vec![r.clone()])
        .unwrap_or_else(|| RELAYS.iter().map(|s| s.to_string()).collect());
    Some(AuthorizationResponse {
        wallet_public_key: event.pubkey.to_hex(),
        relays,
    })
}
#[cfg_attr(feature = "uniffi", uniffi::export(async_runtime = "tokio"))]
pub async fn poll_authorization(
    app_public_key: String,
    created_at: u64,
    now: u64,
) -> Result<Option<AuthorizationResponse>, ApiError> {
    if now < created_at || now >= created_at.saturating_add(600) {
        return Err(invalid());
    }
    let app = PublicKey::from_hex(&app_public_key).map_err(|_| invalid())?;
    let _proxy = super::pinned_proxy::PinnedProxy::start(RELAYS.iter())
        .await
        .map_err(|_| ApiError::NetworkError("Authorization relay unavailable".into()))?;
    let pool = RelayPool::new();
    for relay in RELAYS {
        pool.add_relay(
            relay,
            RelayOptions::default()
                .connection_mode(ConnectionMode::Proxy(_proxy.address))
                .verify_subscriptions(true)
                .ban_relay_on_mismatch(true),
        )
        .await
        .map_err(|_| invalid())?;
    }
    pool.connect().await;
    let filter = Filter::new()
        .kind(Kind::Custom(13194))
        .pubkey(app)
        .since(Timestamp::from(created_at.saturating_sub(30)))
        .limit(16);
    let result = pool
        .fetch_events(
            filter,
            Duration::from_secs(5),
            ReqExitPolicy::WaitDurationAfterEOSE(Duration::from_secs(1)),
        )
        .await;
    pool.shutdown().await;
    match result {
        Ok(events) => Ok(events.iter().find_map(|e| admit(e, &app, created_at, now))),
        Err(_) => Err(ApiError::NetworkError(
            "Authorization response unavailable".into(),
        )),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn signed_response_requires_exact_recipient_and_time_window() {
        let app = Keys::generate();
        let wallet = Keys::generate();
        let event = EventBuilder::new(Kind::Custom(13194), "")
            .tag(Tag::public_key(app.public_key()))
            .custom_created_at(Timestamp::from(100))
            .sign_with_keys(&wallet)
            .unwrap();
        assert!(admit(&event, &app.public_key(), 100, 101).is_some());
        assert!(admit(&event, &Keys::generate().public_key(), 100, 101).is_none());
        assert!(admit(&event, &app.public_key(), 200, 201).is_none());
        let mut tampered = event;
        tampered.content = "tampered".into();
        assert!(admit(&tampered, &app.public_key(), 100, 101).is_none());
    }
}
