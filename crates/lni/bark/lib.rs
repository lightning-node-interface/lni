#[cfg(feature = "napi_rs")]
use napi_derive::napi;

use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;

use bark::bip39::Mnemonic;
use bark::lock_manager::memory::MemoryLockManager;
use bark::persist::sqlite::SqliteClient;
use bark::{Config, Wallet};
use bitcoin::Network;

use crate::bark::types::BarkBackup;
use crate::types::NodeInfo;
#[cfg(not(feature = "uniffi"))]
use crate::LightningNode;
use crate::{
    ApiError, CreateInvoiceParams, CreateOfferParams, ListTransactionsParams, LookupInvoiceParams,
    Offer, PayInvoiceParams, PayInvoiceResponse, Transaction,
};

#[cfg_attr(feature = "napi_rs", napi(object))]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
#[derive(Clone)]
pub struct BarkConfig {
    /// 12 or 24 word mnemonic phrase.
    pub mnemonic: String,
    /// Storage directory path for Bark wallet data.
    pub storage_dir: String,
    /// Ark server URL.
    pub server_url: String,
    /// Optional server access token for private Ark servers.
    #[cfg_attr(feature = "uniffi", uniffi(default = None))]
    pub server_access_token: Option<String>,
    /// Esplora URL for chain data.
    #[cfg_attr(feature = "uniffi", uniffi(default = None))]
    pub esplora_url: Option<String>,
    /// Network: "mainnet", "bitcoin", "signet", "testnet", "testnet4", or "regtest".
    #[cfg_attr(feature = "uniffi", uniffi(default = Some("mainnet")))]
    pub network: Option<String>,
    /// Create the wallet database when it does not exist yet.
    #[cfg_attr(feature = "uniffi", uniffi(default = Some(true)))]
    pub create_if_missing: Option<bool>,
    /// Allow wallet creation without successfully connecting to the Ark server.
    #[cfg_attr(feature = "uniffi", uniffi(default = Some(false)))]
    pub force_create: Option<bool>,
}

impl std::fmt::Debug for BarkConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BarkConfig")
            .field("mnemonic", &"<redacted>")
            .field("storage_dir", &"<redacted>")
            .field("server_url", &self.server_url)
            .field("server_access_token", &"<redacted>")
            .field("esplora_url", &self.esplora_url)
            .field("network", &self.network)
            .field("create_if_missing", &self.create_if_missing)
            .field("force_create", &self.force_create)
            .finish()
    }
}

impl Default for BarkConfig {
    fn default() -> Self {
        Self {
            mnemonic: "".to_string(),
            storage_dir: "./bark_data".to_string(),
            // Signet test endpoints:
            // server_url: "https://ark.signet.2nd.dev"
            // esplora_url: "https://esplora.signet.2nd.dev"
            server_url: "https://ark.second.tech".to_string(),
            server_access_token: None,
            esplora_url: Some("https://mempool.second.tech/api".to_string()),
            network: Some("mainnet".to_string()),
            create_if_missing: Some(true),
            force_create: Some(false),
        }
    }
}

impl BarkConfig {
    fn get_network(&self) -> Result<Network, ApiError> {
        match self.network.as_deref().unwrap_or("mainnet") {
            "mainnet" | "bitcoin" => Ok(Network::Bitcoin),
            "signet" => Ok(Network::Signet),
            "testnet" => Ok(Network::Testnet),
            "testnet4" => Ok(Network::Testnet4),
            "regtest" => Ok(Network::Regtest),
            network => Err(ApiError::InvalidInput(format!(
                "Unsupported Bark network: {}",
                network
            ))),
        }
    }

    fn network_label(&self) -> &str {
        self.network.as_deref().unwrap_or("mainnet")
    }

    fn bark_config(&self, network: Network) -> Config {
        Config {
            server_address: self.server_url.clone(),
            server_access_token: self.server_access_token.clone(),
            esplora_address: self.esplora_url.clone(),
            ..Config::network_default(network)
        }
    }
}

