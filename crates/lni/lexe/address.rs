use lexe::{
    bitcoin::{Address, Network},
    wallet::LexeWallet,
};
use lexe_api::{
    def::UserGatewayApi, models::command::CheckAddressRequest, types::payments::PaymentId,
};

use super::{LexeAddressCheck, LexeReceivingCapabilities};
use crate::ApiError;

fn receiving_capabilities(version: lexe::semver::Version) -> LexeReceivingCapabilities {
    LexeReceivingCapabilities {
        exclusive_addresses: version >= lexe::semver::Version::new(0, 10, 9),
        node_version: version.to_string(),
    }
}

// NodeInfo.version is required and deserialized by the released SDK.
// https://github.com/lexe-app/lexe-public/blob/lexe-sdk-rust-v0.1.29/lexe-api-core/src/models/command.rs
pub(super) async fn get_receiving_capabilities(
    wallet: &LexeWallet,
) -> Result<LexeReceivingCapabilities, ApiError> {
    let info = wallet.node_info().await.map_err(|_| ApiError::Api {
        reason: "Failed to inspect Lexe receiving capabilities".into(),
    })?;
    Ok(receiving_capabilities(info.version))
}

fn bitcoin_network(network: &str) -> Result<Network, ApiError> {
    match network {
        "mainnet" => Ok(Network::Bitcoin),
        "testnet3" => Ok(Network::Testnet),
        _ => Err(ApiError::InvalidInput("Unsupported Lexe network".into())),
    }
}

fn checked_address(address: &str, network: &str) -> Result<Address, ApiError> {
    address
        .parse::<Address<_>>()
        .map_err(|_| ApiError::InvalidInput("Invalid Lexe Bitcoin address".into()))?
        .require_network(bitcoin_network(network)?)
        .map_err(|_| ApiError::InvalidInput("Lexe Bitcoin address has the wrong network".into()))
}

// https://github.com/lexe-app/lexe-public/blob/lexe-sdk-rust-v0.1.29/lexe/src/wallet.rs
pub(super) async fn get_exclusive_address(
    wallet: &LexeWallet,
    network: &str,
) -> Result<String, ApiError> {
    let response = wallet
        .get_exclusive_address()
        .await
        .map_err(|_| ApiError::Api {
            reason: "Failed to get Lexe exclusive address; node v0.10.9 or later is required"
                .into(),
        })?;
    response
        .address
        .require_network(bitcoin_network(network)?)
        .map(|address| address.to_string())
        .map_err(|_| ApiError::Api {
            reason: "Lexe exclusive address has the wrong network".into(),
        })
}

pub(super) async fn check_address(
    wallet: &LexeWallet,
    network: &str,
    address: &str,
    synced_block_height: Option<u32>,
) -> Result<LexeAddressCheck, ApiError> {
    let address = checked_address(address, network)?.into_unchecked();
    // The public wallet check also fetches payments and needs read_payments.
    // This typed gateway call records receives while preserving receive-only access.
    let auth = wallet
        .node_client()
        .get_gateway_proxy_token()
        .await
        .map_err(|_| ApiError::Api {
            reason: "Failed to authorize Lexe address check".into(),
        })?;
    let response = wallet
        .gateway_client()
        .check_address(
            &CheckAddressRequest {
                address,
                synced_block_height,
            },
            auth,
        )
        .await
        .map_err(|_| ApiError::Api {
            reason: "Failed to sync Lexe exclusive address".into(),
        })?;
    Ok(LexeAddressCheck {
        received_txids: response
            .payment_ids
            .into_iter()
            .filter_map(|id| match id {
                PaymentId::OnchainRecv(txid) => Some(txid.to_string()),
                _ => None,
            })
            .collect(),
        synced_block_height: response.synced_block_height,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exclusive_addresses_require_node_0_10_9_or_later() {
        for (version, supported) in [
            ("0.10.8", false),
            ("0.10.9-rc.1", false),
            ("0.10.9", true),
            ("0.10.9+build.66", true),
            ("0.10.10", true),
            ("0.11.0", true),
            ("1.0.0", true),
        ] {
            let capabilities = receiving_capabilities(version.parse().unwrap());
            assert_eq!(capabilities.node_version, version);
            assert_eq!(capabilities.exclusive_addresses, supported, "{version}");
        }
    }

    #[test]
    fn released_sdk_trusts_a_node_with_exclusive_addresses() {
        let (version, _) = lexe::provision::latest_trusted_node_release();
        assert!(version >= lexe::semver::Version::new(0, 10, 9));
    }

    #[test]
    fn address_network_is_verified_before_sending_to_lexe() {
        let mainnet = "1BoatSLRHtKNngkdXEeobR76b53LETtpyT";
        let testnet = "mipcBbFg9gMiCh81Kj8tqqdgoZub1ZJRfn";
        assert_eq!(
            checked_address(mainnet, "mainnet").unwrap().to_string(),
            mainnet
        );
        assert!(checked_address(mainnet, "testnet3").is_err());
        assert!(checked_address(testnet, "mainnet").is_err());
        assert!(checked_address(testnet, "testnet3").is_ok());
        assert!(checked_address("invalid", "mainnet").is_err());
    }
}
