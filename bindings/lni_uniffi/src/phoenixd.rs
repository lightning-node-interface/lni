use lni::{phoenixd::lib::PhoenixdConfig, ApiError};
use std::sync::Arc;

#[derive(uniffi::Object)]
struct PhoenixdNode {
    url: String,
    password: String,
}

#[uniffi::export(async_runtime = "tokio")]
impl PhoenixdNode {
    #[uniffi::constructor]
    pub fn new(url: String, password: String) -> Arc<Self> {
        Arc::new(Self { url, password })
    }

    pub fn get_url(&self) -> String {
        self.url.clone()
    }

    pub fn get_password(&self) -> String {
        self.password.clone()
    }

    pub fn get_config(&self) -> PhoenixdConfig {
        PhoenixdConfig {
            url: self.url.clone(),
            password: self.password.clone(),
        }
    }

    pub async fn get_offer(&self) -> Result<String, ApiError> {
        lni::phoenixd::api::get_offer(self.url.clone(), self.password.clone())
            .await
            .map_err(ApiError::from)
    }
}
