uniffi::setup_scaffolding!("liblni");
mod api_client;
mod phoenixd;

pub use api_client::Fetcher;
pub use lni::phoenixd::lib::PhoenixdNode;
pub use lni::Ip;
