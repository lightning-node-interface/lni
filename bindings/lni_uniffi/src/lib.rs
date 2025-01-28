use lni::phoenixd::{PhoenixdConfig, PhoenixdNode, PhoenixdMakeInvoiceParams};
use lni::phoenixd::api::{InfoResponse};

pub use lni::{ApiError, Result};
pub use lni::phoenixd::*;  
pub use lni::types::*;

uniffi::include_scaffolding!("lni");