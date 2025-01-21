use lni::phoenixd::lib::PhoenixdConfig;
use std::sync::Arc;

#[derive(uniffi::Object)]
pub struct PhoenixdNode {
    inner: PhoenixdConfig,
}

#[uniffi::export]
impl PhoenixdNode {
    #[uniffi::constructor]
    pub fn new(config: PhoenixdConfig) -> Self {
        Self { inner: config }
    }

    pub fn get_url(self: Arc<Self>) -> String {
        self.inner.url.clone()
    }

    pub fn get_password(self: Arc<Self>) -> String {
        self.inner.password.clone()
    }

    pub fn get_config(self: Arc<Self>) -> PhoenixdConfig {
        PhoenixdConfig {
            url: self.inner.url.clone(),
            password: self.inner.password.clone(),
        }
    }

    pub async fn get_offer(self: Arc<Self>) -> crate::Result<String> {
        match lni::phoenixd::api::get_offer(self.inner.url.clone(), self.inner.password.clone()).await {
            Ok(offer) => Ok(offer),
            Err(e) => Err(crate::ApiError::Api { reason: e.to_string() }),
        }
    }
}
