//! Watch-only receiving. No private keys, signing, address counters or merchant service.
//! Esplora observations trust the explicitly selected backend for chain inclusion.

use crate::ApiError;
use lightning::bitcoin::{
    self,
    bip32::{ChildNumber, Xpub},
    secp256k1::Secp256k1,
    Address, CompressedPublicKey, Network,
};
use serde::Deserialize;
use std::str::FromStr;

#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BitcoinNetwork {
    Mainnet,
    Testnet,
    Signet,
    Regtest,
}

impl BitcoinNetwork {
    fn bitcoin(self) -> Network {
        match self {
            Self::Mainnet => Network::Bitcoin,
            Self::Testnet => Network::Testnet,
            Self::Signet => Network::Signet,
            Self::Regtest => Network::Regtest,
        }
    }
}

#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WatchOnlyScript {
    P2pkh,
    P2shP2wpkh,
    P2wpkh,
    P2tr,
}

#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
#[derive(Clone)]
pub struct WatchOnlyConfig {
    /// An account-level xpub/tpub. This interface always derives external /0/index.
    pub account_xpub: String,
    pub network: BitcoinNetwork,
    pub script_type: WatchOnlyScript,
}

fn invalid() -> ApiError {
    ApiError::InvalidInput("Invalid watch-only configuration or chain response".into())
}
fn unavailable() -> ApiError {
    ApiError::NetworkError("Chain backend unavailable".into())
}

// SLIP-132 public version bytes. Private versions and multisig variants are rejected.
fn parse_account_xpub(config: &WatchOnlyConfig) -> Result<Xpub, ApiError> {
    let mut bytes = bitcoin::base58::decode_check(&config.account_xpub).map_err(|_| invalid())?;
    if bytes.len() != 78 {
        return Err(invalid());
    }
    let version = u32::from_be_bytes(bytes[..4].try_into().map_err(|_| invalid())?);
    let (replacement, required) = match version {
        0x0488b21e | 0x043587cf => (version, None),
        0x049d7cb2 => (0x0488b21e, Some(WatchOnlyScript::P2shP2wpkh)),
        0x04b24746 => (0x0488b21e, Some(WatchOnlyScript::P2wpkh)),
        0x044a5262 => (0x043587cf, Some(WatchOnlyScript::P2shP2wpkh)),
        0x045f1cf6 => (0x043587cf, Some(WatchOnlyScript::P2wpkh)),
        _ => return Err(invalid()),
    };
    if required.is_some_and(|script| script != config.script_type) {
        return Err(invalid());
    }
    bytes[..4].copy_from_slice(&replacement.to_be_bytes());
    Xpub::decode(&bytes).map_err(|_| invalid())
}

/// The caller must allocate and persist indices atomically before sharing addresses.
#[cfg_attr(feature = "uniffi", uniffi::export)]
pub fn derive_watch_only_address(config: WatchOnlyConfig, index: u32) -> Result<String, ApiError> {
    let xpub = parse_account_xpub(&config)?;
    let network = config.network.bitcoin();
    if xpub.network != network.into() {
        return Err(invalid());
    }
    let child = ChildNumber::from_normal_idx(index).map_err(|_| invalid())?;
    let secp = Secp256k1::verification_only();
    let key = xpub
        .derive_pub(
            &secp,
            &[
                ChildNumber::from_normal_idx(0).map_err(|_| invalid())?,
                child,
            ],
        )
        .map_err(|_| invalid())?
        .public_key;
    let compressed = CompressedPublicKey(key);
    let address = match config.script_type {
        WatchOnlyScript::P2pkh => Address::p2pkh(bitcoin::PublicKey::new(key), network),
        WatchOnlyScript::P2shP2wpkh => Address::p2shwpkh(&compressed, network),
        WatchOnlyScript::P2wpkh => Address::p2wpkh(&compressed, network),
        WatchOnlyScript::P2tr => Address::p2tr(&secp, key.x_only_public_key().0, None, network),
    };
    Ok(address.to_string())
}

