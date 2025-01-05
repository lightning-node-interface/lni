pub mod interface;
pub mod lnd;

pub use lnd::*;
pub use interface::*;

use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn greet(name: String) -> String {
    format!("Hello, {}!", name)
}