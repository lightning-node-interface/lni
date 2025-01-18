// You must call this once
uniffi::setup_scaffolding!("types");

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
pub use types::*;
pub use lightning_node_interface::LightningNodeInterface;
pub use crate::lnd::lnd::LndNode;
// pub use crate::nwc::nwc::NwcNode;
