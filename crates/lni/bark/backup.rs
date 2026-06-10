use std::fs;
use std::path::{Component, Path, PathBuf};

use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Nonce};
use rand::rngs::OsRng;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::bark::types::{BarkBackup, BarkBackupInfo};
use crate::ApiError;

const BACKUP_MAGIC: &str = "lni-bark-backup";
const BACKUP_VERSION: i32 = 1;
const BACKUP_DB_FILE: &str = "db.sqlite";
const CIPHER_NAME: &str = "AES-256-GCM";
const KDF_NAME: &str = "PBKDF2-HMAC-SHA256";
const KDF_ITERATIONS: u32 = 600_000;
const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 12;
const KEY_LEN: usize = 32;

#[derive(Debug, Serialize, Deserialize)]
struct EncryptedBackupEnvelope {
    magic: String,
    version: i32,
    cipher: String,
    kdf: String,
    kdf_iterations: u32,
    salt: String,
    nonce: String,
    aad: String,
    ciphertext: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct BackupPlaintext {
    magic: String,
    version: i32,
    info: BarkBackupInfo,
    files: Vec<BackupFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BackupFile {
    path: String,
    data: String,
    sha256: String,
}

pub(crate) fn create_encrypted_backup(
    storage_dir: &Path,
    backup_secret: &str,
    wallet_fingerprint: String,
    network: String,
    server_url: String,
    esplora_url: Option<String>,
) -> Result<BarkBackup, ApiError> {
    validate_backup_secret(backup_secret)?;
    validate_storage_dir_for_backup(storage_dir)?;

    let files = collect_storage_files(storage_dir)?;
    let info = BarkBackupInfo {
        version: BACKUP_VERSION,
        created_at: chrono::Utc::now().timestamp(),
        wallet_fingerprint,
        network,
        server_url,
        esplora_url,
        file_count: i32::try_from(files.len()).unwrap_or(i32::MAX),
        snapshot_sha256: snapshot_sha256(&files),
    };
    let plaintext = BackupPlaintext {
        magic: BACKUP_MAGIC.to_string(),
        version: BACKUP_VERSION,
        info: info.clone(),
        files,
    };
    let plaintext_bytes = serde_json::to_vec(&plaintext)?;
    let aad = serde_json::to_vec(&info)?;
    let encrypted_data = encrypt_backup(backup_secret, &aad, &plaintext_bytes)?;

    Ok(BarkBackup {
        info,
        encrypted_data,
    })
}

pub(crate) fn restore_encrypted_backup(
    storage_dir: &Path,
    backup: &BarkBackup,
    backup_secret: &str,
    expected_network: &str,
    expected_server_url: &str,
    overwrite_existing: bool,
) -> Result<BarkBackupInfo, ApiError> {
    validate_backup_secret(backup_secret)?;
    let plaintext = decrypt_backup(backup_secret, &backup.encrypted_data)?;
    validate_plaintext(&plaintext)?;

    if plaintext.info != backup.info {
        return Err(ApiError::InvalidInput(
            "Bark backup metadata does not match encrypted payload".to_string(),
        ));
    }
    if plaintext.info.network != expected_network {
        return Err(ApiError::InvalidInput(format!(
            "Bark backup network mismatch: backup is {}, config is {}",
            plaintext.info.network, expected_network
        )));
    }
    if plaintext.info.server_url != expected_server_url {
        return Err(ApiError::InvalidInput(
            "Bark backup server URL does not match restore config".to_string(),
        ));
    }
    if snapshot_sha256(&plaintext.files) != plaintext.info.snapshot_sha256 {
        return Err(ApiError::InvalidInput(
            "Bark backup snapshot checksum mismatch".to_string(),
        ));
    }

    install_snapshot(storage_dir, &plaintext.files, overwrite_existing)?;
    Ok(plaintext.info)
}

fn validate_backup_secret(backup_secret: &str) -> Result<(), ApiError> {
    if backup_secret.trim().is_empty() {
        return Err(ApiError::InvalidInput(
            "Bark backup secret cannot be empty".to_string(),
        ));
    }
    Ok(())
}

fn validate_storage_dir_for_backup(storage_dir: &Path) -> Result<(), ApiError> {
    if !storage_dir.is_dir() {
        return Err(ApiError::InvalidInput(format!(
            "Bark storage directory does not exist: {}",
            storage_dir.display()
        )));
    }
    let db_path = storage_dir.join(BACKUP_DB_FILE);
    if !db_path.is_file() {
        return Err(ApiError::InvalidInput(format!(
            "Bark storage directory is missing {}",
            BACKUP_DB_FILE
        )));
    }
    Ok(())
}

fn collect_storage_files(storage_dir: &Path) -> Result<Vec<BackupFile>, ApiError> {
    let mut files = Vec::new();
    collect_storage_files_from(storage_dir, storage_dir, &mut files)?;
    files.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(files)
}

fn collect_storage_files_from(
    storage_dir: &Path,
    current_dir: &Path,
    files: &mut Vec<BackupFile>,
) -> Result<(), ApiError> {
    for entry in fs::read_dir(current_dir).map_err(|e| ApiError::Api {
        reason: format!("Failed to read Bark storage directory: {}", e),
    })? {
        let entry = entry.map_err(|e| ApiError::Api {
            reason: format!("Failed to read Bark storage entry: {}", e),
        })?;
        let path = entry.path();
        let file_type = entry.file_type().map_err(|e| ApiError::Api {
            reason: format!("Failed to inspect Bark storage entry: {}", e),
        })?;

        if file_type.is_dir() {
            collect_storage_files_from(storage_dir, &path, files)?;
        } else if file_type.is_file() {
            let data = fs::read(&path).map_err(|e| ApiError::Api {
                reason: format!("Failed to read Bark storage file: {}", e),
            })?;
            files.push(BackupFile {
                path: archive_path(storage_dir, &path)?,
                sha256: hex::encode(Sha256::digest(&data)),
                data: base64::encode(data),
            });
        }
    }
    Ok(())
}

fn archive_path(storage_dir: &Path, file_path: &Path) -> Result<String, ApiError> {
    let relative = file_path
        .strip_prefix(storage_dir)
        .map_err(|e| ApiError::Api {
            reason: format!("Failed to build Bark backup path: {}", e),
        })?;
    let mut parts = Vec::new();
    for component in relative.components() {
        match component {
            Component::Normal(part) => {
                let part = part.to_str().ok_or_else(|| {
                    ApiError::InvalidInput("Bark storage file path must be UTF-8".to_string())
                })?;
                parts.push(part.to_string());
            }
            _ => {
                return Err(ApiError::InvalidInput(
                    "Bark storage file path is not backup-safe".to_string(),
                ));
            }
        }
    }
    if parts.is_empty() {
        return Err(ApiError::InvalidInput(
            "Bark backup file path cannot be empty".to_string(),
        ));
    }
    Ok(parts.join("/"))
}

fn derive_key(backup_secret: &str, salt: &[u8]) -> [u8; KEY_LEN] {
    let mut key = [0u8; KEY_LEN];
    pbkdf2::pbkdf2_hmac::<Sha256>(backup_secret.as_bytes(), salt, KDF_ITERATIONS, &mut key);
    key
}

fn encrypt_backup(backup_secret: &str, aad: &[u8], plaintext: &[u8]) -> Result<String, ApiError> {
    let mut salt = [0u8; SALT_LEN];
    let mut nonce = [0u8; NONCE_LEN];
    OsRng.fill_bytes(&mut salt);
    OsRng.fill_bytes(&mut nonce);

    let key = derive_key(backup_secret, &salt);
    let cipher = Aes256Gcm::new_from_slice(&key).map_err(|e| ApiError::Api {
        reason: format!("Failed to initialize Bark backup cipher: {}", e),
    })?;
    let ciphertext = cipher
        .encrypt(
            Nonce::from_slice(&nonce),
            Payload {
                msg: plaintext,
                aad,
            },
        )
        .map_err(|_| ApiError::Api {
            reason: "Failed to encrypt Bark backup".to_string(),
        })?;

    let envelope = EncryptedBackupEnvelope {
        magic: BACKUP_MAGIC.to_string(),
        version: BACKUP_VERSION,
        cipher: CIPHER_NAME.to_string(),
        kdf: KDF_NAME.to_string(),
        kdf_iterations: KDF_ITERATIONS,
        salt: base64::encode(salt),
        nonce: base64::encode(nonce),
        aad: base64::encode(aad),
        ciphertext: base64::encode(ciphertext),
    };
    let envelope = serde_json::to_vec(&envelope)?;
    Ok(base64::encode(envelope))
}

fn decrypt_backup(backup_secret: &str, encrypted_data: &str) -> Result<BackupPlaintext, ApiError> {
    let envelope_bytes = base64::decode(encrypted_data.trim()).map_err(|e| {
        ApiError::InvalidInput(format!("Invalid Bark backup envelope encoding: {}", e))
    })?;
    let envelope: EncryptedBackupEnvelope = serde_json::from_slice(&envelope_bytes)?;

    if envelope.magic != BACKUP_MAGIC
        || envelope.version != BACKUP_VERSION
        || envelope.cipher != CIPHER_NAME
        || envelope.kdf != KDF_NAME
        || envelope.kdf_iterations != KDF_ITERATIONS
    {
        return Err(ApiError::InvalidInput(
            "Unsupported Bark backup format".to_string(),
        ));
    }

    let salt = base64::decode(envelope.salt)
        .map_err(|e| ApiError::InvalidInput(format!("Invalid Bark backup salt: {}", e)))?;
    let nonce = base64::decode(envelope.nonce)
        .map_err(|e| ApiError::InvalidInput(format!("Invalid Bark backup nonce: {}", e)))?;
    let aad = base64::decode(envelope.aad)
        .map_err(|e| ApiError::InvalidInput(format!("Invalid Bark backup metadata: {}", e)))?;
    let ciphertext = base64::decode(envelope.ciphertext)
        .map_err(|e| ApiError::InvalidInput(format!("Invalid Bark backup ciphertext: {}", e)))?;

    if salt.len() != SALT_LEN || nonce.len() != NONCE_LEN {
        return Err(ApiError::InvalidInput(
            "Invalid Bark backup cryptographic parameters".to_string(),
        ));
    }

    let key = derive_key(backup_secret, &salt);
    let cipher = Aes256Gcm::new_from_slice(&key).map_err(|e| ApiError::Api {
        reason: format!("Failed to initialize Bark backup cipher: {}", e),
    })?;
    let plaintext = cipher
        .decrypt(
            Nonce::from_slice(&nonce),
            Payload {
                msg: ciphertext.as_slice(),
                aad: aad.as_slice(),
            },
        )
        .map_err(|_| {
            ApiError::InvalidInput(
                "Unable to decrypt Bark backup. Check backup secret and backup data.".to_string(),
            )
        })?;

    serde_json::from_slice(&plaintext).map_err(ApiError::from)
}

fn validate_plaintext(plaintext: &BackupPlaintext) -> Result<(), ApiError> {
    if plaintext.magic != BACKUP_MAGIC
        || plaintext.version != BACKUP_VERSION
        || plaintext.info.version != BACKUP_VERSION
    {
        return Err(ApiError::InvalidInput(
            "Unsupported Bark backup payload".to_string(),
        ));
    }
    if plaintext.files.is_empty() {
        return Err(ApiError::InvalidInput(
            "Bark backup contains no storage files".to_string(),
        ));
    }
    if !plaintext
        .files
        .iter()
        .any(|file| file.path == BACKUP_DB_FILE)
    {
        return Err(ApiError::InvalidInput(format!(
            "Bark backup is missing {}",
            BACKUP_DB_FILE
        )));
    }
    Ok(())
}

fn snapshot_sha256(files: &[BackupFile]) -> String {
    let mut hasher = Sha256::new();
    for file in files {
        hasher.update(file.path.as_bytes());
        hasher.update([0]);
        hasher.update(file.sha256.as_bytes());
        hasher.update([0]);
    }
    hex::encode(hasher.finalize())
}

fn install_snapshot(
    storage_dir: &Path,
    files: &[BackupFile],
    overwrite_existing: bool,
) -> Result<(), ApiError> {
    if storage_dir.exists() && !storage_dir.is_dir() {
        return Err(ApiError::InvalidInput(format!(
            "Bark restore target is not a directory: {}",
            storage_dir.display()
        )));
    }
    if storage_dir.exists() && dir_has_entries(storage_dir)? && !overwrite_existing {
        return Err(ApiError::InvalidInput(
            "Bark restore target already has data; set overwrite_existing to true to replace it"
                .to_string(),
        ));
    }

    let parent = storage_dir.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).map_err(|e| ApiError::Api {
        reason: format!("Failed to create Bark restore parent directory: {}", e),
    })?;

    let staging_dir = staging_dir(storage_dir);
    if staging_dir.exists() {
        fs::remove_dir_all(&staging_dir).map_err(|e| ApiError::Api {
            reason: format!("Failed to clear Bark restore staging directory: {}", e),
        })?;
    }
    fs::create_dir_all(&staging_dir).map_err(|e| ApiError::Api {
        reason: format!("Failed to create Bark restore staging directory: {}", e),
    })?;

    if let Err(error) = write_snapshot_files(&staging_dir, files) {
        let _ = fs::remove_dir_all(&staging_dir);
        return Err(error);
    }

    if storage_dir.exists() {
        fs::remove_dir_all(storage_dir).map_err(|e| {
            let _ = fs::remove_dir_all(&staging_dir);
            ApiError::Api {
                reason: format!("Failed to replace existing Bark storage directory: {}", e),
            }
        })?;
    }
    fs::rename(&staging_dir, storage_dir).map_err(|e| {
        let _ = fs::remove_dir_all(&staging_dir);
        ApiError::Api {
            reason: format!("Failed to install Bark restore snapshot: {}", e),
        }
    })?;
    Ok(())
}

fn write_snapshot_files(staging_dir: &Path, files: &[BackupFile]) -> Result<(), ApiError> {
    for file in files {
        let target = safe_restore_path(staging_dir, &file.path)?;
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|e| ApiError::Api {
                reason: format!("Failed to create Bark restore directory: {}", e),
            })?;
        }
        let data = base64::decode(&file.data)
            .map_err(|e| ApiError::InvalidInput(format!("Invalid Bark backup file data: {}", e)))?;
        let actual_sha = hex::encode(Sha256::digest(&data));
        if actual_sha != file.sha256 {
            return Err(ApiError::InvalidInput(format!(
                "Bark backup file checksum mismatch: {}",
                file.path
            )));
        }
        fs::write(target, data).map_err(|e| ApiError::Api {
            reason: format!("Failed to write Bark restore file: {}", e),
        })?;
    }
    Ok(())
}

