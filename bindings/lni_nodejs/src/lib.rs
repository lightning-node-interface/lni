#![deny(clippy::all)]

extern crate napi_derive;
use napi_derive::napi;

pub use lni::types::*;
pub use lni::types::{InvoiceType, ListTransactionsParams, PayInvoiceResponse, Transaction};
pub use lni::utils::*;
pub use lni::ApiError;

mod phoenixd;
pub use phoenixd::PhoenixdNode;

mod cln;
pub use cln::ClnNode;

mod lnd;
pub use lnd::LndNode;

mod blink;
pub use blink::BlinkNode;

mod nwc;
pub use nwc::NwcNode;

mod strike;
pub use strike::StrikeNode;

mod speed;
pub use speed::SpeedNode;

mod spark;
pub use spark::SparkNode;

mod lnurl;
pub use lnurl::*;

use std::time::Duration;

/// Generate a BIP39 mnemonic phrase
///
/// @param wordCount - Optional number of words (12 or 24). Defaults to 12.
/// @returns A space-separated mnemonic phrase
#[napi]
pub fn generate_mnemonic(word_count: Option<u8>) -> napi::Result<String> {
  use bip39::{Language, Mnemonic};
  use rand::rngs::OsRng;
  use rand::RngCore;

  let entropy_size = match word_count {
    Some(24) => 32,
    _ => 16,
  };

  let mut entropy = vec![0u8; entropy_size];
  OsRng.fill_bytes(&mut entropy);

  match Mnemonic::from_entropy_in(Language::English, &entropy) {
    Ok(mnemonic) => Ok(mnemonic.to_string()),
    Err(e) => Err(napi::Error::from_reason(format!(
      "Failed to generate mnemonic: {}",
      e
    ))),
  }
}

// Make an HTTP request to get IP address and simulate latency with optional SOCKS5 proxy
#[napi]
pub async fn say_after_with_tokio(
  ms: u16,
  who: String,
  url: String,
  socks5_proxy: Option<String>,
  header_key: Option<String>,
  header_value: Option<String>,
) -> napi::Result<String> {
  let default_client = || {
    reqwest::Client::builder()
      .redirect(reqwest::redirect::Policy::none())
      .build()
      .expect("default HTTP client must build")
  };

  // Create HTTP client with optional SOCKS5 proxy
  let client = if let Some(proxy_url) = socks5_proxy {
    let client_builder = reqwest::Client::builder().redirect(reqwest::redirect::Policy::none());

    match reqwest::Proxy::all(&proxy_url) {
      Ok(proxy) => {
        match client_builder.proxy(proxy).build() {
          Ok(client) => client,
          Err(_) => default_client(), // Fallback to default client on error
        }
      }
      Err(_) => default_client(), // Fallback to default client on error
    }
  } else {
    default_client()
  };

  // Create request with optional header
  let mut request = client.get(&url);

  if let (Some(key), Some(value)) = (header_key, header_value) {
    request = request.header(&key, &value);
  }

  // Make HTTP request
  let ip_result = request
    .send()
    .await
    .and_then(|response| response.error_for_status());

  let page_content = match ip_result {
    Ok(response) => match response.text().await {
      Ok(html) => html,
      Err(_) => "Failed to read response text".to_string(),
    },
    Err(_) => "Failed to make HTTP request".to_string(),
  };

  // Simulate latency
  tokio::time::sleep(Duration::from_millis(ms.into())).await;

  Ok(format!(
    "Hello, {who}! Your IP address is: {page_content} (with Tokio after {ms}ms delay)"
  ))
}

#[cfg(test)]
mod tests {
  fn assert_redacted(label: &str, value: &str, secret: &str) {
    assert_eq!(
      value, "<redacted>",
      "{} should return a redacted value",
      label
    );
    assert_ne!(value, secret, "{} leaked a secret value", label);
  }

