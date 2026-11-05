#![forbid(unsafe_code)]

use std::path::{Component, Path, PathBuf};
use std::time::Duration;
use async_trait::async_trait;
use async_ssh2_lite::{AsyncSession, AsyncSftp};
use tokio::net::TcpStream;
use futures_util::io::{AsyncReadExt, AsyncWriteExt};
use crate::traits::{FileStat, StorageBackend, StorageError};

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

impl SftpStorageBackend {
    async fn connect_and_auth(&self) -> Result<AsyncSession<TcpStream>, StorageError> {
        let addr = format!("{}:{}", self.host, self.port);
        let stream = match &self.timeout_config {
            Some(tc) => tokio::time::timeout(tc.connect_timeout, TcpStream::connect(&addr)).await,
            None => Ok(TcpStream::connect(&addr).await),
        };

        let stream = match stream {
            Ok(Ok(s)) => s,
            Ok(Err(e)) => return Err(StorageError::Other(e.to_string())),
            Err(_) => return Err(StorageError::NetworkTimeout),
        };

        let mut session = match AsyncSession::new(stream, None) {
            Ok(s) => s,
            Err(e) => return Err(StorageError::Other(e.to_string())),
        };

        if let Err(e) = session.handshake().await {
            return Err(StorageError::Other(e.to_string()));
        }

        let auth_res = match &self.auth_method {
            AuthMethod::Password(pw) => {
                session.userauth_password(&self.username, pw).await
            }
            AuthMethod::PrivateKey { key, passphrase } => {
                session.userauth_pubkey_memory(
                    &self.username,
                    None,
                    key,
                    passphrase.as_deref(),
                ).await
            }
        };

        if let Err(_e) = auth_res {
            return Err(StorageError::AuthenticationFailed);
        }

        if !session.authenticated() {
            return Err(StorageError::AuthenticationFailed);
        }

        Ok(session)
    }

    async fn get_sftp(&self) -> Result<(AsyncSession<TcpStream>, AsyncSftp<TcpStream>), StorageError> {
        let session = self.connect_and_auth().await?;
        let sftp = match session.sftp().await {
            Ok(s) => s,
            Err(e) => return Err(StorageError::Other(e.to_string())),
        };
        Ok((session, sftp))
    }

    async fn throttle(&self, bytes_transferred: usize, start_time: tokio::time::Instant) {
        if let Some(config) = &self.throttle_config {
            let elapsed = start_time.elapsed();
            let expected_duration = Duration::from_secs_f64(bytes_transferred as f64 / config.max_bytes_per_second as f64);
            if expected_duration > elapsed {
                tokio::time::sleep(expected_duration - elapsed).await;
            }
        }
    }
}

#[async_trait]
impl StorageBackend for SftpStorageBackend {
    async fn upload(&self, path: &str, data: Vec<u8>) -> Result<(), StorageError> {
        let normalized = self.normalize_and_validate_path(Path::new(path))
            .map_err(|e| match e {
                SftpError::InvalidPath | SftpError::PathEscapeDetected => StorageError::Other(e.to_string()),
                _ => StorageError::Other("Invalid path".to_string()),
            })?;
            
        let (_session, sftp) = self.get_sftp().await?;
        let start_time = tokio::time::Instant::now();

        let mut file = match sftp.create(&normalized).await {
            Ok(f) => f,
            Err(e) => {
                let msg = e.to_string();
                if msg.contains("Permission denied") {
                    return Err(StorageError::PermissionDenied);
                } else {
                    return Err(StorageError::Other(msg));
                }
            }
        };

        let write_fut = file.write_all(&data);
        match &self.timeout_config {
            Some(tc) => {
                match tokio::time::timeout(tc.write_timeout, write_fut).await {
                    Ok(Ok(_)) => {},
                    Ok(Err(e)) => return Err(StorageError::Other(e.to_string())),
                    Err(_) => return Err(StorageError::NetworkTimeout),
                }
            },
            None => {
                match write_fut.await {
                    Ok(_) => {},
                    Err(e) => return Err(StorageError::Other(e.to_string())),
                }
            }
        }

        self.throttle(data.len(), start_time).await;
        Ok(())
    }