fn safe_restore_path(root: &Path, archive_path: &str) -> Result<PathBuf, ApiError> {
    let relative = Path::new(archive_path);
    if archive_path.is_empty() || relative.is_absolute() {
        return Err(ApiError::InvalidInput(
            "Bark backup contains an unsafe file path".to_string(),
        ));
    }

    let mut target = root.to_path_buf();
    for component in relative.components() {
        match component {
            Component::Normal(part) => target.push(part),
            _ => {
                return Err(ApiError::InvalidInput(
                    "Bark backup contains an unsafe file path".to_string(),
                ));
            }
        }
    }
    Ok(target)
}

fn dir_has_entries(path: &Path) -> Result<bool, ApiError> {
    let mut entries = fs::read_dir(path).map_err(|e| ApiError::Api {
        reason: format!("Failed to inspect Bark restore target: {}", e),
    })?;
    entries
        .next()
        .transpose()
        .map(|entry| entry.is_some())
        .map_err(|e| ApiError::Api {
            reason: format!("Failed to inspect Bark restore target: {}", e),
        })
}

fn staging_dir(storage_dir: &Path) -> PathBuf {
    let parent = storage_dir.parent().unwrap_or_else(|| Path::new("."));
    let name = storage_dir
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("bark-storage");
    parent.join(format!(".{}.restore-{}", name, uuid::Uuid::new_v4()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "lni-bark-backup-{}-{}",
            label,
            uuid::Uuid::new_v4()
        ))
    }

    fn test_info(file_count: i32, snapshot_sha256: String) -> BarkBackupInfo {
        BarkBackupInfo {
            version: BACKUP_VERSION,
            created_at: 1_700_000_000,
            wallet_fingerprint: "test-fingerprint".to_string(),
            network: "signet".to_string(),
            server_url: "https://ark.example".to_string(),
            esplora_url: Some("https://esplora.example".to_string()),
            file_count,
            snapshot_sha256,
        }
    }

    #[test]
    fn encrypted_backup_roundtrips_storage_files() {
        let source = temp_path("source");
        let restore = temp_path("restore");
        fs::create_dir_all(source.join("nested")).unwrap();
        fs::write(source.join(BACKUP_DB_FILE), b"wallet-state").unwrap();
        fs::write(source.join("nested/metadata.json"), br#"{"ok":true}"#).unwrap();

        let backup = create_encrypted_backup(
            &source,
            "backup secret",
            "test-fingerprint".to_string(),
            "signet".to_string(),
            "https://ark.example".to_string(),
            Some("https://esplora.example".to_string()),
        )
        .unwrap();

        let restored_info = restore_encrypted_backup(
            &restore,
            &backup,
            "backup secret",
            "signet",
            "https://ark.example",
            false,
        )
        .unwrap();

        assert_eq!(restored_info, backup.info);
        assert_eq!(
            fs::read(restore.join(BACKUP_DB_FILE)).unwrap(),
            b"wallet-state"
        );
        assert_eq!(
            fs::read(restore.join("nested/metadata.json")).unwrap(),
            br#"{"ok":true}"#
        );

        let _ = fs::remove_dir_all(source);
        let _ = fs::remove_dir_all(restore);
    }

    #[test]
    fn wrong_backup_secret_fails_decryption() {
        let source = temp_path("wrong-secret");
        fs::create_dir_all(&source).unwrap();
        fs::write(source.join(BACKUP_DB_FILE), b"wallet-state").unwrap();

        let backup = create_encrypted_backup(
            &source,
            "correct secret",
            "test-fingerprint".to_string(),
            "signet".to_string(),
            "https://ark.example".to_string(),
            None,
        )
        .unwrap();
        let restore = temp_path("wrong-secret-restore");
        let result = restore_encrypted_backup(
            &restore,
            &backup,
            "wrong secret",
            "signet",
            "https://ark.example",
            false,
        );

        assert!(matches!(result, Err(ApiError::InvalidInput(_))));

        let _ = fs::remove_dir_all(source);
        let _ = fs::remove_dir_all(restore);
    }

    #[test]
    fn restore_refuses_existing_storage_without_overwrite() {
        let source = temp_path("existing-source");
        let restore = temp_path("existing-restore");
        fs::create_dir_all(&source).unwrap();
        fs::create_dir_all(&restore).unwrap();
        fs::write(source.join(BACKUP_DB_FILE), b"wallet-state").unwrap();
        fs::write(restore.join("existing.txt"), b"do not replace").unwrap();

        let backup = create_encrypted_backup(
            &source,
            "backup secret",
            "test-fingerprint".to_string(),
            "signet".to_string(),
            "https://ark.example".to_string(),
            None,
        )
        .unwrap();
        let result = restore_encrypted_backup(
            &restore,
            &backup,
            "backup secret",
            "signet",
            "https://ark.example",
            false,
        );

        assert!(matches!(result, Err(ApiError::InvalidInput(_))));
        assert_eq!(
            fs::read(restore.join("existing.txt")).unwrap(),
            b"do not replace"
        );

        let _ = fs::remove_dir_all(source);
        let _ = fs::remove_dir_all(restore);
    }

    #[test]
    fn bark_backup_debug_redacts_encrypted_data() {
        let backup = BarkBackup {
            info: test_info(1, "abc".to_string()),
            encrypted_data: "super-secret-ciphertext".to_string(),
        };

        let debug = format!("{:?}", backup);
        assert!(debug.contains("<redacted>"));
        assert!(!debug.contains("super-secret-ciphertext"));
    }
}