fn canonical_network_label(network: Network) -> &'static str {
    match network {
        Network::Bitcoin => "bitcoin",
        Network::Signet => "signet",
        Network::Testnet => "testnet",
        Network::Testnet4 => "testnet4",
        Network::Regtest => "regtest",
    }
}

#[cfg_attr(feature = "uniffi", derive(uniffi::Object))]
#[derive(Clone)]
pub struct BarkNode {
    pub config: BarkConfig,
    wallet: Arc<Wallet>,
}

#[cfg_attr(feature = "uniffi", uniffi::export(async_runtime = "tokio"))]
impl BarkNode {
    #[cfg_attr(feature = "uniffi", uniffi::constructor)]
    pub async fn new(config: BarkConfig) -> Result<Self, ApiError> {
        let network = config.get_network()?;
        let mnemonic = Mnemonic::from_str(&config.mnemonic)
            .map_err(|e| ApiError::InvalidInput(format!("Invalid Bark mnemonic: {}", e)))?;
        let storage_dir = PathBuf::from(&config.storage_dir);
        tokio::fs::create_dir_all(&storage_dir)
            .await
            .map_err(|e| ApiError::Api {
                reason: format!("Failed to create Bark storage directory: {}", e),
            })?;

        let db = Arc::new(
            SqliteClient::open(storage_dir.join("db.sqlite")).map_err(|e| ApiError::Api {
                reason: format!("Failed to open Bark database: {}", e),
            })?,
        );
        let bark_config = config.bark_config(network);

        let wallet = match Wallet::open(
            &mnemonic,
            db.clone(),
            bark_config.clone(),
            Box::new(MemoryLockManager::new()),
        )
        .await
        {
            Ok(wallet) => wallet,
            Err(open_error) if config.create_if_missing.unwrap_or(true) => Wallet::create(
                &mnemonic,
                network,
                bark_config,
                db,
                Box::new(MemoryLockManager::new()),
                config.force_create.unwrap_or(false),
            )
            .await
            .map_err(|create_error| ApiError::Api {
                reason: format!(
                    "Failed to open or create Bark wallet: {}; create failed: {}",
                    open_error, create_error
                ),
            })?,
            Err(open_error) => {
                return Err(ApiError::Api {
                    reason: format!("Failed to open Bark wallet: {}", open_error),
                });
            }
        };

        Ok(Self {
            config,
            wallet: Arc::new(wallet),
        })
    }

    pub async fn get_ark_address(&self) -> Result<String, ApiError> {
        self.wallet
            .new_address()
            .await
            .map(|address| address.to_string())
            .map_err(|e| ApiError::Api {
                reason: e.to_string(),
            })
    }
}

impl BarkNode {
    pub fn get_wallet(&self) -> Arc<Wallet> {
        self.wallet.clone()
    }

    /// Create an encrypted snapshot of the Bark wallet storage directory.
    ///
    /// This backs up SDK-owned local state, including VTXO state in SQLite. It is
    /// intentionally not a mnemonic-only restore mechanism; callers should store
    /// the returned encrypted blob wherever their app backup policy requires.
    pub async fn create_backup(&self, backup_secret: String) -> Result<BarkBackup, ApiError> {
        self.wallet.sync().await;
        let network = self.config.get_network()?;

        crate::bark::backup::create_encrypted_backup(
            &PathBuf::from(&self.config.storage_dir),
            &backup_secret,
            self.wallet.fingerprint().to_string(),
            canonical_network_label(network).to_string(),
            self.config.server_url.clone(),
            self.config.esplora_url.clone(),
        )
    }

    /// Restore an encrypted Bark wallet-state snapshot into `config.storage_dir`.
    ///
    /// The restored wallet is opened with `create_if_missing = false` so a bad or
    /// incomplete restore cannot silently create a fresh empty wallet.
    pub async fn restore_from_backup(
        mut config: BarkConfig,
        backup: BarkBackup,
        backup_secret: String,
        overwrite_existing: bool,
    ) -> Result<Self, ApiError> {
        let network = config.get_network()?;
        crate::bark::backup::restore_encrypted_backup(
            &PathBuf::from(&config.storage_dir),
            &backup,
            &backup_secret,
            canonical_network_label(network),
            &config.server_url,
            overwrite_existing,
        )?;

        config.create_if_missing = Some(false);
        Self::new(config).await
    }
}

