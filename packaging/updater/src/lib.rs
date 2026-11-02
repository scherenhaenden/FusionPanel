#![forbid(unsafe_code)]

use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum UpdaterError {
    #[error("Download failed: {0}")]
    Download(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Signature verification failed")]
    SignatureVerification,
    #[error("Invalid signature format")]
    InvalidSignature,
    #[error("Health check failed, rolled back")]
    HealthCheckFailed,
    #[error("Health check failed, AND rollback failed: {0}")]
    RollbackFailed(String),
}

pub struct UpdateConfig {
    pub binary_url: String,
    pub signature_url: String,
    pub public_key: [u8; 32],
    pub target_path: PathBuf,
    pub health_check_url: String,
    pub health_check_timeout: Duration,
}

pub fn run_update(config: &UpdateConfig) -> Result<(), UpdaterError> {
    let client = reqwest::blocking::Client::new();

    // 1. Download binary
    let bin_resp = client
        .get(&config.binary_url)
        .send()
        .map_err(|e| UpdaterError::Download(e.to_string()))?;
    if !bin_resp.status().is_success() {
        return Err(UpdaterError::Download(
            "Binary download returned non-200".to_string(),
        ));
    }
    let bin_data = bin_resp
        .bytes()
        .map_err(|e| UpdaterError::Download(e.to_string()))?;

    // 2. Download signature
    let sig_resp = client
        .get(&config.signature_url)
        .send()
        .map_err(|e| UpdaterError::Download(e.to_string()))?;
    if !sig_resp.status().is_success() {
        return Err(UpdaterError::Download(
            "Signature download returned non-200".to_string(),
        ));
    }
    let sig_data = sig_resp
        .bytes()
        .map_err(|e| UpdaterError::Download(e.to_string()))?;

    // 3. Verify signature
    let public_key = VerifyingKey::from_bytes(&config.public_key)
        .map_err(|_| UpdaterError::SignatureVerification)?;

    let signature =
        Signature::try_from(sig_data.as_ref()).map_err(|_| UpdaterError::InvalidSignature)?;

    public_key
        .verify(&bin_data, &signature)
        .map_err(|_| UpdaterError::SignatureVerification)?;

    // 4. Atomic swap
    let backup_path = config.target_path.with_extension("backup");
    let temp_path = config.target_path.with_extension("tmp");

    fs::write(&temp_path, &bin_data)?;

    let has_backup = config.target_path.exists();
    if has_backup {
        fs::rename(&config.target_path, &backup_path)?;
    }

    if let Err(e) = fs::rename(&temp_path, &config.target_path) {
        if has_backup {
            let _ = fs::rename(&backup_path, &config.target_path);
        }
        let _ = fs::remove_file(&temp_path);
        return Err(e.into());
    }

    // 5. Health check
    let start_time = Instant::now();
    let mut healthy = false;

    while start_time.elapsed() < config.health_check_timeout {
        if let Ok(resp) = client.get(&config.health_check_url).send() {
            if resp.status().is_success() {
                healthy = true;
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(500));
    }

    if !healthy {
        // Rollback
        if has_backup {
            if let Err(e) = fs::rename(&backup_path, &config.target_path) {
                return Err(UpdaterError::RollbackFailed(e.to_string()));
            }
        } else {
            let _ = fs::remove_file(&config.target_path);
        }
        return Err(UpdaterError::HealthCheckFailed);
    }

    // Cleanup backup on success
    if has_backup {
        let _ = fs::remove_file(backup_path);
    }

    Ok(())
}
