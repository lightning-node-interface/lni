pub mod interface;
pub mod lnd;
pub mod cln;

pub use interface::*;
pub use lnd::*;
pub use cln::*;

use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn greet(name: String) -> String {
    format!("Hello, {}!", name)
}
