use std::sync::Arc;
// You must call this once
uniffi::setup_scaffolding!();

mod types;
mod lightning_node_interface;
mod lnd;
mod lnd_api;

// Re-export or declare modules
pub use types::*;
pub use crate::lnd::LndNode; // Re-export LndNode
pub use lightning_node_interface::LightningNodeInterface; // Re-export LightningNodeInterface