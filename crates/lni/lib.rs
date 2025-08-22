#[cfg(feature = "napi_rs")]
use napi_derive::napi;
#[cfg(feature = "napi_rs")]
use napi::bindgen_prelude::*;

#[cfg_attr(feature = "uniffi", derive(uniffi::Object))]
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

pub mod phoenixd {
    pub mod api;
    pub mod lib;
    pub mod types;
    pub use lib::{PhoenixdConfig, PhoenixdNode};
}

pub mod cln {
    pub mod api;
    pub mod lib;
    pub mod types;
    pub use lib::{ClnConfig, ClnNode};
}

pub mod lnd {
    pub mod api;
    pub mod lib;
    pub mod types;
    pub use lib::{LndConfig, LndNode};
}

pub mod nwc {
    pub mod api;
    pub mod lib;
    pub mod types;
    pub use lib::{NwcConfig, NwcNode};
}

pub mod strike {
    pub mod api;
    pub mod lib;
    pub mod types;
    pub use lib::{StrikeConfig, StrikeNode};
}

pub mod blink {
    pub mod api;
    pub mod lib;
    pub mod types;
    pub use lib::{BlinkConfig, BlinkNode};
}

pub mod types;
pub use types::*;

pub mod utils;
pub use utils::*;

pub mod database;
pub use database::{Db, DbError, Payment};

#[cfg(feature = "uniffi")]
uniffi::setup_scaffolding!();
