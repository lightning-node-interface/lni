#![deny(clippy::all)]

extern crate napi_derive;

pub use lni::ApiError;
pub use lni::types::*;
pub use lni::types::{Transaction, InvoiceType, ListTransactionsParams, PayInvoiceResponse};

mod phoenixd;
pub use phoenixd::PhoenixdNode;

mod cln;
pub use cln::ClnNode;
