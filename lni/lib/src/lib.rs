#[cfg(not(target_arch = "wasm32"))]
uniffi::setup_scaffolding!();

pub mod cln;
pub mod interface;
pub mod lnd;

pub use cln::*;
pub use interface::*;
pub use lnd::*;

pub fn welcome(name: String) -> String {
    format!("Welcome {name}, your calendar is ready")
}
