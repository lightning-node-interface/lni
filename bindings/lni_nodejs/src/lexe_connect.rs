use lni::lexe::{
  LexeConnectOptions as CoreLexeConnectOptions, LexeConnectResponse as CoreLexeConnectResponse,
  LexeConnectSession as CoreLexeConnectSession,
};
use napi::bindgen_prelude::Buffer;
use napi_derive::napi;

const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

/// Requester options. With no destination, Lexe's encrypted mailbox is used.
#[napi(object)]
pub struct LexeConnectOptions {
  pub redirect_uri: Option<String>,
  pub post_url: Option<String>,
  pub mailbox_url: Option<String>,
  pub scopes: Option<Vec<String>>,
  pub permissions: Option<Vec<String>>,
  pub account: Option<String>,
  pub metadata: Option<String>,
  pub requester_name: Option<String>,
  pub requester_icon: Option<String>,
  pub label: Option<String>,
  pub expires_at_ms: Option<i64>,
  pub request_ttl_secs: Option<u32>,
}

impl TryFrom<LexeConnectOptions> for CoreLexeConnectOptions {
  type Error = napi::Error;

  fn try_from(value: LexeConnectOptions) -> Result<Self, Self::Error> {
    let expires_at_ms = value
      .expires_at_ms
      .map(|ms| {
        if ms < 0 || ms as u64 > MAX_SAFE_INTEGER {
          Err(napi::Error::from_reason(
            "expiresAtMs must be a non-negative safe integer",
          ))
        } else {
          Ok(ms as u64)
        }
      })
      .transpose()?;
    Ok(Self {
      redirect_uri: value.redirect_uri,
      post_url: value.post_url,
      mailbox_url: value.mailbox_url,
      scopes: value.scopes.unwrap_or_default(),
      permissions: value.permissions.unwrap_or_default(),
      account: value.account,
      metadata: value.metadata,
      requester_name: value.requester_name,
      requester_icon: value.requester_icon,
      label: value.label,
      expires_at_ms,
      request_ttl_secs: value.request_ttl_secs,
    })
  }
}

/// Verified approval or rejection. Never log `clientCredentials`.
#[napi(object)]
pub struct LexeConnectResponse {
  pub client_credentials: Option<String>,
  pub error: Option<String>,
  pub error_message: Option<String>,
  pub scopes: Vec<String>,
  pub permissions: Vec<String>,
  pub expires_at_ms: Option<i64>,
  pub account: Option<String>,
  pub metadata: Option<String>,
}

impl TryFrom<CoreLexeConnectResponse> for LexeConnectResponse {
  type Error = napi::Error;

  fn try_from(value: CoreLexeConnectResponse) -> Result<Self, Self::Error> {
    let expires_at_ms = value
      .expires_at_ms
      .map(|ms| {
        if ms > MAX_SAFE_INTEGER {
          Err(napi::Error::from_reason(
            "LexeConnect expiry exceeds JavaScript's safe integer range",
          ))
        } else {
          Ok(ms as i64)
        }
      })
      .transpose()?;
    Ok(Self {
      client_credentials: value.client_credentials,
      error: value.error,
      error_message: value.error_message,
      scopes: value.scopes,
      permissions: value.permissions,
      expires_at_ms,
      account: value.account,
      metadata: value.metadata,
    })
  }
}

/// One in-memory LexeConnect attempt. Keep this instance until delivery.
#[napi]
pub struct LexeConnectSession {
  inner: CoreLexeConnectSession,
}

#[napi]
impl LexeConnectSession {
  #[napi(constructor)]
  pub fn new(options: LexeConnectOptions) -> napi::Result<Self> {
    let options = options.try_into()?;
    let inner = CoreLexeConnectSession::new(options)
      .map_err(|error| napi::Error::from_reason(error.to_string()))?;
    Ok(Self { inner })
  }

  /// Sensitive URL for a link or QR code; do not log it.
  #[napi]
  pub fn connection_string(&self) -> napi::Result<String> {
    self
      .inner
      .connection_string()
      .map_err(|error| napi::Error::from_reason(error.to_string()))
  }

  #[napi]
  pub fn accept_redirect(&self, url: String) -> napi::Result<LexeConnectResponse> {
    let response = self
      .inner
      .accept_redirect(url)
      .map_err(|error| napi::Error::from_reason(error.to_string()))?;
    response.try_into()
  }

  /// Accept raw encrypted POST or mailbox bytes, not base64 text.
  #[napi]
  pub fn accept_body(&self, body: Buffer) -> napi::Result<LexeConnectResponse> {
    let response = self
      .inner
      .accept_body(body.to_vec())
      .map_err(|error| napi::Error::from_reason(error.to_string()))?;
    response.try_into()
  }

  /// Poll once. `null` means no wallet response yet.
  #[napi]
  pub async fn poll_mailbox(&self) -> napi::Result<Option<LexeConnectResponse>> {
    self
      .inner
      .poll_mailbox()
      .await
      .map_err(|error| napi::Error::from_reason(error.to_string()))?
      .map(TryInto::try_into)
      .transpose()
  }

  #[napi]
  pub fn cancel(&self) -> napi::Result<()> {
    self
      .inner
      .cancel()
      .map_err(|error| napi::Error::from_reason(error.to_string()))
  }
}
