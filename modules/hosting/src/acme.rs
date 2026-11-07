#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};
use std::fs::{self, File};
use std::io::Write;
use std::process::Command;
use std::time::Duration;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AcmeError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("OpenSSL execution error: {0}")]
    OpenSslCommand(String),
    #[error("OpenSSL returned an error: {0}")]
    OpenSsl(String),
    #[error("Invalid token")]
    InvalidToken,
}

pub type Result<T> = std::result::Result<T, AcmeError>;

pub struct AcmeResponder {
    webroot: PathBuf,
}

impl AcmeResponder {
    pub fn new(webroot: impl AsRef<Path>) -> Self {
        Self {
            webroot: webroot.as_ref().to_path_buf(),
        }
    }

    /// Generates a .well-known/acme-challenge/<token> file with the provided content.
    pub fn generate_challenge(&self, token: &str, content: &str) -> Result<PathBuf> {
        if !is_valid_token(token) {
            return Err(AcmeError::InvalidToken);
        }

        let challenge_dir = self.webroot.join(".well-known").join("acme-challenge");
        fs::create_dir_all(&challenge_dir)?;

        let challenge_path = challenge_dir.join(token);
        let mut file = File::create(&challenge_path)?;
        file.write_all(content.as_bytes())?;

        Ok(challenge_path)
    }

    /// Validates that the challenge file exists and matches the expected content.
    pub fn validate_challenge(&self, token: &str, expected_content: &str) -> Result<bool> {
        if !is_valid_token(token) {
            return Err(AcmeError::InvalidToken);
        }

        let challenge_path = self.webroot.join(".well-known").join("acme-challenge").join(token);
        if !challenge_path.exists() {
            return Ok(false);
        }

        let content = fs::read_to_string(&challenge_path)?;
        Ok(content.trim() == expected_content.trim())
    }

    /// Cleans up a challenge file.
    pub fn remove_challenge(&self, token: &str) -> Result<()> {
        if !is_valid_token(token) {
            return Err(AcmeError::InvalidToken);
        }

        let challenge_path = self.webroot.join(".well-known").join("acme-challenge").join(token);
        if challenge_path.exists() {
            fs::remove_file(challenge_path)?;
        }
        Ok(())
    }
}

fn is_valid_token(token: &str) -> bool {
    !token.is_empty() && token.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

pub struct CertMonitor {
    cert_path: PathBuf,
}

impl CertMonitor {
    pub fn new(cert_path: impl AsRef<Path>) -> Self {
        Self {
            cert_path: cert_path.as_ref().to_path_buf(),
        }
    }

    /// Checks if the certificate will expire within the given duration.
    pub fn needs_renewal(&self, threshold: Duration) -> Result<bool> {
        if !self.cert_path.exists() {
            return Err(AcmeError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Certificate file not found",
            )));
        }

        let seconds = threshold.as_secs().to_string();
        let output = Command::new("openssl")
            .arg("x509")
            .arg("-checkend")
            .arg(&seconds)
            .arg("-noout")
            .arg("-in")
            .arg(&self.cert_path)
            .output()
            .map_err(|e| AcmeError::OpenSslCommand(e.to_string()))?;

        if output.status.success() {
            Ok(false)
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            // openssl x509 -checkend exits with 1 if the certificate will expire.
            if output.status.code() == Some(1) {
                Ok(true)
            } else {
                Err(AcmeError::OpenSsl(stderr.into_owned()))
            }
        }
    }
}
