//! Native bridge for the shared LNI LexeConnect requester.

use crate::LexeError;

#[derive(Clone, uniffi::Record)]
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

impl From<LexeConnectOptions> for lni::lexe::LexeConnectOptions {
    fn from(value: LexeConnectOptions) -> Self {
        Self {
            redirect_uri: value.redirect_uri,
            post_url: value.post_url,
            mailbox_url: value.mailbox_url,
            scopes: value.scopes,
            permissions: value.permissions,
            account: value.account,
            metadata: value.metadata,
            requester_name: value.requester_name,
            requester_icon: value.requester_icon,
            label: value.label,
            expires_at_ms: value.expires_at_ms,
            request_ttl_secs: value.request_ttl_secs,
        }
    }
}

#[derive(Clone, uniffi::Record)]
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

impl From<lni::lexe::LexeConnectResponse> for LexeConnectResponse {
    fn from(value: lni::lexe::LexeConnectResponse) -> Self {
        Self {
            client_credentials: value.client_credentials,
            error: value.error,
            error_message: value.error_message,
            scopes: value.scopes,
            permissions: value.permissions,
            expires_at_ms: value.expires_at_ms,
            account: value.account,
            metadata: value.metadata,
        }
    }
}

#[derive(uniffi::Object)]
pub struct LexeConnectSession {
    inner: lni::lexe::LexeConnectSession,
}

#[uniffi::export]
impl LexeConnectSession {
    #[uniffi::constructor]
    pub fn new(options: LexeConnectOptions) -> Result<Self, LexeError> {
        Ok(Self {
            inner: lni::lexe::LexeConnectSession::new(options.into())?,
        })
    }

    pub fn connection_string(&self) -> Result<String, LexeError> {
        Ok(self.inner.connection_string()?)
    }

    pub fn accept_redirect(&self, url: String) -> Result<LexeConnectResponse, LexeError> {
        Ok(self.inner.accept_redirect(url)?.into())
    }

    pub fn accept_body(&self, body: Vec<u8>) -> Result<LexeConnectResponse, LexeError> {
        Ok(self.inner.accept_body(body)?.into())
    }

    pub fn cancel(&self) -> Result<(), LexeError> {
        Ok(self.inner.cancel()?)
    }
}

#[uniffi::export(async_runtime = "tokio")]
impl LexeConnectSession {
    pub async fn poll_mailbox(&self) -> Result<Option<LexeConnectResponse>, LexeError> {
        Ok(self.inner.poll_mailbox().await?.map(Into::into))
    }
}
