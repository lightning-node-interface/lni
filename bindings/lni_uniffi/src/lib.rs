mod api_client;

use std::str;
use lni::phoenixd::{PhoenixdConfig, PhoenixdNode};
use lni::phoenixd::api::{PhoenixService, InfoResponse};
use std::Error;

pub use lni::{ApiError, Ip, Result};
pub use api_client::Fetcher;
pub use lni::phoenixd::*;  

uniffi::include_scaffolding!("lni");