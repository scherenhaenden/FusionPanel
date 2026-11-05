#![forbid(unsafe_code)]

use ed25519_dalek::{Signer, SigningKey};
use rand::rngs::OsRng;
use std::fs;
use std::process::Child;
use std::time::Duration;
use tempfile::NamedTempFile;
use updater::{run_update, UpdateConfig, UpdaterError};

struct TestProcessGuard(Option<Child>);

impl Drop for TestProcessGuard {
    fn drop(&mut self) {
        if let Some(mut child) = self.0.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

fn create_keypair() -> SigningKey {
    SigningKey::generate(&mut OsRng)
}

fn create_valid_executable_script() -> Vec<u8> {
    b"#!/bin/sh\nsleep 10\n".to_vec()
}

#[test]
fn test_successful_update() {
    let mut mock_server = mockito::Server::new();

    let signing_key = create_keypair();
    let verifying_key = signing_key.verifying_key();
    let public_key_bytes = verifying_key.to_bytes();

    let new_binary_data = create_valid_executable_script();
    let signature = signing_key.sign(&new_binary_data);

    let bin_mock = mock_server
        .mock("GET", "/bin")
        .with_status(200)
        .with_body(new_binary_data.clone())
        .create();

    let sig_mock = mock_server
        .mock("GET", "/sig")
        .with_status(200)
        .with_body(signature.to_bytes())
        .create();

    let health_mock = mock_server.mock("GET", "/health").with_status(200).create();

    let target_file = NamedTempFile::new().unwrap();
    fs::write(target_file.path(), b"old_binary_content").unwrap();

    let config = UpdateConfig {
        binary_url: format!("{}/bin", mock_server.url()),
        signature_url: format!("{}/sig", mock_server.url()),
        public_key: public_key_bytes,
        target_path: target_file.path().to_path_buf(),
        health_check_url: format!("{}/health", mock_server.url()),
        health_check_timeout: Duration::from_secs(2),
    };

    let result = run_update(&config);
    assert!(result.is_ok());
    let _guard = TestProcessGuard(Some(result.unwrap()));

    let final_content = fs::read(target_file.path()).unwrap();
    assert_eq!(final_content, new_binary_data);

    let recovery_marker = target_file.path().with_extension("recovery-marker");
    assert!(
        !recovery_marker.exists(),
        "Recovery marker should be removed on success"
    );

    bin_mock.assert();
    sig_mock.assert();
    health_mock.assert();
}

#[test]
fn test_health_check_timeout_and_rollback() {
    let mut mock_server = mockito::Server::new();

    let signing_key = create_keypair();
    let verifying_key = signing_key.verifying_key();
    let public_key_bytes = verifying_key.to_bytes();

    let new_binary_data = create_valid_executable_script();
    let signature = signing_key.sign(&new_binary_data);

    let bin_mock = mock_server
        .mock("GET", "/bin")
        .with_status(200)
        .with_body(new_binary_data.clone())
        .create();

    let sig_mock = mock_server
        .mock("GET", "/sig")
        .with_status(200)
        .with_body(signature.to_bytes())
        .create();

    // Health check always fails (e.g. 500)
    let health_mock = mock_server
        .mock("GET", "/health")
        .with_status(500)
        .expect_at_least(1) // called multiple times
        .create();

    let target_file = NamedTempFile::new().unwrap();
    let old_binary_data = create_valid_executable_script();
    fs::write(target_file.path(), &old_binary_data).unwrap();

    let config = UpdateConfig {
        binary_url: format!("{}/bin", mock_server.url()),
        signature_url: format!("{}/sig", mock_server.url()),
        public_key: public_key_bytes,
        target_path: target_file.path().to_path_buf(),
        health_check_url: format!("{}/health", mock_server.url()),
        health_check_timeout: Duration::from_secs(2), // small timeout for tests
    };

    let result = run_update(&config);

    assert!(matches!(result, Err(UpdaterError::HealthCheckFailed)));

    // Rollback should restore original file
    let final_content = fs::read(target_file.path()).unwrap();
    assert_eq!(final_content, old_binary_data);

    let recovery_marker = target_file.path().with_extension("recovery-marker");
    assert!(
        !recovery_marker.exists(),
        "Recovery marker should be removed on clean rollback"
    );

    bin_mock.assert();
    sig_mock.assert();
    health_mock.assert();
}

#[test]
fn test_invalid_signature() {
    let mut mock_server = mockito::Server::new();

    let signing_key = create_keypair();
    let verifying_key = signing_key.verifying_key();
    let public_key_bytes = verifying_key.to_bytes();

    let new_binary_data = b"new_binary_content";
    // Bad signature
    let wrong_key = create_keypair();
    let bad_signature = wrong_key.sign(new_binary_data);

    let _bin_mock = mock_server
        .mock("GET", "/bin")
        .with_status(200)
        .with_body(new_binary_data)
        .create();

    let _sig_mock = mock_server
        .mock("GET", "/sig")
        .with_status(200)
        .with_body(bad_signature.to_bytes())
        .create();

    let target_file = NamedTempFile::new().unwrap();
    let old_binary_data = b"old_binary_content";
    fs::write(target_file.path(), old_binary_data).unwrap();

    let config = UpdateConfig {
        binary_url: format!("{}/bin", mock_server.url()),
        signature_url: format!("{}/sig", mock_server.url()),
        public_key: public_key_bytes,
        target_path: target_file.path().to_path_buf(),
        health_check_url: format!("{}/health", mock_server.url()),
        health_check_timeout: Duration::from_secs(2),
    };

    let result = run_update(&config);

    assert!(matches!(result, Err(UpdaterError::SignatureVerification)));

    // Ensure atomic swap didn't even start, original file untouched
    let final_content = fs::read(target_file.path()).unwrap();
    assert_eq!(final_content, old_binary_data);
}
