mod api_client;

use lni::phoenixd::{PhoenixdConfig, PhoenixdNode};
use lni::phoenixd::api::{PhoenixService, InfoResponse};

pub use lni::{ApiError, Ip, Result};
pub use api_client::Fetcher;
pub use lni::phoenixd::*;  
pub use lni::types::*;

uniffi::include_scaffolding!("lni");