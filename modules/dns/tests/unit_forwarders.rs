#![forbid(unsafe_code)]

use dns::forwarders::{BindForwarderConfig, ConfigError, ForwardPolicy};

#[test]
fn test_add_forwarder_ipv4() {
    let mut config = BindForwarderConfig::default();
    assert!(config.add_forwarder("8.8.8.8").is_ok());
    assert_eq!(config.forwarders.len(), 1);
}

#[test]
fn test_add_forwarder_ipv6() {
    let mut config = BindForwarderConfig::default();
    assert!(config.add_forwarder("2001:4860:4860::8888").is_ok());
    assert_eq!(config.forwarders.len(), 1);
}

#[test]
fn test_add_forwarder_invalid() {
    let mut config = BindForwarderConfig::default();
    assert_eq!(config.add_forwarder("invalid.ip"), Err(ConfigError::InvalidIp));
    assert_eq!(config.add_forwarder("256.256.256.256"), Err(ConfigError::InvalidIp));
}

#[test]
fn test_add_forwarder_syntax_injection() {
    let mut config = BindForwarderConfig::default();
    assert_eq!(config.add_forwarder("8.8.8.8; allow-transfer { any; };"), Err(ConfigError::SyntaxInjection));
    assert_eq!(config.add_forwarder("8.8.8.8\n"), Err(ConfigError::SyntaxInjection));
}

#[test]
fn test_add_recursion_acl_ip() {
    let mut config = BindForwarderConfig::default();
    assert!(config.add_recursion_acl("192.168.1.1").is_ok());
    assert!(config.add_recursion_acl("2001:db8::1").is_ok());
    assert_eq!(config.allow_recursion.len(), 2);
}

#[test]
fn test_add_recursion_acl_cidr() {
    let mut config = BindForwarderConfig::default();
    assert!(config.add_recursion_acl("192.168.1.0/24").is_ok());
    assert!(config.add_recursion_acl("2001:db8::/32").is_ok());
    assert_eq!(config.allow_recursion.len(), 2);
}

#[test]
fn test_add_recursion_acl_invalid() {
    let mut config = BindForwarderConfig::default();
    assert_eq!(config.add_recursion_acl("invalid.cidr"), Err(ConfigError::InvalidCidr));
    assert_eq!(config.add_recursion_acl("192.168.1.0/33"), Err(ConfigError::InvalidCidr));
    assert_eq!(config.add_recursion_acl("2001:db8::/129"), Err(ConfigError::InvalidCidr));
}

#[test]
fn test_add_recursion_acl_syntax_injection() {
    let mut config = BindForwarderConfig::default();
    assert_eq!(config.add_recursion_acl("192.168.1.0/24; allow-query { any; };"), Err(ConfigError::SyntaxInjection));
}

#[test]
fn test_generate_config_empty() {
    let config = BindForwarderConfig::default();
    assert_eq!(config.generate_config(), "");
}

#[test]
fn test_generate_config_forward_only() {
    let mut config = BindForwarderConfig::new(ForwardPolicy::Only);
    config.add_forwarder("8.8.8.8").unwrap();
    config.add_forwarder("8.8.4.4").unwrap();

    let gen = config.generate_config();
    assert_eq!(gen, "forwarders { 8.8.8.8; 8.8.4.4; };\nforward only;\n");
}

#[test]
fn test_generate_config_forward_first() {
    let mut config = BindForwarderConfig::new(ForwardPolicy::First);
    config.add_forwarder("1.1.1.1").unwrap();

    let gen = config.generate_config();
    assert_eq!(gen, "forwarders { 1.1.1.1; };\nforward first;\n");
}

#[test]
fn test_generate_config_with_recursion() {
    let mut config = BindForwarderConfig::new(ForwardPolicy::First);
    config.add_recursion_acl("10.0.0.0/8").unwrap();
    config.add_recursion_acl("192.168.0.0/16").unwrap();

    let gen = config.generate_config();
    assert_eq!(gen, "allow-recursion { 10.0.0.0/8; 192.168.0.0/16; };\n");
}

#[test]
fn test_generate_config_combined() {
    let mut config = BindForwarderConfig::new(ForwardPolicy::Only);
    config.add_forwarder("9.9.9.9").unwrap();
    config.add_recursion_acl("127.0.0.1").unwrap();

    let gen = config.generate_config();
    assert_eq!(gen, "forwarders { 9.9.9.9; };\nforward only;\nallow-recursion { 127.0.0.1; };\n");
}
