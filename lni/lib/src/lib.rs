// src/lib.rs

// Re-export or declare modules
mod types;
mod lightning_node_interface;

// Always include the core logic module
mod lnd {
    pub mod lnd; // core logic

    // Only include these if the features are enabled
    #[cfg(feature = "uniffi")]
    pub mod lnd_uniffi;

    #[cfg(feature = "wasm")]
    pub mod lnd_wasm;
}

// (Optional) re-export things if you want them public at top-level
pub use types::*;
pub use lightning_node_interface::*;

// Conditional re-exports based on features
#[cfg(feature = "wasm")]
pub use lnd::lnd_wasm::WasmLndNode as LndNode;

#[cfg(feature = "uniffi")]
pub use lnd::lnd::LndNode;

// Optionally, you could define feature conflicts:
#[cfg(all(feature = "wasm", feature = "uniffi"))]
compile_error!("Please enable only one feature: 'wasm' or 'uniffi', not both.");