#[cfg_attr(feature = "uniffi", uniffi::export)]
pub fn validate_bitcoin_address(
    address: String,
    network: BitcoinNetwork,
) -> Result<String, ApiError> {
    Ok(Address::from_str(&address)
        .map_err(|_| invalid())?
        .require_network(network.bitcoin())
        .map_err(|_| invalid())?
        .to_string())
}

/// Derive an explicitly selected external branch of a public ranged descriptor.
/// Multipath descriptors must be split by the user into the intended receive branch.
/// Secret keys and hardened child derivation are never accepted.
#[cfg_attr(feature = "uniffi", uniffi::export)]
pub fn derive_descriptor_address(
    descriptor: String,
    network: BitcoinNetwork,
    index: u32,
) -> Result<String, ApiError> {
    use miniscript::{
        descriptor::{DescriptorPublicKey, Wildcard},
        Descriptor, ForEachKey,
    };
    if index >= (1 << 31) || descriptor.len() > 16_384 {
        return Err(invalid());
    }
    let descriptor =
        Descriptor::<DescriptorPublicKey>::from_str(&descriptor).map_err(|_| invalid())?;
    descriptor.sanity_check().map_err(|_| invalid())?;
    if !descriptor.has_wildcard()
        || !descriptor.for_each_key(|key| match key {
            DescriptorPublicKey::XPub(key) => {
                key.xkey.network == network.bitcoin().into()
                    && key.wildcard != Wildcard::Hardened
                    && key
                        .derivation_path
                        .into_iter()
                        .all(|child| !child.is_hardened())
            }
            DescriptorPublicKey::MultiXPub(_) => false,
            DescriptorPublicKey::Single(_) => true,
        })
    {
        return Err(invalid());
    }
    let definite = descriptor
        .at_derivation_index(index)
        .map_err(|_| invalid())?;
    let derived = definite.derived_descriptor(&Secp256k1::verification_only());
    Ok(derived
        .address(network.bitcoin())
        .map_err(|_| invalid())?
        .to_string())
}

#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BitcoinPaymentObservation {
    pub txid: String,
    pub vout: u32,
    pub amount_sats: u64,
    pub confirmed: bool,
    pub block_height: Option<u32>,
    pub block_hash: Option<String>,
    pub confirmations: u32,
}

#[derive(Deserialize)]
struct EsploraTransaction {
    txid: String,
    status: EsploraStatus,
}
#[derive(Deserialize)]
struct EsploraStatus {
    confirmed: bool,
    block_height: Option<u32>,
    block_hash: Option<String>,
}

