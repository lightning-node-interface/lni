#[cfg(feature = "napi_rs")]
use napi_derive::napi;

use crate::{cln::api::*, ApiError};
use serde::{Deserialize, Serialize};

use crate::types::NodeInfo;

#[cfg_attr(feature = "napi_rs", napi(object))]
pub struct ClnConfig {
    pub url: String,
    pub rune: String,
}

#[cfg_attr(feature = "napi_rs", napi(object))]
pub struct ClnNode {
    pub url: String,
    pub rune: String,
}

impl ClnNode {
    pub fn new(config: ClnConfig) -> Self {
        Self {
            url: config.url,
            rune: config.rune,
        }
    }

    pub async fn get_info(&self) -> Result<NodeInfo, ApiError> {
        crate::cln::api::get_info(self.url.clone(), self.rune.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dotenv::dotenv;
    use lazy_static::lazy_static;
    use std::env;
    use tokio::test;

    lazy_static! {
        static ref URL: String = {
            dotenv().ok();
            env::var("CLN_URL").expect("CLN_URL must be set")
        };
        static ref RUNE: String = {
            dotenv().ok();
            env::var("CLN_RUNE").expect("CLN_RUNE must be set")
        };
        static ref NODE: ClnNode = {
            ClnNode::new(ClnConfig {
                url: URL.clone(),
                rune: RUNE.clone(),
            })
        };
    }

    #[test]
    async fn test_get_info() {
        match NODE.get_info().await {
            Ok(info) => {
                println!("info: {:?}", info);
                assert!(!info.pubkey.is_empty(), "Node pubkey should not be empty");
            }
            Err(e) => {
                panic!("Failed to get offer: {:?}", e);
            }
        }
    }
}
