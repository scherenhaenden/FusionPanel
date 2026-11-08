#![forbid(unsafe_code)]

use security::{SshWhitelistEngine, EngineError, CidrError};
use std::net::{IpAddr, Ipv4Addr};
use std::str::FromStr;

#[test]
fn test_port_validation() {
    let mut engine = SshWhitelistEngine::new();

    // Add valid port
    assert!(engine.add_port(22).is_ok());
    assert!(engine.add_port(65535).is_ok());

    // Invalid port 0
    let err = engine.add_port(0).unwrap_err();
    assert_eq!(err, EngineError::InvalidPort(0));
}

#[test]
fn test_cidr_validation() {
    let mut engine = SshWhitelistEngine::new();

    // Valid IPv4 CIDR
    assert!(engine.add_cidr("192.168.1.0/24").is_ok());

    // Valid IPv6 CIDR
    assert!(engine.add_cidr("2001:db8::/32").is_ok());

    // Invalid format
    let err = engine.add_cidr("192.168.1.0").unwrap_err();
    assert_eq!(err, EngineError::InvalidCidr(CidrError::InvalidFormat));

    // Invalid IP
    let err = engine.add_cidr("invalid/24").unwrap_err();
    assert_eq!(err, EngineError::InvalidCidr(CidrError::InvalidIp));

    // Invalid IPv4 prefix
    let err = engine.add_cidr("192.168.1.0/33").unwrap_err();
    assert_eq!(err, EngineError::InvalidCidr(CidrError::InvalidPrefix));

    // Invalid IPv6 prefix
    let err = engine.add_cidr("2001:db8::/129").unwrap_err();
    assert_eq!(err, EngineError::InvalidCidr(CidrError::InvalidPrefix));
}

#[test]
fn test_ip_allowlist_evaluation() {
    let mut engine = SshWhitelistEngine::new();

    // If empty, no IPs are allowed
    assert!(!engine.is_ip_allowed(&IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100))));

    engine.add_cidr("192.168.1.0/24").unwrap();
    engine.add_cidr("10.0.0.0/8").unwrap();
    engine.add_cidr("2001:db8::/32").unwrap();

    // IPv4 Evaluation
    assert!(engine.is_ip_allowed(&IpAddr::V4(Ipv4Addr::new(192, 168, 1, 10))));
    assert!(engine.is_ip_allowed(&IpAddr::V4(Ipv4Addr::new(192, 168, 1, 255))));
    assert!(!engine.is_ip_allowed(&IpAddr::V4(Ipv4Addr::new(192, 168, 2, 10))));

    assert!(engine.is_ip_allowed(&IpAddr::V4(Ipv4Addr::new(10, 20, 30, 40))));
    assert!(!engine.is_ip_allowed(&IpAddr::V4(Ipv4Addr::new(11, 0, 0, 1))));

    // IPv6 Evaluation
    assert!(engine.is_ip_allowed(&IpAddr::from_str("2001:db8:1234::1").unwrap()));
    assert!(!engine.is_ip_allowed(&IpAddr::from_str("2001:db9::1").unwrap()));
}

#[test]
fn test_sshd_config_generation() {
    let mut engine = SshWhitelistEngine::new();

    engine.add_port(22).unwrap();
    engine.add_port(2222).unwrap();

    engine.add_cidr("192.168.1.0/24").unwrap();
    engine.add_cidr("2001:db8::/32").unwrap();

    let config = engine.generate_sshd_config();

    assert!(config.contains("Port 22\n"));
    assert!(config.contains("Port 2222\n"));
    assert!(config.contains("Match Address 192.168.1.0/24,2001:db8::/32\n"));
}
