#![forbid(unsafe_code)]

use ed25519_dalek::{Signer, SigningKey};
use rand::rngs::OsRng;
use std::fs;
use std::process::Child;
use std::time::Duration;
use tempfile::{NamedTempFile, TempDir};
use updater::preflight::{
    run_preflight, verify_artifact, ArtifactVerification, PreflightError, PreflightRequest,
};
use updater::{run_update, run_update_with_preflight, UpdateConfig, UpdaterError};

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

fn base_preflight_request(target_path: std::path::PathBuf) -> PreflightRequest {
    PreflightRequest {
        candidate_id: "fusionpanel-1.2.1".to_string(),
        current_version: "1.2.0".to_string(),
        target_version: "1.2.1".to_string(),
        allow_reinstall: false,
        expected_os: "debian".to_string(),
        expected_arch: "x86_64".to_string(),
        actual_os: "debian".to_string(),
        actual_arch: "x86_64".to_string(),
        target_path,
        state_paths: vec![],
        required_bytes: 1024,
        available_bytes: Some(1024 * 1024),
    }
}

#[test]
fn test_preflight_rejects_major_version_jump_and_downgrade() {
    let target_file = NamedTempFile::new().unwrap();

    // Major version jump: 1.2.0 -> 2.0.0
    let mut jump_req = base_preflight_request(target_file.path().to_path_buf());
    jump_req.target_version = "2.0.0".to_string();
    let err = run_preflight(&jump_req).unwrap_err();
    assert!(matches!(
        err,
        PreflightError::UnsupportedSourceVersion { .. }
    ));

    // Downgrade: 1.2.0 -> 1.1.9
    let mut downgrade_req = base_preflight_request(target_file.path().to_path_buf());
    downgrade_req.target_version = "1.1.9".to_string();
    let err = run_preflight(&downgrade_req).unwrap_err();
    assert!(matches!(
        err,
        PreflightError::UnsupportedSourceVersion { .. }
    ));

    // Prerelease downgrade: 1.2.0 -> 1.2.0-rc.1
    let mut pre_downgrade_req = base_preflight_request(target_file.path().to_path_buf());
    pre_downgrade_req.target_version = "1.2.0-rc.1".to_string();
    let err = run_preflight(&pre_downgrade_req).unwrap_err();
    assert!(matches!(
        err,
        PreflightError::UnsupportedSourceVersion { .. }
    ));
}

#[test]
fn test_preflight_rejects_duplicate_version_unless_reinstall_allowed() {
    let target_file = NamedTempFile::new().unwrap();
    let mut req = base_preflight_request(target_file.path().to_path_buf());
    req.target_version = "1.2.0".to_string();

    let err = run_preflight(&req).unwrap_err();
    assert!(matches!(err, PreflightError::DuplicateVersion { .. }));

    req.allow_reinstall = true;
    assert!(run_preflight(&req).is_ok());
}

#[test]
fn test_preflight_rejects_corrupt_artifact_digest_and_signature() {
    let payload = b"fusionpanel-release-binary";
    // Wrong SHA-256 digest
    let wrong_digest = "0".repeat(64);
    let err = verify_artifact(payload, ArtifactVerification::Sha256Hex(&wrong_digest)).unwrap_err();
    assert!(matches!(err, PreflightError::ArtifactDigestMismatch { .. }));

    // Invalid digest format
    let err = verify_artifact(payload, ArtifactVerification::Sha256Hex("not-hex")).unwrap_err();
    assert!(matches!(err, PreflightError::InvalidDigestFormat(_)));

    // Corrupt Ed25519 signature
    let signing_key = create_keypair();
    let public_key = signing_key.verifying_key().to_bytes();
    let signature = signing_key.sign(payload);
    let err = verify_artifact(
        b"tampered-release-binary",
        ArtifactVerification::Ed25519 {
            signature: &signature.to_bytes(),
            public_key: &public_key,
        },
    )
    .unwrap_err();
    assert!(matches!(err, PreflightError::ArtifactSignatureInvalid(_)));

    // Valid Ed25519 signature passes
    assert!(verify_artifact(
        payload,
        ArtifactVerification::Ed25519 {
            signature: &signature.to_bytes(),
            public_key: &public_key,
        },
    )
    .is_ok());
}

