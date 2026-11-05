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

use std::os::unix::fs::PermissionsExt;
use std::process::Command;

pub fn run_update(config: &UpdateConfig) -> Result<std::process::Child, UpdaterError> {
    let client = reqwest::blocking::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(300))
        .build()
        .map_err(|e| UpdaterError::Download(e.to_string()))?;

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
    let recovery_marker = config.target_path.with_extension("recovery-marker");

    fs::write(&temp_path, &bin_data)?;
    fs::set_permissions(&temp_path, fs::Permissions::from_mode(0o755))?;

    let has_backup = config.target_path.exists();

    fs::write(&recovery_marker, b"")?;

    if has_backup {
        fs::rename(&config.target_path, &backup_path)?;
    }

    if let Err(e) = fs::rename(&temp_path, &config.target_path) {
        if has_backup {
            let _ = fs::rename(&backup_path, &config.target_path);
        }
        let _ = fs::remove_file(&temp_path);
        let _ = fs::remove_file(&recovery_marker);
        return Err(e.into());
    }

    // 5. Spawn candidate
    let mut child = match Command::new(&config.target_path).spawn() {
        Ok(c) => c,
        Err(e) => {
            // Rollback immediately if spawn fails
            if has_backup {
                let _ = fs::rename(&backup_path, &config.target_path);
            } else {
                let _ = fs::remove_file(&config.target_path);
            }
            let _ = fs::remove_file(&recovery_marker);
            return Err(UpdaterError::Io(e));
        }
    };

    // 6. Health check
    let start_time = Instant::now();
    let mut healthy = false;

    while start_time.elapsed() < config.health_check_timeout {
        if let Ok(Some(_status)) = child.try_wait() {
            // Process exited prematurely
            break;
        }

        let remaining = config
            .health_check_timeout
            .saturating_sub(start_time.elapsed());
        if remaining.is_zero() {
            break;
        }

        if let Ok(resp) = client
            .get(&config.health_check_url)
            .timeout(remaining)
            .send()
        {
            if resp.status().is_success() {
                healthy = true;
                break;
            }
        }

        let remaining = config
            .health_check_timeout
            .saturating_sub(start_time.elapsed());
        if remaining.is_zero() {
            break;
        }
        std::thread::sleep(Duration::from_millis(500).min(remaining));
    }

    if !healthy {
        let _ = child.kill();
        let _ = child.wait();

        // Rollback
        if has_backup {
            if let Err(e) = fs::rename(&backup_path, &config.target_path) {
                return Err(UpdaterError::RollbackFailed(e.to_string()));
            }
        } else {
            let _ = fs::remove_file(&config.target_path);
        }
        let _ = fs::remove_file(&recovery_marker);
        return Err(UpdaterError::HealthCheckFailed);
    }

    // Cleanup backup on success
    if has_backup {
        let _ = fs::remove_file(backup_path);
    }
    let _ = fs::remove_file(&recovery_marker);

    Ok(child)
}
