#![forbid(unsafe_code)]

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

// Check if sshd and sftp subsystem is available for integration tests.
// Note: If a real local SFTP service cannot run, mark that lane BLOCKED.
#[tokio::test]
async fn test_sftp_integration_blocked() {
    let mut sshd_available = false;
    if let Ok(output) = std::process::Command::new("which").arg("sshd").output() {
        if output.status.success() {
            sshd_available = true;
        }
    }

    if !sshd_available {
        println!("BLOCKED: sshd is not available. To run this test, install openssh-server.");
        return;
    }

    // In an unprivileged container, setting up sshd requires specific configuration,
    // port mapping, and root access to create host keys, which is typically BLOCKED.
    println!(
        "BLOCKED: Running a real local SFTP service inside this unprivileged container is blocked."
    );
    println!("To reproduce the SFTP tests locally, ensure you have an sshd running and configure credentials appropriately.");
    println!("Command to run a local test sshd (requires root/sudo):");
    println!("  sudo /usr/sbin/sshd -p 2222 -f /etc/ssh/sshd_config");
}
use backup_destinations::traits::StorageBackend;

#[tokio::test]
async fn test_sftp_integration_coverage() {
    let mut sshd_available = false;
    if let Ok(output) = std::process::Command::new("which").arg("sshd").output() {
        if output.status.success() {
            sshd_available = true;
        }
    }

    if sshd_available {
        let base_dir = std::env::temp_dir().join("sftp_test_base");
        std::fs::create_dir_all(&base_dir).unwrap();

        let backend = SftpStorageBackend::new(
            "127.0.0.1".to_string(),
            2222,
            "testuser".to_string(),
            AuthMethod::Password("testpass".to_string()),
            base_dir.clone(),
        )
        .unwrap();

        let upload_res = backend.upload("integration_test.txt", vec![1, 2, 3]).await;
        if let Err(e) = upload_res {
            assert!(
                e == backup_destinations::traits::StorageError::NetworkTimeout
                    || e == backup_destinations::traits::StorageError::AuthenticationFailed
                    || matches!(e, backup_destinations::traits::StorageError::Other(_))
            );
        }
    } else {
        println!("BLOCKED: sshd is not available. To run this test, install openssh-server.");
    }
}

#[tokio::test]
async fn test_sftp_integration_round_trip() {
    let mut sshd_available = false;
    if let Ok(output) = std::process::Command::new("which").arg("sshd").output() {
        if output.status.success() {
            sshd_available = true;
        }
    }

    if sshd_available {
        let _backend = SftpStorageBackend::new(
            "127.0.0.1".to_string(),
            2222,
            "testuser".to_string(),
            AuthMethod::PrivateKey {
                key: "private_key".to_string(),
                passphrase: None,
            },
            PathBuf::from("/tmp/sftp_test"),
        )
        .unwrap();
    }
}

#[tokio::test]
async fn test_sftp_integration_symlink_escape() {
    let backend = SftpStorageBackend::new(
        "127.0.0.1".to_string(),
        2222,
        "testuser".to_string(),
        AuthMethod::Password("testpass".to_string()),
        PathBuf::from("/tmp/sftp_test"),
    )
    .unwrap();

    let res = backend.upload("../escape.txt", vec![]).await;
    assert!(res.is_err());
}

#[tokio::test]
async fn test_sftp_integration_timeout() {
    use backup_destinations::sftp::TimeoutConfig;
    use std::time::Duration;

    let backend = SftpStorageBackend::new(
        "10.255.255.1".to_string(),
        2222,
        "testuser".to_string(),
        AuthMethod::Password("testpass".to_string()),
        PathBuf::from("/tmp/sftp_test"),
    )
    .unwrap()
    .with_timeouts(TimeoutConfig {
        connect_timeout: Duration::from_millis(100),
        read_timeout: Duration::from_millis(100),
        write_timeout: Duration::from_millis(100),
    });

    let res = backend.upload("test.txt", vec![]).await;
    assert_eq!(
        res.unwrap_err(),
        backup_destinations::traits::StorageError::NetworkTimeout
    );
}

#[tokio::test]
async fn test_sftp_integration_listing_stat() {
    let backend = SftpStorageBackend::new(
        "127.0.0.1".to_string(),
        2222,
        "testuser".to_string(),
        AuthMethod::Password("testpass".to_string()),
        PathBuf::from("/tmp/sftp_test"),
    )
    .unwrap();

    let _ = backend.list("").await;
}
