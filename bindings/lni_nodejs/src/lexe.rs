use lni::lexe::{LexeConfig, LexeNode as CoreLexeNode};
use napi_derive::napi;

/// Stateless Lexe access for Node.js functions. Credentials stay on the server.
#[napi]
pub struct LexeNode {
  inner: CoreLexeNode,
}

#[napi]
impl LexeNode {
  #[napi(constructor)]
  pub fn new(client_credentials: String, network: Option<String>) -> napi::Result<Self> {
    let config = LexeConfig {
      client_credentials,
      network,
      data_dir: None,
    };
    let inner = CoreLexeNode::new_stateless(config)
      .map_err(|_| napi::Error::from_reason("Failed to initialize Lexe node"))?;
    Ok(Self { inner })
  }

  #[napi]
  pub async fn get_info(&self) -> napi::Result<lni::NodeInfo> {
    self
      .inner
      .get_info()
      .await
      .map_err(|_| napi::Error::from_reason("Failed to get Lexe node info"))
  }

  #[napi]
  pub async fn get_next_unused_address(&self) -> napi::Result<String> {
    self
      .inner
      .get_next_unused_address()
      .await
      .map_err(|_| napi::Error::from_reason("Failed to get Lexe receiving address"))
  }
}
