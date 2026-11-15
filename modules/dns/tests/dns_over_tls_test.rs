#![forbid(unsafe_code)]

use dns::dns_over_tls::{DotForwarderPolicy, UpstreamEndpoint, Protocol, SpkiPin, FallbackPolicy, DotError};
use std::net::{IpAddr, Ipv4Addr};

#[test]
fn test_spki_pin_valid() {
    let pin_str = "pin-sha256=1234abcd1234abcd1234abcd1234abcd1234abcd1234abcd1234abcd1234abcd";
    let pin = SpkiPin::new(pin_str).unwrap();
    assert_eq!(pin.as_str(), pin_str);
}

#[test]
fn test_spki_pin_invalid_prefix() {
    let err = SpkiPin::new("invalid=1234").unwrap_err();
    assert!(matches!(err, DotError::InvalidSpkiPin(_)));
}

#[test]
fn test_spki_pin_invalid_base64() {
    let err = SpkiPin::new("pin-sha256=1234!").unwrap_err();
    assert!(matches!(err, DotError::InvalidSpkiPin(_)));
}

#[test]
fn test_endpoint_creation() {
    let ip = IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1));
    let mut ep = UpstreamEndpoint::new(
        ip,
        853,
        Protocol::DoT,
        Some("cloudflare-dns.com".to_string()),
        None,
    ).unwrap();
    assert_eq!(ep.ip, ip);
    assert_eq!(ep.port, 853);
    assert_eq!(ep.protocol, Protocol::DoT);
    assert_eq!(ep.tls_auth_name.as_deref(), Some("cloudflare-dns.com"));
    assert_eq!(ep.health.is_healthy, true);
    assert_eq!(ep.health.latency_ms, None);

    ep.update_health(false, Some(42));
    assert_eq!(ep.health.is_healthy, false);
    assert_eq!(ep.health.latency_ms, Some(42));
}

#[test]
fn test_render_coredns_strict() {
    let ep = UpstreamEndpoint::new(
        IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1)),
        853,
        Protocol::DoT,
        Some("cloudflare-dns.com".to_string()),
        None,
    ).unwrap();

    let policy = DotForwarderPolicy {
        endpoints: vec![ep],
        fallback_policy: FallbackPolicy::Strict,
    };

    let rendered = policy.render_coredns().unwrap();
    let expected = "\
forward . tls://1.1.1.1:853 {
    tls_servername cloudflare-dns.com
}
";
    assert_eq!(rendered, expected);
}

#[test]
fn test_render_coredns_opportunistic() {
    let ep = UpstreamEndpoint::new(
        IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8)),
        853,
        Protocol::DoT,
        None,
        None,
    ).unwrap();

    let policy = DotForwarderPolicy {
        endpoints: vec![ep],
        fallback_policy: FallbackPolicy::Opportunistic,
    };

    let rendered = policy.render_coredns().unwrap();
    let expected = "\
forward . tls://8.8.8.8:853 {
    tls_servername ignore
}
";
    assert_eq!(rendered, expected);
}

#[test]
fn test_render_coredns_strict_missing_hostname() {
    let ep = UpstreamEndpoint::new(
        IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1)),
        853,
        Protocol::DoT,
        None,
        None,
    ).unwrap();

    let policy = DotForwarderPolicy {
        endpoints: vec![ep],
        fallback_policy: FallbackPolicy::Strict,
    };

    let err = policy.render_coredns().unwrap_err();
    assert_eq!(err, DotError::MissingTlsHostname);
}

#[test]
fn test_render_unbound_strict() {
    let ep = UpstreamEndpoint::new(
        IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1)),
        853,
        Protocol::DoT,
        Some("cloudflare-dns.com".to_string()),
        None,
    ).unwrap();

    let policy = DotForwarderPolicy {
        endpoints: vec![ep],
        fallback_policy: FallbackPolicy::Strict,
    };

    let rendered = policy.render_unbound().unwrap();
    let expected = "\
forward-zone:
    name: \".\"
    forward-addr: 1.1.1.1@853#cloudflare-dns.com
    forward-tls-upstream: yes
";
    assert_eq!(rendered, expected);
}

#[test]
fn test_render_unbound_opportunistic() {
    let ep = UpstreamEndpoint::new(
        IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8)),
        853,
        Protocol::DoT,
        None,
        None,
    ).unwrap();

    let policy = DotForwarderPolicy {
        endpoints: vec![ep],
        fallback_policy: FallbackPolicy::Opportunistic,
    };

    let rendered = policy.render_unbound().unwrap();
    let expected = "\
forward-zone:
    name: \".\"
    forward-addr: 8.8.8.8@853
    forward-tls-upstream: no
";
    assert_eq!(rendered, expected);
}

#[test]
fn test_render_unbound_unsupported_doh() {
    let ep = UpstreamEndpoint::new(
        IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1)),
        443,
        Protocol::DoH,
        Some("cloudflare-dns.com".to_string()),
        None,
    ).unwrap();

    let policy = DotForwarderPolicy {
        endpoints: vec![ep],
        fallback_policy: FallbackPolicy::Strict,
    };

    let err = policy.render_unbound().unwrap_err();
    assert!(matches!(err, DotError::UnsupportedProtocol(_)));
}
