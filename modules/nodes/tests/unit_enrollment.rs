#![forbid(unsafe_code)]

#[path = "../src/enrollment.rs"]
mod enrollment;

use chrono::Duration;
use enrollment::{EnrollmentError, MasterEnrollment};

#[test]
fn test_token_generation_and_join() {
    let mut master = MasterEnrollment::new();
    let token = master.generate_token();
    let public_key = "test_pub_key";

    let result = master.join(&token, public_key);
    assert!(result.is_ok());

    let res = result.unwrap();
    assert_eq!(res.cert_bundle, "mtls_cert_bundle_for_test_pub_key");
    assert!(!res.node_id.is_nil());
}

#[test]
fn test_token_replay_rejection() {
    let mut master = MasterEnrollment::new();
    let token = master.generate_token();
    let public_key = "test_pub_key";

    // First join should succeed
    let result1 = master.join(&token, public_key);
    assert!(result1.is_ok());

    // Second join with the same token should fail
    let result2 = master.join(&token, public_key);
    assert!(matches!(result2, Err(EnrollmentError::TokenNotFound)));
}

#[test]
fn test_token_expiration() {
    let mut master = MasterEnrollment::new();
    // Generate a token that expired 1 minute ago
    let token = master.generate_token_with_ttl(Duration::minutes(-1));
    let public_key = "test_pub_key";

    let result = master.join(&token, public_key);
    assert!(matches!(result, Err(EnrollmentError::TokenExpired)));
}

#[test]
fn test_invalid_token() {
    let mut master = MasterEnrollment::new();
    let public_key = "test_pub_key";

    let result = master.join("non_existent_token", public_key);
    assert!(matches!(result, Err(EnrollmentError::TokenNotFound)));
}
