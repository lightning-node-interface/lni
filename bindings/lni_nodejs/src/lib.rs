#![deny(clippy::all)]

extern crate napi_derive;

mod phoenixd;

pub use lni::ApiError;
pub use phoenixd::PhoenixdNode;
pub use lni::types::*;

mod database;
pub use database::Db;