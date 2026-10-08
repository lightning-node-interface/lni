//! Requester-side LexeConnect helpers shared with the React Native binding.
//!
//! A session holds a fresh HPKE key in native memory. Open its connection string
//! in Lexe, then accept a redirect/POST body or poll the encrypted mailbox. Each
//! session accepts exactly one verified response and expires after five minutes
//! by default. Connection strings and returned credentials must not be logged.

use std::{
    sync::Mutex,
    time::{Duration, Instant},
};

use lexe_connect::{
    request::{CredentialRequestParams, Delivery},
    requester::PendingRequest,
    response::{CredentialResponse, CredentialResult},
};

use crate::ApiError;

const MAX_RESPONSE_BYTES: usize = 64 * 1024;
const MAX_REQUEST_TTL_SECS: u32 = 300;

/// Exactly one delivery destination may be set. With none, use Lexe's mailbox.
/// Responses are always encrypted, including POST delivery.
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
#[derive(Clone, Default)]
pub struct LexeConnectOptions {
    pub redirect_uri: Option<String>,
    pub post_url: Option<String>,
    pub mailbox_url: Option<String>,
    pub scopes: Vec<String>,
    pub permissions: Vec<String>,
    pub account: Option<String>,
    pub metadata: Option<String>,
    pub requester_name: Option<String>,
    pub requester_icon: Option<String>,
    pub label: Option<String>,
    /// Suggested credential expiry, in Unix milliseconds; editable in Lexe.
    pub expires_at_ms: Option<u64>,
    /// Local request lifetime, 1–300 seconds. Defaults to 300.
    pub request_ttl_secs: Option<u32>,
}

/// Verified approval or rejection. Exactly one of `client_credentials` and
/// `error` is populated. `error` is `user_rejected` or `other`.
/// Credentials can be passed directly to `LexeConfig::client_credentials`.
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
#[derive(Clone)]
pub struct LexeConnectResponse {
    pub client_credentials: Option<String>,
    pub error: Option<String>,
    pub error_message: Option<String>,
    pub scopes: Vec<String>,
    pub permissions: Vec<String>,
    pub expires_at_ms: Option<u64>,
    pub account: Option<String>,
    pub metadata: Option<String>,
}

impl std::fmt::Debug for LexeConnectResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LexeConnectResponse")
            .field("approved", &self.client_credentials.is_some())
            .field("error", &self.error)
            .finish_non_exhaustive()
    }
}

impl From<CredentialResponse> for LexeConnectResponse {
    fn from(response: CredentialResponse) -> Self {
        let mut result = Self {
            client_credentials: None,
            error: None,
            error_message: None,
            scopes: Vec::new(),
            permissions: Vec::new(),
            expires_at_ms: None,
            account: response.account,
            metadata: response.metadata,
        };
        match response.result {
            CredentialResult::Granted(grant) => {
                result.client_credentials = Some(grant.credential);
                result.scopes = grant.scopes.into_iter().collect();
                result.permissions = grant.permissions.into_iter().collect();
                result.expires_at_ms = grant.expires_at.map(|time| time.to_u64());
            }
            CredentialResult::Error(error) => {
                result.error = Some(error.code.to_string());
                result.error_message = error.message;
            }
        }
        result
    }
}

enum State {
    Pending(PendingRequest),
    Completed,
    Cancelled,
    Expired,
}

/// In-memory, single-use connection attempt. Drop or cancel it when leaving the
/// connection screen. No private key or one-time-secret export is provided.
#[cfg_attr(feature = "uniffi", derive(uniffi::Object))]
pub struct LexeConnectSession {
    state: Mutex<State>,
    deadline: Instant,
    http: reqwest::Client,
}

impl std::fmt::Debug for LexeConnectSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LexeConnectSession").finish_non_exhaustive()
    }
}

fn invalid(message: &str) -> ApiError {
    ApiError::InvalidInput(message.to_owned())
}

fn transport() -> ApiError {
    // reqwest errors may contain the private mailbox address; never forward them.
    ApiError::Api {
        reason: "LexeConnect mailbox request failed".to_owned(),
    }
}