  #[test]
  fn node_config_getters_redact_secret_fields() {
    let proxy = "socks5h://proxy-user:proxy-pass@127.0.0.1:9150";

    let lnd = crate::LndNode::new(lni::lnd::LndConfig {
      url: "https://lnd.example".to_string(),
      macaroon: "lnd-macaroon-secret".to_string(),
      socks5_proxy: Some(proxy.to_string()),
      accept_invalid_certs: Some(false),
      http_timeout: Some(30),
    });
    let lnd_config = lnd.get_config();
    assert_redacted(
      "LndConfig.macaroon",
      &lnd_config.macaroon,
      "lnd-macaroon-secret",
    );
    assert_redacted(
      "LndConfig.socks5_proxy",
      lnd_config.socks5_proxy.as_deref().unwrap_or_default(),
      proxy,
    );

    let cln = crate::ClnNode::new(lni::cln::ClnConfig {
      url: "https://cln.example".to_string(),
      rune: "cln-rune-secret".to_string(),
      socks5_proxy: Some(proxy.to_string()),
      accept_invalid_certs: Some(false),
      http_timeout: Some(30),
    });
    let cln_config = cln.get_config();
    assert_redacted("ClnConfig.rune", &cln_config.rune, "cln-rune-secret");
    assert_redacted(
      "ClnConfig.socks5_proxy",
      cln_config.socks5_proxy.as_deref().unwrap_or_default(),
      proxy,
    );

    let phoenixd = crate::PhoenixdNode::new(lni::phoenixd::PhoenixdConfig {
      url: "https://phoenixd.example".to_string(),
      password: "phoenixd-password-secret".to_string(),
      socks5_proxy: Some(proxy.to_string()),
      accept_invalid_certs: Some(false),
      http_timeout: Some(30),
    });
    let phoenixd_config = phoenixd.get_config();
    assert_redacted(
      "PhoenixdConfig.password",
      &phoenixd_config.password,
      "phoenixd-password-secret",
    );
    assert_redacted(
      "PhoenixdConfig.socks5_proxy",
      phoenixd_config.socks5_proxy.as_deref().unwrap_or_default(),
      proxy,
    );

    let nwc = crate::NwcNode::new(lni::nwc::NwcConfig {
      nwc_uri: "nostr+walletconnect://wallet-pubkey?relay=wss://relay.example&secret=nwc-secret"
        .to_string(),
      socks5_proxy: Some(proxy.to_string()),
      accept_invalid_certs: Some(false),
      http_timeout: Some(30),
    });
    let nwc_config = nwc.get_config();
    assert_redacted("NwcConfig.nwc_uri", &nwc_config.nwc_uri, "nwc-secret");
    assert_redacted(
      "NwcConfig.socks5_proxy",
      nwc_config.socks5_proxy.as_deref().unwrap_or_default(),
      proxy,
    );

    let strike = crate::StrikeNode::new(lni::strike::StrikeConfig {
      base_url: Some("https://strike.example".to_string()),
      api_key: "strike-api-key-secret".to_string(),
      socks5_proxy: Some(proxy.to_string()),
      accept_invalid_certs: Some(false),
      http_timeout: Some(30),
    });
    let strike_config = strike.get_config();
    assert_redacted(
      "StrikeConfig.api_key",
      &strike_config.api_key,
      "strike-api-key-secret",
    );
    assert_redacted(
      "StrikeConfig.socks5_proxy",
      strike_config.socks5_proxy.as_deref().unwrap_or_default(),
      proxy,
    );

    let blink = crate::BlinkNode::new(lni::blink::BlinkConfig {
      base_url: Some("https://blink.example".to_string()),
      api_key: "blink-api-key-secret".to_string(),
      socks5_proxy: Some(proxy.to_string()),
      accept_invalid_certs: Some(false),
      http_timeout: Some(30),
    });
    let blink_config = blink.get_config();
    assert_redacted(
      "BlinkConfig.api_key",
      &blink_config.api_key,
      "blink-api-key-secret",
    );
    assert_redacted(
      "BlinkConfig.socks5_proxy",
      blink_config.socks5_proxy.as_deref().unwrap_or_default(),
      proxy,
    );

    let speed = crate::SpeedNode::new(lni::speed::SpeedConfig {
      base_url: Some("https://speed.example".to_string()),
      api_key: "speed-api-key-secret".to_string(),
      socks5_proxy: Some(proxy.to_string()),
      accept_invalid_certs: Some(false),
      http_timeout: Some(30),
    });
    let speed_config = speed.get_config();
    assert_redacted(
      "SpeedConfig.api_key",
      &speed_config.api_key,
      "speed-api-key-secret",
    );
    assert_redacted(
      "SpeedConfig.socks5_proxy",
      speed_config.socks5_proxy.as_deref().unwrap_or_default(),
      proxy,
    );

    let spark = crate::SparkNode::new(lni::spark::SparkConfig {
      mnemonic: "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about".to_string(),
      passphrase: Some("spark-passphrase-secret".to_string()),
      api_key: Some("spark-api-key-secret".to_string()),
      storage_dir: "/tmp/lni-spark-secret-path".to_string(),
      network: Some("mainnet".to_string()),
    });
    let spark_config = spark.get_config();
    assert_redacted(
      "SparkConfig.mnemonic",
      &spark_config.mnemonic,
      "abandon abandon abandon",
    );
    assert_redacted(
      "SparkConfig.passphrase",
      spark_config.passphrase.as_deref().unwrap_or_default(),
      "spark-passphrase-secret",
    );
    assert_redacted(
      "SparkConfig.api_key",
      spark_config.api_key.as_deref().unwrap_or_default(),
      "spark-api-key-secret",
    );
    assert_redacted(
      "SparkConfig.storage_dir",
      &spark_config.storage_dir,
      "/tmp/lni-spark-secret-path",
    );
  }
}
