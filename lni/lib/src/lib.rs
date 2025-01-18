// You must call this once
uniffi::setup_scaffolding!("lni");

mod types;
mod lightning_node_interface;
mod lnd {
    pub mod lnd; // Declare the lnd module
    pub mod lnd_api; // Declare the lnd_api module if needed
}
mod phoenixd {
    pub mod phoenixd; 
    pub mod phoenixd_api;
}
mod nwc {
    pub mod nwc; 
    pub mod nwc_api;
}

// Re-export or declare modules
pub use types::NodeConfig;
pub use types::Transaction;
pub use lightning_node_interface::LightningNodeInterface;
pub use crate::lnd::lnd::LndNode;
// pub use crate::nwc::nwc::NwcNode;


mod api_client;
mod tasks;
mod test_data;

pub use api_client::{ApiClient, HttpClient, Issue, IssueState, Ip, Fetcher};
pub use tasks::{run_task, RustTask, TaskRunner};
pub use test_data::test_response_data;

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("HttpError: {reason}")]
    Http { reason: String },
    #[error("ApiError: {reason}")]
    Api { reason: String },
    #[error("JsonError: {reason}")]
    Json { reason: String },
}

pub type Result<T> = std::result::Result<T, ApiError>;