    async fn download(&self, path: &str) -> Result<Vec<u8>, StorageError> {
        let normalized = self.normalize_and_validate_path(Path::new(path))
            .map_err(|e| StorageError::Other(e.to_string()))?;

        let (_session, sftp) = self.get_sftp().await?;
        let start_time = tokio::time::Instant::now();

        let mut file = match sftp.open(&normalized).await {
            Ok(f) => f,
            Err(e) => {
                let msg = e.to_string();
                if msg.contains("no such file") || msg.contains("No such file") {
                    return Err(StorageError::NotFound);
                } else if msg.contains("Permission denied") {
                    return Err(StorageError::PermissionDenied);
                } else {
                    return Err(StorageError::Other(msg));
                }
            }
        };

        let mut buf = Vec::new();
        let read_fut = file.read_to_end(&mut buf);
        match &self.timeout_config {
            Some(tc) => {
                match tokio::time::timeout(tc.read_timeout, read_fut).await {
                    Ok(Ok(_)) => {},
                    Ok(Err(e)) => return Err(StorageError::Other(e.to_string())),
                    Err(_) => return Err(StorageError::NetworkTimeout),
                }
            },
            None => {
                match read_fut.await {
                    Ok(_) => {},
                    Err(e) => return Err(StorageError::Other(e.to_string())),
                }
            }
        }

        self.throttle(buf.len(), start_time).await;
        Ok(buf)
    }

    async fn delete(&self, path: &str) -> Result<(), StorageError> {
        let normalized = self.normalize_and_validate_path(Path::new(path))
            .map_err(|e| StorageError::Other(e.to_string()))?;

        let (_session, sftp) = self.get_sftp().await?;

        match sftp.unlink(&normalized).await {
            Ok(_) => Ok(()),
            Err(e) => {
                let msg = e.to_string();
                if msg.contains("no such file") || msg.contains("No such file") {
                    Err(StorageError::NotFound)
                } else if msg.contains("Permission denied") {
                    Err(StorageError::PermissionDenied)
                } else {
                    Err(StorageError::Other(msg))
                }
            }
        }
    }

    async fn list(&self, prefix: &str) -> Result<Vec<String>, StorageError> {
        // Find the deepest directory to list
        let prefix_path = Path::new(prefix);
        let prefix_str = prefix_path.to_string_lossy().to_string();
        
        let dir_to_list = if prefix.is_empty() || prefix.ends_with('/') {
            prefix.to_string()
        } else {
            prefix_path.parent().map(|p| p.to_string_lossy().to_string()).unwrap_or_else(|| "".to_string())
        };

        let normalized_dir = self.normalize_and_validate_path(Path::new(&dir_to_list))
            .map_err(|e| StorageError::Other(e.to_string()))?;

        let (_session, sftp) = self.get_sftp().await?;

        let read_dir_fut = sftp.readdir(&normalized_dir);
        let read_dir_res = match &self.timeout_config {
            Some(tc) => {
                match tokio::time::timeout(tc.read_timeout, read_dir_fut).await {
                    Ok(res) => res,
                    Err(_) => return Err(StorageError::NetworkTimeout),
                }
            },
            None => read_dir_fut.await
        };

        let mut results = Vec::new();
        match read_dir_res {
            Ok(entries) => {
                for (path, _stat) in entries {
                    if path.file_name() == Some(std::ffi::OsStr::new(".")) || path.file_name() == Some(std::ffi::OsStr::new("..")) {
                        continue;
                    }
                    
                    let filename = path.file_name().unwrap_or_default().to_string_lossy();
                    let relative_path = if dir_to_list.is_empty() {
                        filename.to_string()
                    } else if dir_to_list.ends_with('/') {
                        format!("{}{}", dir_to_list, filename)
                    } else {
                        format!("{}/{}", dir_to_list, filename)
                    };
                    
                    if relative_path.starts_with(&prefix_str) {
                        results.push(relative_path);
                    }
                }
            }
            Err(e) => {
                let msg = e.to_string();
                if msg.contains("no such file") || msg.contains("No such file") {
                    // Empty list if directory doesn't exist
                    return Ok(vec![]);
                } else if msg.contains("Permission denied") {
                    return Err(StorageError::PermissionDenied);
                } else {
                    return Err(StorageError::Other(msg));
                }
            }
        }
        
        results.sort();
        Ok(results)
    }

    async fn stat(&self, path: &str) -> Result<FileStat, StorageError> {
        let normalized = self.normalize_and_validate_path(Path::new(path))
            .map_err(|e| StorageError::Other(e.to_string()))?;

        let (_session, sftp) = self.get_sftp().await?;

        let stat_fut = sftp.stat(&normalized);
        let stat_res = match &self.timeout_config {
            Some(tc) => {
                match tokio::time::timeout(tc.read_timeout, stat_fut).await {
                    Ok(res) => res,
                    Err(_) => return Err(StorageError::NetworkTimeout),
                }
            },
            None => stat_fut.await
        };

        match stat_res {
            Ok(stat) => {
                Ok(FileStat {
                    size: stat.size.unwrap_or(0),
                })
            }
            Err(e) => {
                let msg = e.to_string();
                if msg.contains("no such file") || msg.contains("No such file") {
                    Err(StorageError::NotFound)
                } else if msg.contains("Permission denied") {
                    Err(StorageError::PermissionDenied)
                } else {
                    Err(StorageError::Other(msg))
                }
            }
        }
    }
}
