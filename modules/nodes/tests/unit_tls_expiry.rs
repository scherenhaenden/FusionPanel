#![forbid(unsafe_code)]

use chrono::{Duration, Utc};
use nodes::tls_expiry::{ExpirationStatus, TlsExpiryMonitor};
use std::process::Command;

fn generate_cert(days: i64) -> String {
    let output = Command::new("openssl")
        .arg("req")
        .arg("-x509")
        .arg("-newkey")
        .arg("rsa:2048")
        .arg("-keyout")
        .arg("/dev/null")
        .arg("-outform")
        .arg("PEM")
        .arg("-days")
        .arg(days.to_string())
        .arg("-nodes")
        .arg("-subj")
        .arg("/CN=test")
        .output()
        .expect("Failed to generate cert");
    String::from_utf8_lossy(&output.stdout).to_string()
}

#[test]
fn test_valid_cert() {
    let pem = generate_cert(365);
    let info = TlsExpiryMonitor::parse_pem(&pem).expect("Should parse successfully");

    let now = Utc::now() + Duration::days(1);
    assert_eq!(info.evaluate_status(now, 30), ExpirationStatus::Valid);
    assert!(info.remaining_duration(now).unwrap().as_secs() > 300 * 24 * 3600);
}

#[test]
fn test_expiring_soon_cert() {
    let pem = generate_cert(365);
    let info = TlsExpiryMonitor::parse_pem(&pem).expect("Should parse successfully");

    let now = info.not_after - Duration::days(15);
    assert_eq!(
        info.evaluate_status(now, 30),
        ExpirationStatus::ExpiringSoon
    );
    assert_eq!(info.evaluate_status(now, 10), ExpirationStatus::Valid);
}

#[test]
fn test_expired_cert() {
    let pem = generate_cert(365);
    let info = TlsExpiryMonitor::parse_pem(&pem).expect("Should parse successfully");

    let now = info.not_after + Duration::days(1);
    assert_eq!(info.evaluate_status(now, 30), ExpirationStatus::Expired);
    assert!(info.remaining_duration(now).is_none());
}

#[test]
fn test_malformed_pem() {
    let result = TlsExpiryMonitor::parse_pem("not a pem");
    assert!(result.is_err());
}