async fn bounded_get(client: &reqwest::Client, url: reqwest::Url) -> Result<Vec<u8>, ApiError> {
    let mut response = client.get(url).send().await.map_err(|_| unavailable())?;
    if !response.status().is_success() {
        return Err(unavailable());
    }
    const LIMIT: usize = 8_000_000;
    if response.content_length().is_some_and(|n| n > LIMIT as u64) {
        return Err(invalid());
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| unavailable())? {
        if bytes.len().saturating_add(chunk.len()) > LIMIT {
            return Err(invalid());
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

/// Reads every confirmed page plus mempool transactions, verifies each raw transaction's
/// txid and destination outputs, and checks the backend's current block hash at its height.
/// Does not assert independent consensus or settlement; callers choose confirmations and
/// must re-observe after reorganizations. Fails closed when pagination exceeds 100 pages.
#[cfg_attr(feature = "uniffi", uniffi::export(async_runtime = "tokio"))]
pub async fn observe_bitcoin_address(
    address: String,
    network: BitcoinNetwork,
    esplora_url: String,
) -> Result<Vec<BitcoinPaymentObservation>, ApiError> {
    let checked = Address::from_str(&address)
        .map_err(|_| invalid())?
        .require_network(network.bitcoin())
        .map_err(|_| invalid())?;
    let base = crate::lnurl::validate_public_https_url(&esplora_url).map_err(|_| invalid())?;
    if base.query().is_some() || base.fragment().is_some() {
        return Err(invalid());
    }
    let base = reqwest::Url::parse(&format!("{}/", base.as_str().trim_end_matches('/')))
        .map_err(|_| invalid())?;
    let client = crate::lnurl::lnurl_http_client(&base)
        .await
        .map_err(|_| unavailable())?;
    observe_with_client(checked, network, base, client).await
}

async fn observe_with_client(
    checked: Address,
    network: BitcoinNetwork,
    base: reqwest::Url,
    client: reqwest::Client,
) -> Result<Vec<BitcoinPaymentObservation>, ApiError> {
    let endpoint = |path: &str| base.join(path).map_err(|_| invalid());
    let genesis = bounded_get(&client, endpoint("block-height/0")?).await?;
    if std::str::from_utf8(&genesis).map_err(|_| invalid())?.trim()
        != bitcoin::blockdata::constants::genesis_block(network.bitcoin())
            .block_hash()
            .to_string()
    {
        return Err(invalid());
    }
    // Pin the tip across the complete read to avoid reporting mixed snapshots.
    let tip_hash = bounded_get(&client, endpoint("blocks/tip/hash")?).await?;
    let tip = bounded_get(&client, endpoint("blocks/tip/height")?).await?;
    let tip: u32 = std::str::from_utf8(&tip)
        .map_err(|_| invalid())?
        .trim()
        .parse()
        .map_err(|_| invalid())?;
    let mut transactions: Vec<EsploraTransaction> = serde_json::from_slice(
        &bounded_get(
            &client,
            endpoint(&format!("address/{checked}/txs/mempool"))?,
        )
        .await?,
    )
    .map_err(|_| invalid())?;
    let mut cursor = String::new();
    for page in 0..100 {
        let path = format!("address/{checked}/txs/chain{cursor}");
        let batch: Vec<EsploraTransaction> =
            serde_json::from_slice(&bounded_get(&client, endpoint(&path)?).await?)
                .map_err(|_| invalid())?;
        if batch.len() > 25 {
            return Err(invalid());
        }
        let done = batch.len() < 25;
        if let Some(last) = batch.last() {
            bitcoin::Txid::from_str(&last.txid).map_err(|_| invalid())?;
            cursor = format!("/{}", last.txid);
        }
        transactions.extend(batch);
        if done {
            break;
        }
        if page == 99 {
            return Err(invalid());
        }
    }
    let script = checked.script_pubkey();
    let mut seen = std::collections::HashSet::new();
    let mut observations = Vec::new();
    for item in transactions {
        let txid = bitcoin::Txid::from_str(&item.txid).map_err(|_| invalid())?;
        if !seen.insert(txid) {
            continue;
        }
        let raw = bounded_get(&client, endpoint(&format!("tx/{txid}/raw"))?).await?;
        let tx: bitcoin::Transaction =
            bitcoin::consensus::deserialize(&raw).map_err(|_| invalid())?;
        if tx.compute_txid() != txid {
            return Err(invalid());
        }
        let (height, hash, confirmations) = if item.status.confirmed {
            let height = item.status.block_height.ok_or_else(invalid)?;
            let hash = item.status.block_hash.ok_or_else(invalid)?;
            bitcoin::BlockHash::from_str(&hash).map_err(|_| invalid())?;
            if height > tip {
                return Err(invalid());
            }
            let current =
                bounded_get(&client, endpoint(&format!("block-height/{height}"))?).await?;
            if std::str::from_utf8(&current).map_err(|_| invalid())?.trim() != hash {
                return Err(invalid());
            }
            (
                Some(height),
                Some(hash),
                tip.checked_sub(height)
                    .and_then(|n| n.checked_add(1))
                    .ok_or_else(invalid)?,
            )
        } else {
            (None, None, 0)
        };
        for (vout, output) in tx.output.iter().enumerate() {
            if output.script_pubkey == script {
                if output.value.to_sat() > 2_100_000_000_000_000 {
                    return Err(invalid());
                }
                observations.push(BitcoinPaymentObservation {
                    txid: txid.to_string(),
                    vout: u32::try_from(vout).map_err(|_| invalid())?,
                    amount_sats: output.value.to_sat(),
                    confirmed: item.status.confirmed,
                    block_height: height,
                    block_hash: hash.clone(),
                    confirmations,
                });
            }
        }
    }
    if bounded_get(&client, endpoint("blocks/tip/hash")?).await? != tip_hash {
        return Err(unavailable());
    }
    Ok(observations)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitcoin::{
        absolute,
        bip32::{DerivationPath, Xpriv},
        consensus::serialize,
        hashes::Hash,
        transaction, Amount, TxOut,
    };
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    fn bip84() -> WatchOnlyConfig {
        // Public BIP84 test vector, never a wallet used for funds.
        let mnemonic = bip39::Mnemonic::parse("abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about").unwrap();
        let secp = Secp256k1::new();
        let account = Xpriv::new_master(Network::Bitcoin, &mnemonic.to_seed(""))
            .unwrap()
            .derive_priv(&secp, &DerivationPath::from_str("m/84'/0'/0'").unwrap())
            .unwrap();
        WatchOnlyConfig {
            account_xpub: Xpub::from_priv(&secp, &account).to_string(),
            network: BitcoinNetwork::Mainnet,
            script_type: WatchOnlyScript::P2wpkh,
        }
    }

    #[test]
    fn public_bip84_vector_and_network_hardened_rejection() {
        assert_eq!(
            derive_watch_only_address(bip84(), 0).unwrap(),
            "bc1qcr8te4kr609gcawutmrza0j4xv80jy8z306fyu"
        );
        assert!(derive_watch_only_address(bip84(), 1 << 31).is_err());
        let mut wrong = bip84();
        wrong.network = BitcoinNetwork::Testnet;
        assert!(derive_watch_only_address(wrong, 0).is_err());
        let mut secret = bip84();
        secret.account_xpub = "xprv-not-accepted".into();
        assert!(derive_watch_only_address(secret, 0).is_err());
        assert!(validate_bitcoin_address(
            "bc1qcr8te4kr609gcawutmrza0j4xv80jy8z306fyu".into(),
            BitcoinNetwork::Testnet
        )
        .is_err());
        for kind in [
            WatchOnlyScript::P2pkh,
            WatchOnlyScript::P2shP2wpkh,
            WatchOnlyScript::P2wpkh,
            WatchOnlyScript::P2tr,
        ] {
            let mut config = bip84();
            config.script_type = kind;
            let first = derive_watch_only_address(config.clone(), 0).unwrap();
            let second = derive_watch_only_address(config, 1).unwrap();
            assert_ne!(first, second);
            assert!(validate_bitcoin_address(first, BitcoinNetwork::Mainnet).is_ok());
        }
    }

    #[test]
    fn slip132_public_keys_require_matching_script() {
        let mut config = bip84();
        let expected = derive_watch_only_address(config.clone(), 0).unwrap();
        let mut bytes = Xpub::from_str(&config.account_xpub).unwrap().encode();
        bytes[..4].copy_from_slice(&0x04b24746_u32.to_be_bytes());
        config.account_xpub = bitcoin::base58::encode_check(&bytes);
        assert!(config.account_xpub.starts_with("zpub"));
        assert_eq!(
            derive_watch_only_address(config.clone(), 0).unwrap(),
            expected
        );
        config.script_type = WatchOnlyScript::P2pkh;
        assert!(derive_watch_only_address(config, 0).is_err());
    }

    #[test]
    fn descriptor_derivation_checks_network_checksum_and_branch() {
        let config = bip84();
        let descriptor = format!("wpkh({}/0/*)", config.account_xpub);
        assert_eq!(
            derive_descriptor_address(descriptor.clone(), BitcoinNetwork::Mainnet, 0).unwrap(),
            derive_watch_only_address(config.clone(), 0).unwrap()
        );
        assert!(derive_descriptor_address(descriptor.clone(), BitcoinNetwork::Testnet, 0).is_err());
        assert!(derive_descriptor_address(
            format!("{descriptor}#aaaaaaaa"),
            BitcoinNetwork::Mainnet,
            0
        )
        .is_err());
        for suffix in ["/0/0", "/0/*h", "/0h/*", "/<0;1>/*"] {
            assert!(derive_descriptor_address(
                format!("wpkh({}{suffix})", config.account_xpub),
                BitcoinNetwork::Mainnet,
                0
            )
            .is_err());
        }
    }

    #[tokio::test]
    async fn local_esplora_checks_exact_outputs_and_rejects_wrong_raw_tx() {
        let address = Address::from_str(&derive_watch_only_address(bip84(), 0).unwrap())
            .unwrap()
            .require_network(Network::Bitcoin)
            .unwrap();
        let tx = bitcoin::Transaction {
            version: transaction::Version::TWO,
            lock_time: absolute::LockTime::ZERO,
            input: vec![bitcoin::TxIn::default()],
            output: vec![
                TxOut {
                    value: Amount::from_sat(1234),
                    script_pubkey: address.script_pubkey(),
                },
                TxOut {
                    value: Amount::from_sat(9999),
                    script_pubkey: bitcoin::ScriptBuf::new(),
                },
            ],
        };
        let txid = tx.compute_txid();
        let genesis = bitcoin::blockdata::constants::genesis_block(Network::Bitcoin)
            .block_hash()
            .to_string();
        for corrupt in [false, true] {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let base = reqwest::Url::parse(&format!("http://{}/", listener.local_addr().unwrap()))
                .unwrap();
            let raw = if corrupt {
                serialize(&bitcoin::Transaction {
                    output: vec![],
                    ..tx.clone()
                })
            } else {
                serialize(&tx)
            };
            let genesis = genesis.clone();
            let server = tokio::spawn(async move {
                loop {
                    let (mut stream, _) = listener.accept().await.unwrap();
                    let mut data = vec![0; 8192];
                    let n = stream.read(&mut data).await.unwrap();
                    let request = std::str::from_utf8(&data[..n]).unwrap();
                    let path = request.split_whitespace().nth(1).unwrap();
                    let body = if path == "/block-height/0" {
                        genesis.as_bytes().to_vec()
                    } else if path == "/blocks/tip/hash" || path == "/block-height/100" {
                        bitcoin::BlockHash::from_byte_array([1; 32])
                            .to_string()
                            .into_bytes()
                    } else if path == "/blocks/tip/height" {
                        b"102".to_vec()
                    } else if path.ends_with("/txs/mempool") {
                        b"[]".to_vec()
                    } else if path.ends_with("/txs/chain") {
                        serde_json::to_vec(&serde_json::json!([{ "txid": txid.to_string(), "status": { "confirmed": true, "block_height":100, "block_hash":bitcoin::BlockHash::from_byte_array([1;32]).to_string() } }])).unwrap()
                    } else if path == format!("/tx/{txid}/raw") {
                        raw.clone()
                    } else {
                        panic!("unexpected endpoint")
                    };
                    stream.write_all(format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",body.len()).as_bytes()).await.unwrap();
                    stream.write_all(&body).await.unwrap();
                }
            });
            let result = observe_with_client(
                address.clone(),
                BitcoinNetwork::Mainnet,
                base,
                reqwest::Client::new(),
            )
            .await;
            server.abort();
            if corrupt {
                assert!(result.is_err());
            } else {
                let results = result.unwrap();
                assert_eq!(results.len(), 1);
                assert_eq!(results[0].amount_sats, 1234);
                assert_eq!(results[0].confirmations, 3);
                assert_eq!(results[0].vout, 0);
            }
        }
    }
}
