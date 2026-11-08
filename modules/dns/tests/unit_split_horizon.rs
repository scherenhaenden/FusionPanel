#![forbid(unsafe_code)]

use dns::split_horizon::{Cidr, DnsView, SplitHorizonResolver};
use std::net::IpAddr;
use std::str::FromStr;

#[test]
fn test_view_resolution_by_client_ip() {
    let mut resolver = SplitHorizonResolver::new();
    
    let mut internal_view = DnsView::new("internal", true);
    internal_view.add_acl(Cidr::from_str("10.0.0.0/8").unwrap());
    internal_view.add_acl(Cidr::from_str("192.168.1.0/24").unwrap());
    internal_view.add_record("app.example.com", IpAddr::from_str("10.0.0.5").unwrap());
    
    let mut external_view = DnsView::new("external", false);
    external_view.add_record("app.example.com", IpAddr::from_str("203.0.113.5").unwrap());
    
    resolver.add_view(internal_view);
    resolver.set_default_view(external_view);

    // Internal client gets internal record
    let client_ip = IpAddr::from_str("10.5.5.5").unwrap();
    let records = resolver.resolve(client_ip, "app.example.com").unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0], IpAddr::from_str("10.0.0.5").unwrap());

    // Another internal client gets internal record
    let client_ip2 = IpAddr::from_str("192.168.1.100").unwrap();
    let records2 = resolver.resolve(client_ip2, "app.example.com").unwrap();
    assert_eq!(records2.len(), 1);
    assert_eq!(records2[0], IpAddr::from_str("10.0.0.5").unwrap());

    // External client gets external record
    let ext_client_ip = IpAddr::from_str("8.8.8.8").unwrap();
    let ext_records = resolver.resolve(ext_client_ip, "app.example.com").unwrap();
    assert_eq!(ext_records.len(), 1);
    assert_eq!(ext_records[0], IpAddr::from_str("203.0.113.5").unwrap());
}

#[test]
fn test_internal_vs_public_answer_segregation() {
    let mut resolver = SplitHorizonResolver::new();
    
    let mut internal_view = DnsView::new("internal", true);
    internal_view.add_acl(Cidr::from_str("10.0.0.0/8").unwrap());
    internal_view.add_record("db.example.com", IpAddr::from_str("10.0.1.10").unwrap());
    
    let mut external_view = DnsView::new("external", false);
    external_view.add_record("web.example.com", IpAddr::from_str("203.0.113.10").unwrap());
    
    resolver.add_view(internal_view);
    resolver.set_default_view(external_view);

    let int_client = IpAddr::from_str("10.0.0.2").unwrap();
    let ext_client = IpAddr::from_str("8.8.8.8").unwrap();

    // Internal client sees db.example.com
    assert!(resolver.resolve(int_client, "db.example.com").is_some());
    // External client does not see db.example.com
    assert!(resolver.resolve(ext_client, "db.example.com").is_none());
    
    // Internal client does not see web.example.com (because it uses internal view only)
    assert!(resolver.resolve(int_client, "web.example.com").is_none());
    // External client sees web.example.com
    assert!(resolver.resolve(ext_client, "web.example.com").is_some());
}

#[test]
fn test_strict_prevention_of_private_ip_leakage() {
    let mut resolver = SplitHorizonResolver::new();
    
    // External view with a misconfigured record pointing to an internal IP
    let mut external_view = DnsView::new("external", false);
    
    // Public IP
    external_view.add_record("mix.example.com", IpAddr::from_str("203.0.113.5").unwrap());
    // RFC1918 Private IP
    external_view.add_record("mix.example.com", IpAddr::from_str("192.168.0.1").unwrap());
    // Loopback IP
    external_view.add_record("mix.example.com", IpAddr::from_str("127.0.0.1").unwrap());
    // CGNAT IP
    external_view.add_record("mix.example.com", IpAddr::from_str("100.64.0.10").unwrap());

    resolver.set_default_view(external_view);

    let ext_client = IpAddr::from_str("8.8.8.8").unwrap();
    
    let records = resolver.resolve(ext_client, "mix.example.com").unwrap();
    
    // Only the public IP should be returned, all others filtered out
    assert_eq!(records.len(), 1);
    assert_eq!(records[0], IpAddr::from_str("203.0.113.5").unwrap());
}

#[test]
fn test_ipv6_private_ip_leakage() {
    let mut resolver = SplitHorizonResolver::new();
    
    let mut external_view = DnsView::new("external", false);
    
    // Public IPv6
    external_view.add_record("ipv6.example.com", IpAddr::from_str("2001:db8::1").unwrap());
    // ULA IPv6
    external_view.add_record("ipv6.example.com", IpAddr::from_str("fc00::1").unwrap());
    external_view.add_record("ipv6.example.com", IpAddr::from_str("fd12:3456:789a:1::1").unwrap());
    // Loopback IPv6
    external_view.add_record("ipv6.example.com", IpAddr::from_str("::1").unwrap());

    resolver.set_default_view(external_view);

    let ext_client = IpAddr::from_str("2001:db8::2").unwrap();
    
    let records = resolver.resolve(ext_client, "ipv6.example.com").unwrap();
    
    assert_eq!(records.len(), 1);
    assert_eq!(records[0], IpAddr::from_str("2001:db8::1").unwrap());
}

#[test]
fn test_ipv6_cidr_match() {
    let mut resolver = SplitHorizonResolver::new();
    
    let mut internal_view = DnsView::new("internal", true);
    internal_view.add_acl(Cidr::from_str("2001:db8::/32").unwrap());
    internal_view.add_record("app.example.com", IpAddr::from_str("10.0.0.5").unwrap());
    
    resolver.add_view(internal_view);

    let client_match = IpAddr::from_str("2001:db8:1234::1").unwrap();
    let client_no_match = IpAddr::from_str("2001:db9::1").unwrap();

    assert!(resolver.resolve(client_match, "app.example.com").is_some());
    assert!(resolver.resolve(client_no_match, "app.example.com").is_none());
}