#[cfg_attr(feature = "uniffi", uniffi::export(async_runtime = "tokio"))]
impl BarkNode {
    pub async fn get_permissions(&self) -> Result<crate::Permissions, ApiError> {
        Err(ApiError::InvalidInput(
            "Bark wallet credentials cannot be introspected. Manually test permissions against Bark wallet operations.".to_string(),
        ))
    }

    pub async fn get_info(&self) -> Result<NodeInfo, ApiError> {
        crate::bark::api::get_info(self.wallet.clone(), self.config.network_label()).await
    }

    pub async fn create_invoice(
        &self,
        params: CreateInvoiceParams,
    ) -> Result<Transaction, ApiError> {
        crate::bark::api::create_invoice(self.wallet.clone(), params).await
    }

    pub async fn pay_invoice(
        &self,
        params: PayInvoiceParams,
    ) -> Result<PayInvoiceResponse, ApiError> {
        crate::bark::api::pay_invoice(self.wallet.clone(), params).await
    }

    pub async fn create_offer(&self, _params: CreateOfferParams) -> Result<Offer, ApiError> {
        Err(ApiError::Api {
            reason: "create_offer not yet implemented for BarkNode".to_string(),
        })
    }

    pub async fn lookup_invoice(
        &self,
        params: LookupInvoiceParams,
    ) -> Result<Transaction, ApiError> {
        crate::bark::api::lookup_invoice(self.wallet.clone(), params).await
    }

    pub async fn list_transactions(
        &self,
        params: ListTransactionsParams,
    ) -> Result<Vec<Transaction>, ApiError> {
        crate::bark::api::list_transactions(self.wallet.clone(), params).await
    }

    pub async fn decode(&self, str: String) -> Result<String, ApiError> {
        crate::utils::decode_bolt11(str)
    }

    pub async fn decode_offer(&self, offer: String) -> Result<String, ApiError> {
        crate::utils::decode_offer(offer)
    }

    pub async fn on_invoice_events(
        &self,
        params: crate::types::OnInvoiceEventParams,
        callback: std::sync::Arc<dyn crate::types::OnInvoiceEventCallback>,
    ) {
        crate::bark::api::on_invoice_events(self.wallet.clone(), params, callback).await
    }

    pub async fn get_offer(&self, search: Option<String>) -> Result<Offer, ApiError> {
        crate::bark::api::get_offer(search)
    }

    pub async fn list_offers(&self, search: Option<String>) -> Result<Vec<Offer>, ApiError> {
        crate::bark::api::list_offers(search)
    }

    pub async fn pay_offer(
        &self,
        offer: String,
        amount_msats: i64,
        payer_note: Option<String>,
    ) -> Result<PayInvoiceResponse, ApiError> {
        crate::bark::api::pay_offer(offer, amount_msats, payer_note)
    }
}

crate::impl_lightning_node!(BarkNode);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bark_config_default() {
        let config = BarkConfig::default();

        assert!(config.mnemonic.is_empty());
        assert_eq!(config.network.as_deref(), Some("mainnet"));
        assert_eq!(config.create_if_missing, Some(true));
    }

    #[test]
    fn test_bark_network_parsing() {
        let mut config = BarkConfig {
            mnemonic: "".to_string(),
            storage_dir: "".to_string(),
            server_url: "".to_string(),
            server_access_token: None,
            esplora_url: None,
            network: Some("mainnet".to_string()),
            create_if_missing: Some(true),
            force_create: Some(false),
        };

        assert_eq!(config.get_network().unwrap(), Network::Bitcoin);
        config.network = Some("signet".to_string());
        assert_eq!(config.get_network().unwrap(), Network::Signet);
        config.network = Some("regtest".to_string());
        assert_eq!(config.get_network().unwrap(), Network::Regtest);
        config.network = Some("bogus".to_string());
        assert!(config.get_network().is_err());
    }
}
