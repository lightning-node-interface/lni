#![allow(clippy::new_without_default)]


pub mod error;
mod api_client;
mod phoenixd;

use crate::error::Result;
pub use api_client::Fetcher;
pub use phoenixd::PhoenixdNode;

pub struct Feenix {}

impl Feenix {
    pub fn new() -> Self {
        Self {}
    }
}

// for use with uniffi decorators (not udl files)
uniffi::setup_scaffolding!("lni_sdk");