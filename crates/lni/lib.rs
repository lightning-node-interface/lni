#[cfg(feature = "napi_rs")]
use napi_derive::napi;
#[cfg(feature = "napi_rs")]
use napi::bindgen_prelude::*;

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("HttpError: {reason}")]
    Http { reason: String },
    #[error("ApiError: {reason}")]
    Api { reason: String },
    #[error("JsonError: {reason}")]
    Json { reason: String },
}
impl From<serde_json::Error> for ApiError {
    fn from(e: serde_json::Error) -> Self {
        Self::Json {
            reason: e.to_string(),
        }
    }
}
pub type Result<T> = std::result::Result<T, ApiError>;

pub mod phoenixd {
    pub mod lib;
    pub mod api;
    pub use lib::{PhoenixdConfig, PhoenixdNode, PhoenixdMakeInvoiceParams};
}

pub mod types;
pub use types::*;