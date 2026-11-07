use std::fs;
use std::time::Duration;
use tempfile::TempDir;
use hosting::acme::{AcmeResponder, CertMonitor};
use std::process::Command;

#[test]
fn test_generate_and_validate_challenge() {
    let temp_dir = TempDir::new().unwrap();
    let responder = AcmeResponder::new(temp_dir.path());

    let token = "test_token";
    let content = "test_content";

    // Generate
    let path = responder.generate_challenge(token, content).unwrap();
    assert!(path.exists());
    assert_eq!(fs::read_to_string(&path).unwrap(), content);

    // Validate
    assert!(responder.validate_challenge(token, content).unwrap());
    assert!(!responder.validate_challenge(token, "wrong_content").unwrap());

    // Clean up
    responder.remove_challenge(token).unwrap();
    assert!(!path.exists());
}

#[test]
fn test_invalid_token() {
    let temp_dir = TempDir::new().unwrap();
    let responder = AcmeResponder::new(temp_dir.path());

    let invalid_tokens = vec!["", "token with spaces", "../token", "token/"];

    for token in invalid_tokens {
        assert!(responder.generate_challenge(token, "content").is_err());
        assert!(responder.validate_challenge(token, "content").is_err());
        assert!(responder.remove_challenge(token).is_err());
    }
}

#[test]
fn test_cert_monitor_needs_renewal() {
    let temp_dir = TempDir::new().unwrap();
    let cert_path = temp_dir.path().join("cert.pem");
    let key_path = temp_dir.path().join("key.pem");

    // Generate a test certificate valid for 1 day
    let status = Command::new("openssl")
        .arg("req")
        .arg("-x509")
        .arg("-newkey")
        .arg("rsa:2048")
        .arg("-keyout")
        .arg(&key_path)
        .arg("-out")
        .arg(&cert_path)
        .arg("-days")
        .arg("1")
        .arg("-nodes")
        .arg("-subj")
        .arg("/CN=test.local")
        .status()
        .unwrap();

    assert!(status.success(), "Failed to generate test certificate");

    let monitor = CertMonitor::new(&cert_path);

    // Certificate is valid for 1 day, so it shouldn't need renewal in 1 hour
    let needs_renewal_soon = monitor.needs_renewal(Duration::from_secs(3600)).unwrap();
    assert!(!needs_renewal_soon);

    // Certificate is valid for 1 day, so it WILL need renewal in 2 days
    let needs_renewal_later = monitor.needs_renewal(Duration::from_secs(2 * 24 * 3600)).unwrap();
    assert!(needs_renewal_later);
}

#[test]
fn test_cert_monitor_file_not_found() {
    let temp_dir = TempDir::new().unwrap();
    let monitor = CertMonitor::new(temp_dir.path().join("nonexistent.pem"));

    assert!(monitor.needs_renewal(Duration::from_secs(3600)).is_err());
}
