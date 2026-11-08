#![forbid(unsafe_code)]

use dns::axfr_acl::{AxfrAuthorizer, AxfrError};
use std::net::IpAddr;
use std::str::FromStr;

#[test]
fn test_unauthorized_ip() {
    let mut auth = AxfrAuthorizer::new();
    auth.add_allowed_cidr("192.168.1.0/24").unwrap();

    // Allowed
    assert!(auth.is_ip_allowed(IpAddr::from_str("192.168.1.100").unwrap()));
    // Unauthorized
    assert!(!auth.is_ip_allowed(IpAddr::from_str("192.168.2.100").unwrap()));
    assert!(!auth.is_ip_allowed(IpAddr::from_str("10.0.0.1").unwrap()));
}

#[test]
fn test_tsig_valid_signature() {
    let mut auth = AxfrAuthorizer::new();
    // Key: "secretkey" -> base64: "c2VjcmV0a2V5"
    auth.add_tsig_key("key1", "c2VjcmV0a2V5").unwrap();

    let message_data = b"dns_message_data";
    // MAC calculated for key "secretkey" and data "dns_message_data" using HMAC-SHA256
    let valid_mac: [u8; 32] = [173, 173, 131, 101, 149, 180, 85, 38, 74, 203, 237, 44, 71, 246, 61, 41, 121, 220, 155, 101, 122, 181, 183, 227, 253, 62, 2, 179, 0, 1, 136, 60];

    let result = auth.validate_tsig(
        "key1",
        &valid_mac,
        message_data,
        1000,
        1000,
        300
    );
    assert_eq!(result, Ok(()));

    // Invalid MAC
    let mut invalid_mac = valid_mac;
    invalid_mac[0] = 0;

    let result = auth.validate_tsig(
        "key1",
        &invalid_mac,
        message_data,
        1000,
        1000,
        300
    );
    assert_eq!(result, Err(AxfrError::InvalidSignature));
}

#[test]
fn test_tsig_expired_timestamp() {
    let mut auth = AxfrAuthorizer::new();
    auth.add_tsig_key("key1", "c2VjcmV0a2V5").unwrap();

    let result = auth.validate_tsig(
        "key1",
        b"some_mac",
        b"data",
        1000,
        1500,
        300
    );
    assert_eq!(result, Err(AxfrError::TimeFudgeExceeded));
}

#[test]
fn test_tsig_unrecognized_key() {
    let auth = AxfrAuthorizer::new();

    let result = auth.validate_tsig(
        "unknown_key",
        b"some_mac",
        b"data",
        1000,
        1000,
        300
    );
    assert_eq!(result, Err(AxfrError::KeyNotFound));
}
