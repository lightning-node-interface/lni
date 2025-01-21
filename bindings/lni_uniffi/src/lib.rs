#![allow(clippy::new_without_default)]


pub mod error;
mod api_client;
mod phoenixd;

use crate::error::Result;
pub use api_client::Fetcher;
pub use lni::phoenixd::lib::PhoenixdNode;
pub use lni::Ip;

uniffi::setup_scaffolding!("lni_sdk");