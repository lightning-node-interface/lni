use lni::phoenixd::lib::PhoenixdConfig;
use std::sync::Arc;

#[derive(uniffi::Object)]
pub struct PhoenixdNode {
    url: String,
    password: String,
}

#[uniffi::export]
impl PhoenixdNode {
    #[uniffi::constructor]
    pub fn new(url: String, password: String) -> Self {
        Self { url, password }
    }

    pub fn get_url(self: Arc<Self>) -> String {
        self.url.clone()
    }

    pub fn get_password(self: Arc<Self>) -> String {
        self.password.clone()
    }

    pub fn get_config(self: Arc<Self>) -> PhoenixdConfig {
        PhoenixdConfig {
            url: self.url.clone(),
            password: self.password.clone(),
        }
    }

    pub async fn get_offer(self: Arc<Self>) -> lni::Result<String> {
        match lni::phoenixd::api::get_offer(self.url.clone(), self.password.clone()).await {
            Ok(offer) => Ok(offer),
            Err(e) => Err(lni::ApiError::Api {
                reason: e.to_string(),
            }),
        }
    }
}
