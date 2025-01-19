#![deny(clippy::all)]

#[macro_use]
extern crate napi_derive;

use napi::bindgen_prelude::*;
mod api_client;

pub use lni::{ApiError, Ip, Result};
pub use api_client::Fetcher;
