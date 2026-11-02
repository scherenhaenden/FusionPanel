#![forbid(unsafe_code)]

#[path = "../src/sftp/mod.rs"]
mod sftp;

use backup_destinations::sftp::{AuthMethod, SftpError, SftpStorageBackend};
use std::path::{Path, PathBuf};

#[test]
fn test_auth_parameter_validation() {
    // Valid password
    let backend = SftpStorageBackend::new(
        "localhost".to_string(),
        22,
        "user".to_string(),
        AuthMethod::Password("pass".to_string()),
        PathBuf::from("/base"),
    );
    assert!(backend.is_ok());

    // Empty password
    let backend = SftpStorageBackend::new(
        "localhost".to_string(),
        22,
        "user".to_string(),
        AuthMethod::Password("".to_string()),
        PathBuf::from("/base"),
    );
    assert_eq!(backend.unwrap_err(), SftpError::InvalidAuthParameters);

    // Valid private key
    let backend = SftpStorageBackend::new(
        "localhost".to_string(),
        22,
        "user".to_string(),
        AuthMethod::PrivateKey {
            key: "key_data".to_string(),
            passphrase: None,
        },
        PathBuf::from("/base"),
    );
    assert!(backend.is_ok());

    // Empty private key
    let backend = SftpStorageBackend::new(
        "localhost".to_string(),
        22,
        "user".to_string(),
        AuthMethod::PrivateKey {
            key: "".to_string(),
            passphrase: None,
        },
        PathBuf::from("/base"),
    );
    assert_eq!(backend.unwrap_err(), SftpError::InvalidAuthParameters);
}

#[test]
fn test_path_normalization_valid() {
    let backend = SftpStorageBackend::new(
        "localhost".to_string(),
        22,
        "user".to_string(),
        AuthMethod::Password("pass".to_string()),
        PathBuf::from("/var/backups"),
    )
    .unwrap();

    let path = Path::new("subdir/file.txt");
    let result = backend.normalize_and_validate_path(path).unwrap();
    assert_eq!(result, PathBuf::from("/var/backups/subdir/file.txt"));

    let path = Path::new("subdir/./file.txt");
    let result = backend.normalize_and_validate_path(path).unwrap();
    assert_eq!(result, PathBuf::from("/var/backups/subdir/file.txt"));

    let path = Path::new("subdir/../file.txt");
    let result = backend.normalize_and_validate_path(path).unwrap();
    assert_eq!(result, PathBuf::from("/var/backups/file.txt"));
}

#[test]
fn test_path_normalization_escape() {
    let backend = SftpStorageBackend::new(
        "localhost".to_string(),
        22,
        "user".to_string(),
        AuthMethod::Password("pass".to_string()),
        PathBuf::from("/var/backups"),
    )
    .unwrap();

    // Escape beyond base directory
    let path = Path::new("../file.txt");
    let err = backend.normalize_and_validate_path(path).unwrap_err();
    assert_eq!(err, SftpError::PathEscapeDetected);

    let path = Path::new("subdir/../../file.txt");
    let err = backend.normalize_and_validate_path(path).unwrap_err();
    assert_eq!(err, SftpError::PathEscapeDetected);

    // Absolute path is rejected
    let path = Path::new("/etc/passwd");
    let err = backend.normalize_and_validate_path(path).unwrap_err();
    assert_eq!(err, SftpError::InvalidPath);
}

#[test]
fn test_config_builders() {
    use backup_destinations::sftp::{ThrottleConfig, TimeoutConfig};
    use std::time::Duration;

    let backend = SftpStorageBackend::new(
        "localhost".to_string(),
        22,
        "user".to_string(),
        AuthMethod::Password("pass".to_string()),
        PathBuf::from("/base"),
    )
    .unwrap()
    .with_throttle(ThrottleConfig {
        max_bytes_per_second: 1024,
    })
    .with_timeouts(TimeoutConfig {
        connect_timeout: Duration::from_secs(10),
        read_timeout: Duration::from_secs(30),
        write_timeout: Duration::from_secs(30),
    });

    assert_eq!(backend.throttle_config.unwrap().max_bytes_per_second, 1024);
    assert_eq!(
        backend.timeout_config.unwrap().connect_timeout,
        Duration::from_secs(10)
    );
    assert_eq!(backend.host, "localhost");
    assert_eq!(backend.port, 22);
    assert_eq!(backend.username, "user");
    assert_eq!(
        backend.auth_method,
        AuthMethod::Password("pass".to_string())
    );
}
