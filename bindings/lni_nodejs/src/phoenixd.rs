use lni::phoenixd::lib::PhoenixdConfig;
use napi_derive::napi;
use lni::phoenixd::api::PhoenixService;

#[napi]
pub struct PhoenixdNode {
  inner: PhoenixdConfig,
}

#[napi]
impl PhoenixdNode {
  #[napi(constructor)]
  pub fn new(config: PhoenixdConfig) -> Self {
    Self { inner: config }
  }

  #[napi]
  pub fn get_url(&self) -> String {
    self.inner.url.clone()
  }

  #[napi]
  pub fn get_password(&self) -> String {
    self.inner.password.clone()
  }

  #[napi]
  pub fn get_config(&self) -> PhoenixdConfig {
    PhoenixdConfig {
      url: self.inner.url.clone(),
      password: self.inner.password.clone(),
    }
  }

  // #[napi]
  // pub async fn get_offer(&self) -> napi::Result<String> {
  //   let offer = lni::phoenixd::api::get_offer(self.inner.url.clone(), self.inner.password.clone())
  //     .await
  //     .map_err(|e| napi::Error::from_reason(e.to_string()))?;
  //   Ok(offer.clone())
  // }
}