#[test]
fn test_preflight_rejects_insufficient_disk_space() {
    let target_file = NamedTempFile::new().unwrap();
    let mut req = base_preflight_request(target_file.path().to_path_buf());
    req.required_bytes = 50 * 1024 * 1024;
    req.available_bytes = Some(10 * 1024 * 1024);

    let err = run_preflight(&req).unwrap_err();
    assert!(matches!(
        err,
        PreflightError::InsufficientDiskSpace {
            available_bytes: 10_485_760,
            required_bytes: 52_428_800,
            ..
        }
    ));

    // Also verify real `df -Pk` query works when available_bytes is None
    req.required_bytes = 1;
    req.available_bytes = None;
    assert!(run_preflight(&req).is_ok());
}

#[test]
fn test_preflight_validates_identity_platform_and_symlink_safety() {
    let dir = TempDir::new().unwrap();
    let target_path = dir.path().join("fusionpanel");
    fs::write(&target_path, b"installed").unwrap();

    // Path traversal in candidate_id
    let mut req = base_preflight_request(target_path.clone());
    req.candidate_id = "../evil".to_string();
    assert!(matches!(
        run_preflight(&req),
        Err(PreflightError::InvalidCandidateIdentity(_))
    ));

    // Incompatible platform
    let mut req = base_preflight_request(target_path.clone());
    req.expected_arch = "aarch64".to_string();
    assert!(matches!(
        run_preflight(&req),
        Err(PreflightError::IncompatiblePlatform { .. })
    ));

    // Symlink state path rejected
    let real_state = dir.path().join("real-state");
    fs::create_dir(&real_state).unwrap();
    let symlink_state = dir.path().join("state-link");
    std::os::unix::fs::symlink(&real_state, &symlink_state).unwrap();

    let mut req = base_preflight_request(target_path);
    req.state_paths = vec![symlink_state];
    assert!(matches!(
        run_preflight(&req),
        Err(PreflightError::UnsafePath { .. })
    ));
}

#[test]
fn test_failed_preflight_leaves_install_unchanged_and_skips_download() {
    let mut mock_server = mockito::Server::new();
    let bin_mock = mock_server
        .mock("GET", "/bin")
        .expect(0)
        .create();
    let sig_mock = mock_server
        .mock("GET", "/sig")
        .expect(0)
        .create();

    let target_file = NamedTempFile::new().unwrap();
    let original_bytes = b"active-install-v1.2.0";
    fs::write(target_file.path(), original_bytes).unwrap();

    let mut req = base_preflight_request(target_file.path().to_path_buf());
    req.target_version = "2.0.0".to_string(); // Major jump -> preflight must fail

    let config = UpdateConfig {
        binary_url: format!("{}/bin", mock_server.url()),
        signature_url: format!("{}/sig", mock_server.url()),
        public_key: [0u8; 32],
        target_path: target_file.path().to_path_buf(),
        health_check_url: format!("{}/health", mock_server.url()),
        health_check_timeout: Duration::from_secs(1),
    };

    let result = run_update_with_preflight(&config, &req);
    assert!(matches!(
        result,
        Err(UpdaterError::Preflight(
            PreflightError::UnsupportedSourceVersion { .. }
        ))
    ));

    // Active install must remain completely untouched
    assert_eq!(fs::read(target_file.path()).unwrap(), original_bytes);
    assert!(!target_file.path().with_extension("tmp").exists());
    assert!(!target_file.path().with_extension("backup").exists());
    assert!(!target_file.path().with_extension("recovery-marker").exists());

    bin_mock.assert();
    sig_mock.assert();
}
