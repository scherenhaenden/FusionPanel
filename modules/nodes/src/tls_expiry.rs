#![forbid(unsafe_code)]

use chrono::{DateTime, NaiveDateTime, Utc};
use std::process::{Command, Stdio};
use std::io::Write;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum TlsExpiryError {
    #[error("Failed to execute openssl command: {0}")]
    CommandError(#[from] std::io::Error),
    #[error("OpenSSL command failed: {0}")]
    OpenSslError(String),
    #[error("Failed to parse date string: {0}")]
    ParseError(#[from] chrono::ParseError),
    #[error("Missing or malformed dates in OpenSSL output")]
    MissingDates,
}

#[derive(Debug, PartialEq)]
pub enum ExpirationStatus {
    Valid,
    ExpiringSoon,
    Expired,
}

#[derive(Debug)]
pub struct TlsExpiryInfo {
    pub not_before: DateTime<Utc>,
    pub not_after: DateTime<Utc>,
}

impl TlsExpiryInfo {
    pub fn remaining_duration(&self, now: DateTime<Utc>) -> Option<std::time::Duration> {
        if self.not_after > now {
            let duration = self.not_after.signed_duration_since(now);
            Some(duration.to_std().unwrap_or(std::time::Duration::ZERO))
        } else {
            None
        }
    }

    pub fn evaluate_status(&self, now: DateTime<Utc>, warning_days: i64) -> ExpirationStatus {
        if now >= self.not_after {
            ExpirationStatus::Expired
        } else if now < self.not_before {
            ExpirationStatus::Expired
        } else {
            let duration = self.not_after.signed_duration_since(now);
            if duration.num_days() <= warning_days {
                ExpirationStatus::ExpiringSoon
            } else {
                ExpirationStatus::Valid
            }
        }
    }
}

pub struct TlsExpiryMonitor;

impl TlsExpiryMonitor {
    /// Extracts a PEM certificate string and invokes openssl to find validity timestamps.
    /// This uses standard system tools since adding dependencies via Cargo.toml is restricted.
    pub fn parse_pem(pem: &str) -> Result<TlsExpiryInfo, TlsExpiryError> {
        let mut child = Command::new("openssl")
            .arg("x509")
            .arg("-noout")
            .arg("-dates")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(pem.as_bytes())?;
        }

        let output = child.wait_with_output()?;
        
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            return Err(TlsExpiryError::OpenSslError(stderr));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        
        let mut not_before = None;
        let mut not_after = None;

        for line in stdout.lines() {
            if let Some(rest) = line.strip_prefix("notBefore=") {
                let dt = NaiveDateTime::parse_from_str(rest, "%b %e %H:%M:%S %Y %Z")
                    .or_else(|_| NaiveDateTime::parse_from_str(rest, "%b %e %H:%M:%S %Y GMT"))?;
                not_before = Some(DateTime::<Utc>::from_naive_utc_and_offset(dt, Utc));
            } else if let Some(rest) = line.strip_prefix("notAfter=") {
                let dt = NaiveDateTime::parse_from_str(rest, "%b %e %H:%M:%S %Y %Z")
                    .or_else(|_| NaiveDateTime::parse_from_str(rest, "%b %e %H:%M:%S %Y GMT"))?;
                not_after = Some(DateTime::<Utc>::from_naive_utc_and_offset(dt, Utc));
            }
        }

        match (not_before, not_after) {
            (Some(not_before), Some(not_after)) => Ok(TlsExpiryInfo { not_before, not_after }),
            _ => Err(TlsExpiryError::MissingDates),
        }
    }
}
