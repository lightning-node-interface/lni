use reqwest;
#[cfg(feature = "napi_rs")]
use napi_derive::napi;
#[cfg(feature = "napi_rs")]
use napi::bindgen_prelude::*;
#[cfg(feature = "uniffi_rs")]
uniffi::setup_scaffolding!();

#[derive(Debug, thiserror::Error)]
#[cfg_attr(feature = "uniffi_rs", derive(uniffi::Error))]
pub enum ApiError {
    #[error("HttpError: {reason}")]
    Http { reason: String },
    #[error("ApiError: {reason}")]
    Api { reason: String },
    #[error("JsonError: {reason}")]
    Json { reason: String },
}
impl From<serde_json::Error> for ApiError {
    fn from(e: serde_json::Error) -> Self {
        Self::Json {
            reason: e.to_string(),
        }
    }
}

pub type Result<T> = std::result::Result<T, ApiError>;

#[derive(Debug, serde::Deserialize, serde::Serialize)]
#[cfg_attr(feature = "napi_rs", napi(object))]
pub struct Ip {
    pub origin: String,
}

pub async fn get_ip_address() -> Result<Ip> {
    let client: reqwest::blocking::Client = reqwest::blocking::Client::new();
    let response: reqwest::blocking::Response = client.get("https://httpbin.org/ip").send().unwrap();
    let resp_text = response.text().unwrap();
    let ip_address_response: Ip = serde_json::from_str(&resp_text)?;
    Ok(ip_address_response)
}


pub mod phoenixd {
    pub mod lib;
    pub mod api;
}