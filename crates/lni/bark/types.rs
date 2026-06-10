//! Types for Bark node integration.
//!
//! The Bark wallet crate supplies most domain types directly. LNI keeps the
//! public cross-node surface in `crate::types`.

#[cfg(feature = "napi_rs")]
use napi_derive::napi;
use serde::{Deserialize, Serialize};

#[cfg_attr(feature = "napi_rs", napi(object))]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct BarkBackupInfo {
    pub version: i32,
    pub created_at: i64,
    pub wallet_fingerprint: String,
    pub network: String,
    pub server_url: String,
    pub esplora_url: Option<String>,
    pub file_count: i32,
    pub snapshot_sha256: String,
}

#[cfg_attr(feature = "napi_rs", napi(object))]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BarkBackup {
    pub info: BarkBackupInfo,
    /// Base64-encoded JSON envelope containing the encrypted wallet-state snapshot.
    ///
    /// Bark owns the SQLite schema for the VTXO tables, so this backup stores
    /// the full `db.sqlite` instead of serializing individual VTXOs. Schema
    /// reference:
    /// https://gitlab.com/ark-bitcoin/bark/-/blob/25e8ac17f308fae685525fe543531e47d3c90855/bark/schema.sql
    ///
    /// Sample decrypted VTXO-oriented payload shape:
    /// {
    ///   "magic": "lni-bark-backup",
    ///   "version": 1,
    ///   "info": {
    ///     "version": 1,
    ///     "created_at": 1717977600,
    ///     "wallet_fingerprint": "bark-wallet-fingerprint",
    ///     "network": "signet",
    ///     "server_url": "https://ark.example",
    ///     "esplora_url": "https://esplora.example",
    ///     "file_count": 1,
    ///     "snapshot_sha256": "..."
    ///   },
    ///   "files": [
    ///     {
    ///       "path": "db.sqlite",
    ///       "sha256": "...",
    ///       "data": "<base64 db.sqlite using the Bark schema above>"
    ///     }
    ///   ]
    /// }
    pub encrypted_data: String,
}

impl std::fmt::Debug for BarkBackup {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BarkBackup")
            .field("info", &self.info)
            .field("encrypted_data", &"<redacted>")
            .finish()
    }
}
