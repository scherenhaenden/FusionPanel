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
    #[error("Source directory does not exist: {0}")]
    SourceNotFound(String),
    #[error("Destination path cannot be inside source directory")]
    DestinationInsideSource,
    #[error("File size mismatch for {path}: expected {expected}, got {actual}")]
    SizeMismatch {
        path: String,
        expected: u64,
        actual: u64,
    },
    #[error("Duplicate manifest found in archive")]
    DuplicateManifest,
    #[error("File in archive not in manifest: {0}")]
    UnexpectedFile(String),
    #[error("Missing files from manifest")]
    MissingFiles,
    #[error("Duplicate file in archive: {0}")]
    DuplicateFile(String),
}
