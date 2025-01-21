uniffi::setup_scaffolding!("lni_uniffi");
mod api_client;
mod phoenixd;

#[derive(Debug, thiserror::Error)]
#[derive(uniffi::Error)]
#[uniffi(flat_error)]
pub enum ApiError {
    #[error("HttpError: {reason}")]
    Http { reason: String },
    #[error("ApiError: {reason}")]
    Api { reason: String },
    #[error("JsonError: {reason}")]
    Json { reason: String },
}

pub type Result<T> = uniffi::Result<T, ApiError>;

pub use api_client::Fetcher;
pub use lni::phoenixd::lib::PhoenixdNode;
pub use lni::Ip;
