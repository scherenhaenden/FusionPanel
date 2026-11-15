#![forbid(unsafe_code)]

use dns::rrl_config::RrlConfig;

#[test]
fn test_default_rrl_config() {
    let config = RrlConfig::default();
    assert_eq!(config.responses_per_second, 5);
    assert_eq!(config.window, None);
    assert_eq!(config.slip, None);
    assert_eq!(config.ipv4_prefix_length, None);
    assert_eq!(config.ipv6_prefix_length, None);
    assert!(config.exempt_clients.is_empty());

    let expected = "\
rate-limit {
    responses-per-second 5;
};";
    assert_eq!(config.generate(), expected);
}

#[test]
fn test_custom_rrl_config() {
    let config = RrlConfig::new(10)
        .with_window(15)
        .with_slip(2)
        .with_ipv4_prefix_length(24)
        .with_ipv6_prefix_length(56)
        .add_exempt_client("10.0.0.0/8")
        .add_exempt_client("192.168.1.0/24");

    let expected = "\
rate-limit {
    responses-per-second 10;
    window 15;
    slip 2;
    ipv4-prefix-length 24;
    ipv6-prefix-length 56;
    exempt-clients { 10.0.0.0/8; 192.168.1.0/24; };
};";
    assert_eq!(config.generate(), expected);
}

#[test]
fn test_partial_rrl_config() {
    let config = RrlConfig::new(20)
        .with_slip(3)
        .add_exempt_client("127.0.0.0/8");

    let expected = "\
rate-limit {
    responses-per-second 20;
    slip 3;
    exempt-clients { 127.0.0.0/8; };
};";
    assert_eq!(config.generate(), expected);
}
