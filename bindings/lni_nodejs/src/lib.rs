#![deny(clippy::all)]

extern crate napi_derive;

mod api_client;
mod phoenixd;

pub use lni::{ApiError, Ip, Result};
pub use api_client::Fetcher;
pub use phoenixd::PhoenixdNode;