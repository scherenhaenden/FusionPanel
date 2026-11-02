#![forbid(unsafe_code)]

use std::path::{Component, Path, PathBuf};
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthMethod {
    Password(String),
    PrivateKey {
        key: String,
        passphrase: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThrottleConfig {
    pub max_bytes_per_second: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimeoutConfig {
    pub connect_timeout: Duration,
    pub read_timeout: Duration,
    pub write_timeout: Duration,
}

#[derive(Debug, PartialEq, Eq)]
pub enum SftpError {
    InvalidAuthParameters,
    PathEscapeDetected,
    InvalidPath,
}

impl std::fmt::Display for SftpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SftpError::InvalidAuthParameters => write!(f, "Authentication parameter is invalid"),
            SftpError::PathEscapeDetected => write!(
                f,
                "Path traversal detected or escape outside base directory"
            ),
            SftpError::InvalidPath => write!(f, "Invalid path"),
        }
    }
}

impl std::error::Error for SftpError {}

#[derive(Debug)]
pub struct SftpStorageBackend {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth_method: AuthMethod,
    pub base_dir: PathBuf,
    pub throttle_config: Option<ThrottleConfig>,
    pub timeout_config: Option<TimeoutConfig>,
}

impl SftpStorageBackend {
    pub fn new(
        host: String,
        port: u16,
        username: String,
        auth_method: AuthMethod,
        base_dir: PathBuf,
    ) -> Result<Self, SftpError> {
        match &auth_method {
            AuthMethod::Password(pw) if pw.is_empty() => {
                return Err(SftpError::InvalidAuthParameters)
            }
            AuthMethod::PrivateKey { key, .. } if key.is_empty() => {
                return Err(SftpError::InvalidAuthParameters)
            }
            _ => {}
        }

        Ok(Self {
            host,
            port,
            username,
            auth_method,
            base_dir,
            throttle_config: None,
            timeout_config: None,
        })
    }

    pub fn with_throttle(mut self, config: ThrottleConfig) -> Self {
        self.throttle_config = Some(config);
        self
    }

    pub fn with_timeouts(mut self, config: TimeoutConfig) -> Self {
        self.timeout_config = Some(config);
        self
    }

    pub fn normalize_and_validate_path(&self, requested_path: &Path) -> Result<PathBuf, SftpError> {
        if requested_path.is_absolute() {
            return Err(SftpError::InvalidPath);
        }

        let mut depth: isize = 0;
        for component in requested_path.components() {
            match component {
                Component::ParentDir => {
                    depth -= 1;
                    if depth < 0 {
                        return Err(SftpError::PathEscapeDetected);
                    }
                }
                Component::Normal(_) => {
                    depth += 1;
                }
                Component::CurDir => {}
                Component::RootDir | Component::Prefix(_) => {
                    return Err(SftpError::InvalidPath);
                }
            }
        }

        let mut normalized = self.base_dir.clone();
        for component in requested_path.components() {
            match component {
                Component::ParentDir => {
                    normalized.pop();
                }
                Component::Normal(c) => {
                    normalized.push(c);
                }
                _ => {}
            }
        }

        Ok(normalized)
    }
}