impl LexeConnectSession {
    fn with_pending<T>(
        &self,
        f: impl FnOnce(&PendingRequest) -> Result<T, ApiError>,
    ) -> Result<T, ApiError> {
        let mut state = self.lock()?;
        f(self.pending(&mut state)?)
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, State>, ApiError> {
        self.state
            .lock()
            .map_err(|_| invalid("LexeConnect session unavailable"))
    }

    fn pending<'a>(&self, state: &'a mut State) -> Result<&'a PendingRequest, ApiError> {
        if matches!(state, State::Pending(_)) && Instant::now() >= self.deadline {
            *state = State::Expired;
        }
        match state {
            State::Pending(request) => Ok(request),
            State::Completed => Err(invalid("LexeConnect session already completed")),
            State::Cancelled => Err(invalid("LexeConnect session cancelled")),
            State::Expired => Err(invalid("LexeConnect session expired")),
        }
    }

    fn accept(
        &self,
        f: impl FnOnce(&PendingRequest) -> Result<CredentialResponse, ApiError>,
    ) -> Result<LexeConnectResponse, ApiError> {
        let mut state = self.lock()?;
        let request = self.pending(&mut state)?;
        let response = f(request)?;
        // The protocol verifies account, secret and grants. Also require the
        // optional metadata echo, and reject empty credentials without consuming.
        if response.metadata != request.request().params.metadata
            || matches!(&response.result, CredentialResult::Granted(grant) if grant.credential.is_empty())
        {
            return Err(invalid("Invalid LexeConnect response"));
        }
        *state = State::Completed;
        Ok(response.into())
    }
}

#[cfg_attr(feature = "uniffi", uniffi::export)]
impl LexeConnectSession {
    #[cfg_attr(feature = "uniffi", uniffi::constructor)]
    pub fn new(options: LexeConnectOptions) -> Result<Self, ApiError> {
        let ttl = options.request_ttl_secs.unwrap_or(MAX_REQUEST_TTL_SECS);
        if ttl == 0 || ttl > MAX_REQUEST_TTL_SECS {
            return Err(invalid(
                "LexeConnect request_ttl_secs must be between 1 and 300",
            ));
        }
        let delivery = match (options.redirect_uri, options.post_url, options.mailbox_url) {
            (Some(uri), None, None) => Delivery::Redirect(uri),
            (None, Some(url), None) => Delivery::Post(url),
            (None, None, Some(url)) => Delivery::Mailbox(url),
            (None, None, None) => Delivery::Mailbox(lexe_connect::LEXE_MAILBOX_URL.to_owned()),
            _ => return Err(invalid("Choose only one LexeConnect delivery destination")),
        };
        let (destination, reserved) = match &delivery {
            Delivery::Redirect(uri) => (uri, Some("response")),
            Delivery::Mailbox(url) => (url, Some("address")),
            Delivery::Post(url) => (url, None),
        };
        let destination = reqwest::Url::parse(destination)
            .map_err(|_| invalid("Invalid LexeConnect delivery destination"))?;
        if destination.fragment().is_some()
            || !destination.username().is_empty()
            || destination.password().is_some()
            || destination
                .query_pairs()
                .any(|(key, _)| Some(key.as_ref()) == reserved)
        {
            return Err(invalid("Invalid LexeConnect delivery destination"));
        }
        // Commas delimit grants in a connection string. Reject ambiguous names
        // before rendering, rather than requesting a different set of grants.
        if options
            .scopes
            .iter()
            .chain(&options.permissions)
            .any(|value| value.is_empty() || value.contains(',') || value.trim() != value)
        {
            return Err(invalid("Invalid LexeConnect scope or permission name"));
        }
        let expires_at = options
            .expires_at_ms
            .map(|ms| ms.to_string().parse())
            .transpose()
            .map_err(|_| invalid("Invalid LexeConnect credential expiration"))?;
        let params = CredentialRequestParams {
            delivery,
            account: options.account,
            metadata: options.metadata,
            requester_name: options.requester_name,
            requester_icon: options.requester_icon,
            scopes: options.scopes.into_iter().collect(),
            permissions: options.permissions.into_iter().collect(),
            label: options.label,
            expires_at,
        };
        let request = PendingRequest::new(&mut rand::rngs::OsRng, params)
            .map_err(|_| invalid("Invalid LexeConnect request options"))?;
        let http = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(15))
            .build()
            .map_err(|_| transport())?;
        Ok(Self {
            state: Mutex::new(State::Pending(request)),
            deadline: Instant::now() + Duration::from_secs(u64::from(ttl)),
            http,
        })
    }

    /// Sensitive URL to open with the OS or render as a QR code. Never log it.
    pub fn connection_string(&self) -> Result<String, ApiError> {
        self.with_pending(|request| Ok(request.connection_string()))
    }

    /// Verify a wallet callback URL. An invalid response leaves the session live.
    pub fn accept_redirect(&self, url: String) -> Result<LexeConnectResponse, ApiError> {
        if url.len() > MAX_RESPONSE_BYTES * 2 {
            return Err(invalid("LexeConnect response too large"));
        }
        self.accept(|request| {
            let Delivery::Redirect(expected) = &request.request().params.delivery else {
                return Err(invalid(
                    "LexeConnect session does not use redirect delivery",
                ));
            };
            let mut actual =
                reqwest::Url::parse(&url).map_err(|_| invalid("Invalid LexeConnect callback"))?;
            let expected = reqwest::Url::parse(expected)
                .map_err(|_| invalid("Invalid LexeConnect callback"))?;
            let pairs: Vec<(String, String)> = actual
                .query_pairs()
                .map(|(k, v)| (k.into_owned(), v.into_owned()))
                .collect();
            if pairs.iter().filter(|(key, _)| key == "response").count() != 1
                || actual.fragment().is_some()
            {
                return Err(invalid("Invalid LexeConnect callback"));
            }
            actual.set_query(None);
            let remaining: Vec<_> = pairs.iter().filter(|(key, _)| key != "response").collect();
            if !remaining.is_empty() {
                actual
                    .query_pairs_mut()
                    .extend_pairs(remaining.into_iter().map(|(k, v)| (k, v)));
            }
            // Compare parsed URLs; existing query parameters must be preserved.
            let mut expected_base = expected.clone();
            expected_base.set_query(None);
            let mut actual_base = actual.clone();
            actual_base.set_query(None);
            if actual_base != expected_base
                || actual.query_pairs().collect::<Vec<_>>()
                    != expected.query_pairs().collect::<Vec<_>>()
            {
                return Err(invalid("LexeConnect callback destination mismatch"));
            }
            request
                .accept_redirect(&url)
                .map_err(|_| invalid("Invalid LexeConnect response"))
        })
    }

    /// Verify the encrypted raw POST body or mailbox blob (not base64 text).
    pub fn accept_body(&self, body: Vec<u8>) -> Result<LexeConnectResponse, ApiError> {
        if body.len() > MAX_RESPONSE_BYTES {
            return Err(invalid("LexeConnect response too large"));
        }
        self.accept(|request| {
            request
                .accept_body(&body)
                .map_err(|_| invalid("Invalid LexeConnect response"))
        })
    }

    /// Discard request secrets. Safe to call more than once.
    pub fn cancel(&self) -> Result<(), ApiError> {
        let mut state = self.lock()?;
        if matches!(*state, State::Pending(_)) {
            *state = State::Cancelled;
        }
        Ok(())
    }
}

