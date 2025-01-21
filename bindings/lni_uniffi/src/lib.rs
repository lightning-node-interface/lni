uniffi::setup_scaffolding!("lni");
mod api_client;
mod phoenixd;

pub use api_client::Fetcher;
pub use lni::phoenixd::lib::PhoenixdNode;
pub use lni::{ApiError, Ip, Result};
