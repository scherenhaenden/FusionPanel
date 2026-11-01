use thiserror::Error;

#[derive(Error, Debug)]
pub enum BackupError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Serialization error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("Unsafe path in archive: {0}")]
    UnsafePath(String),
    #[error("Checksum mismatch for file {path}: expected {expected}, got {actual}")]
    ChecksumMismatch {
        path: String,
        expected: String,
        actual: String,
    },
    #[error("Manifest not found in backup")]
    ManifestNotFound,
    #[error("System time error: {0}")]
    TimeError(#[from] std::time::SystemTimeError),
}
