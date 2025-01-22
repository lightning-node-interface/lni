mod api_client;

use std::str;
use lni::phoenixd::{PhoenixdConfig, PhoenixdNode};


pub use lni::{ApiError, Ip, Result};
pub use api_client::Fetcher;
pub use lni::phoenixd::*;  

uniffi::include_scaffolding!("lni");