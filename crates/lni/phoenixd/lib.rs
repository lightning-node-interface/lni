#[cfg(feature = "napi_rs")]
use napi_derive::napi;

#[cfg_attr(feature = "napi_rs", napi(object))]
pub struct PhoenixdConfig {
    pub url: String,
    pub password: String,
}

#[cfg_attr(feature = "napi_rs", napi(object))]
pub struct PhoenixdNode {
    pub url: String,
    pub password: String,
}

impl PhoenixdNode {
    pub fn new(config: PhoenixdConfig) -> Self {
        Self {
            url: config.url,
            password: config.password,
        }
    }

    pub async fn get_offer(&self) -> crate::Result<String> {
        crate::phoenixd::api::get_offer(self.url.clone(), self.password.clone()).await
    }
}
