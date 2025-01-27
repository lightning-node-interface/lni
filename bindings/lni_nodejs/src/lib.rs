#![deny(clippy::all)]

extern crate napi_derive;

mod phoenixd;

pub use lni::{ApiError, Result};
pub use phoenixd::PhoenixdNode;
pub use lni::types::*;