#[cfg_attr(feature = "uniffi", uniffi::export(async_runtime = "tokio"))]
impl LexeConnectSession {
    /// Poll once: `None` means the wallet has not responded (HTTP 404).
    /// Transport failures leave the request live for retry. No redirects followed.
    pub async fn poll_mailbox(&self) -> Result<Option<LexeConnectResponse>, ApiError> {
        let (url, address) = self.with_pending(|request| {
            request
                .mailbox()
                .map(|(url, address)| (url.to_owned(), address.to_string()))
                .ok_or_else(|| invalid("LexeConnect session does not use mailbox delivery"))
        })?;
        let remaining = self.deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return self.with_pending(|_| Err(invalid("LexeConnect session expired")));
        }
        let mut response = self
            .http
            .get(url)
            .query(&[("address", address)])
            .timeout(remaining.min(Duration::from_secs(15)))
            .send()
            .await
            .map_err(|_| transport())?;
        // Cancellation/expiration while the HTTP request was running wins.
        self.with_pending(|_| Ok(()))?;
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if response.status() != reqwest::StatusCode::OK {
            return Err(transport());
        }
        if response
            .content_length()
            .is_some_and(|length| length > MAX_RESPONSE_BYTES as u64)
        {
            return Err(invalid("LexeConnect response too large"));
        }
        let mut body = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|_| transport())? {
            if body.len() + chunk.len() > MAX_RESPONSE_BYTES {
                return Err(invalid("LexeConnect response too large"));
            }
            body.extend_from_slice(&chunk);
        }
        self.accept_body(body).map(Some)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lexe_connect::{
        request::CredentialRequest,
        response::Grant,
        wallet::{DeliveryAction, Outcome},
    };

    fn options() -> LexeConnectOptions {
        LexeConnectOptions {
            scopes: vec!["read_info".into(), "receive".into()],
            permissions: vec!["cancel_payment".into()],
            account: Some("@example".into()),
            metadata: Some("example-state".into()),
            ..Default::default()
        }
    }

    fn request(session: &LexeConnectSession) -> CredentialRequest {
        CredentialRequest::parse(&session.connection_string().unwrap()).unwrap()
    }

    fn approval() -> Outcome {
        Outcome::Approved {
            credential: "test-only-credential".into(),
            expires_at: Some("1821484800000".parse().unwrap()),
        }
    }

    fn delivered_body(session: &LexeConnectSession, outcome: Outcome) -> Vec<u8> {
        match request(session)
            .respond(&mut rand::rngs::OsRng, outcome)
            .unwrap()
        {
            DeliveryAction::Http(http) => {
                assert_eq!(http.content_type, "application/octet-stream");
                http.body
            }
            _ => panic!("expected HTTP delivery"),
        }
    }

    #[test]
    fn encrypted_mailbox_round_trip_is_single_use() {
        let session = LexeConnectSession::new(options()).unwrap();
        assert!(
            matches!(request(&session).params.delivery, Delivery::Mailbox(url) if url == lexe_connect::LEXE_MAILBOX_URL)
        );
        let body = delivered_body(&session, approval());
        assert!(!body
            .windows(b"test-only-credential".len())
            .any(|w| w == b"test-only-credential"));
        let response = session.accept_body(body.clone()).unwrap();
        assert!(response.client_credentials.as_deref() == Some("test-only-credential"));
        assert_eq!(response.scopes, vec!["read_info", "receive"]);
        assert_eq!(response.permissions, vec!["cancel_payment"]);
        assert_eq!(response.expires_at_ms, Some(1821484800000));
        assert_eq!(response.account.as_deref(), Some("@example"));
        assert_eq!(response.metadata.as_deref(), Some("example-state"));
        assert!(response.error.is_none());
        assert!(session.accept_body(body).is_err());
        assert!(session.connection_string().is_err());
    }

    #[test]
    fn redirect_round_trip_preserves_query_and_validates_destination() {
        for uri in [
            "https://example.com/connect?state=a%20b",
            "exampleapp://connect?state=a%20b",
            "exampleapp:/connect",
        ] {
            let session = LexeConnectSession::new(LexeConnectOptions {
                redirect_uri: Some(uri.into()),
                ..options()
            })
            .unwrap();
            let DeliveryAction::Redirect(url) = request(&session)
                .respond(&mut rand::rngs::OsRng, approval())
                .unwrap()
            else {
                panic!("expected redirect")
            };
            assert!(session
                .accept_redirect(format!("{url}&response=duplicate"))
                .is_err());
            assert!(session
                .accept_redirect(url.replace("connect", "wrong"))
                .is_err());
            assert!(session
                .accept_redirect(url)
                .unwrap()
                .client_credentials
                .is_some());
        }
    }

    #[test]
    fn encrypted_post_rejection_and_failure_are_terminal() {
        for outcome in [Outcome::Rejected, Outcome::Failed("test-only-error".into())] {
            let session = LexeConnectSession::new(LexeConnectOptions {
                post_url: Some("https://example.com/connect".into()),
                ..options()
            })
            .unwrap();
            let body = delivered_body(&session, outcome);
            let response = session.accept_body(body.clone()).unwrap();
            assert!(response.client_credentials.is_none());
            assert!(matches!(
                response.error.as_deref(),
                Some("user_rejected" | "other")
            ));
            assert!(session.accept_body(body).is_err());
        }
    }

    #[test]
    fn invalid_or_foreign_responses_do_not_consume_request() {
        let session = LexeConnectSession::new(options()).unwrap();
        let foreign = LexeConnectSession::new(options()).unwrap();
        assert!(session
            .accept_body(delivered_body(&foreign, approval()))
            .is_err());
        let body = delivered_body(&session, approval());
        let mut tampered = body.clone();
        tampered[35] ^= 1;
        assert!(session.accept_body(tampered).is_err());
        assert!(session
            .accept_body(b"{\"credential\":\"plaintext\"}".to_vec())
            .is_err());
        assert!(session
            .accept_body(vec![0; MAX_RESPONSE_BYTES + 1])
            .is_err());
        assert!(session.accept_body(body).is_ok());
    }

    #[test]
    fn verifies_secret_account_metadata_and_exact_grants() {
        let session = LexeConnectSession::new(options()).unwrap();
        let request = request(&session);
        let response = CredentialResponse {
            result: CredentialResult::Granted(Grant {
                credential: "test-only-credential".into(),
                scopes: request.params.scopes.clone(),
                permissions: request.params.permissions.clone(),
                expires_at: None,
            }),
            one_time_secret: request.one_time_secret,
            account: request.params.account.clone(),
            metadata: request.params.metadata.clone(),
        };
        for index in 0..6 {
            let mut altered = response.clone();
            match index {
                0 => altered.one_time_secret = "00000000000000000000000000000000".parse().unwrap(),
                1 => altered.account = Some("@different".into()),
                2 => altered.metadata = None,
                _ => {
                    if let CredentialResult::Granted(grant) = &mut altered.result {
                        match index {
                            3 => {
                                grant.scopes.insert("spend".into());
                            }
                            4 => grant.permissions.clear(),
                            _ => grant.credential.clear(),
                        }
                    }
                }
            }
            let body = altered.seal(&mut rand::rngs::OsRng, &request).unwrap().0;
            assert!(session.accept_body(body).is_err());
        }
        assert!(session
            .accept_body(response.seal(&mut rand::rngs::OsRng, &request).unwrap().0)
            .is_ok());
    }

    #[test]
    fn cancellation_and_expiration_release_request_and_reject_response() {
        let session = LexeConnectSession::new(options()).unwrap();
        let body = delivered_body(&session, approval());
        session.cancel().unwrap();
        session.cancel().unwrap();
        assert!(session.accept_body(body).is_err());
        assert!(matches!(*session.lock().unwrap(), State::Cancelled));
        let mut session = LexeConnectSession::new(options()).unwrap();
        let body = delivered_body(&session, approval());
        session.deadline = Instant::now();
        assert!(session.accept_body(body).is_err());
        assert!(matches!(*session.lock().unwrap(), State::Expired));
    }

    #[test]
    fn rejects_invalid_options_and_redacts_debug() {
        for options in [
            LexeConnectOptions::default(),
            LexeConnectOptions {
                request_ttl_secs: Some(0),
                ..options()
            },
            LexeConnectOptions {
                request_ttl_secs: Some(301),
                ..options()
            },
            LexeConnectOptions {
                post_url: Some("http://example.com".into()),
                ..options()
            },
            LexeConnectOptions {
                mailbox_url: Some("http://example.com".into()),
                ..options()
            },
            LexeConnectOptions {
                post_url: Some("https://example.com".into()),
                redirect_uri: Some("exampleapp://connect".into()),
                ..options()
            },
            LexeConnectOptions {
                scopes: vec!["read_info,spend".into()],
                ..options()
            },
            LexeConnectOptions {
                account: Some("x".repeat(65)),
                ..options()
            },
            LexeConnectOptions {
                metadata: Some("x".repeat(1025)),
                ..options()
            },
            LexeConnectOptions {
                expires_at_ms: Some(u64::MAX),
                ..options()
            },
        ] {
            assert!(LexeConnectSession::new(options).is_err());
        }
        let session = LexeConnectSession::new(options()).unwrap();
        assert_eq!(format!("{session:?}"), "LexeConnectSession { .. }");
        let response = session
            .accept_body(delivered_body(&session, approval()))
            .unwrap();
        let debug = format!("{response:?}");
        assert!(!debug.contains("test-only-credential"));
        assert!(!debug.contains("@example"));
        assert!(!debug.contains("example-state"));
    }

    #[test]
    fn concurrent_responses_only_one_wins() {
        let session = std::sync::Arc::new(LexeConnectSession::new(options()).unwrap());
        let body = delivered_body(&session, approval());
        let threads: Vec<_> = (0..4)
            .map(|_| {
                let session = session.clone();
                let body = body.clone();
                std::thread::spawn(move || session.accept_body(body).is_ok())
            })
            .collect();
        assert_eq!(
            threads
                .into_iter()
                .filter_map(|t| t.join().unwrap().then_some(()))
                .count(),
            1
        );
    }

    /// Exercises the actual public relay with an encrypted, synthetic rejection.
    /// No wallet credentials, user account, or payment is involved.
    #[tokio::test]
    #[ignore = "contacts the public Lexe mailbox"]
    async fn public_mailbox_round_trip() {
        let session = LexeConnectSession::new(LexeConnectOptions {
            scopes: vec!["read_info".into()],
            ..Default::default()
        })
        .unwrap();
        assert!(session.poll_mailbox().await.unwrap().is_none());
        let DeliveryAction::Http(delivery) = request(&session)
            .respond(&mut rand::rngs::OsRng, Outcome::Rejected)
            .unwrap()
        else {
            panic!("expected mailbox delivery")
        };
        let response = session
            .http
            .post(delivery.url)
            .header(reqwest::header::CONTENT_TYPE, delivery.content_type)
            .body(delivery.body)
            .send()
            .await
            .map_err(|_| transport())
            .unwrap();
        assert_eq!(response.status(), reqwest::StatusCode::OK);
        let response = session.poll_mailbox().await.unwrap().unwrap();
        assert_eq!(response.error.as_deref(), Some("user_rejected"));
        assert!(session.poll_mailbox().await.is_err());
    }
}
