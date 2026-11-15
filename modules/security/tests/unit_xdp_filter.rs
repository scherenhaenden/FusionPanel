#![forbid(unsafe_code)]

use std::net::IpAddr;
use security::xdp_filter::{CidrBlock, XdpAction, XdpFilterPolicy};

#[test]
fn test_cidr_block_creation() {
    let valid_ipv4 = CidrBlock::new("192.168.1.0".parse().unwrap(), 24);
    assert!(valid_ipv4.is_ok());

    let invalid_ipv4 = CidrBlock::new("192.168.1.0".parse().unwrap(), 33);
    assert!(invalid_ipv4.is_err());

    let valid_ipv6 = CidrBlock::new("2001:db8::".parse().unwrap(), 64);
    assert!(valid_ipv6.is_ok());

    let invalid_ipv6 = CidrBlock::new("2001:db8::".parse().unwrap(), 129);
    assert!(invalid_ipv6.is_err());
}

#[test]
fn test_xdp_action_display() {
    assert_eq!(XdpAction::Pass.to_string(), "XDP_PASS");
    assert_eq!(XdpAction::Drop.to_string(), "XDP_DROP");
}

#[test]
fn test_xdp_filter_policy_builder() {
    let ip: IpAddr = "10.0.0.5".parse().unwrap();
    let cidr = CidrBlock::new("192.168.0.0".parse().unwrap(), 16).unwrap();

    let policy = XdpFilterPolicy::new("eth0")
        .with_default_action(XdpAction::Drop)
        .with_syn_flood_mitigation(true)
        .allow_cidr(cidr.clone())
        .block_ip(ip);

    assert_eq!(policy.interface, "eth0");
    assert_eq!(policy.default_action, XdpAction::Drop);
    assert!(policy.syn_flood_mitigation);
    assert_eq!(policy.allowlist.len(), 1);
    assert_eq!(policy.allowlist[0], cidr);
    assert_eq!(policy.blacklist.len(), 1);
    assert_eq!(policy.blacklist[0], ip);
}

#[test]
fn test_format_bpf_maps() {
    let policy = XdpFilterPolicy::new("eth1")
        .with_default_action(XdpAction::Pass)
        .with_syn_flood_mitigation(true)
        .allow_cidr(CidrBlock::new("10.0.0.0".parse().unwrap(), 8).unwrap())
        .block_ip("192.168.1.100".parse().unwrap())
        .block_ip("192.168.1.101".parse().unwrap());

    let formatted = policy.format_bpf_maps();

    assert!(formatted.contains("# XDP BPF Map Configuration for eth1"));
    assert!(formatted.contains("DEFAULT_ACTION=XDP_PASS"));
    assert!(formatted.contains("SYN_FLOOD_MITIGATION=ENABLED"));
    assert!(formatted.contains("ALLOW 10.0.0.0/8"));
    assert!(formatted.contains("BLOCK 192.168.1.100"));
    assert!(formatted.contains("BLOCK 192.168.1.101"));
}

#[test]
fn test_format_bpf_maps_empty_lists() {
    let policy = XdpFilterPolicy::new("eth2")
        .with_default_action(XdpAction::Drop)
        .with_syn_flood_mitigation(false);

    let formatted = policy.format_bpf_maps();

    assert!(formatted.contains("# XDP BPF Map Configuration for eth2"));
    assert!(formatted.contains("DEFAULT_ACTION=XDP_DROP"));
    assert!(formatted.contains("SYN_FLOOD_MITIGATION=DISABLED"));
    assert!(formatted.contains("\n# ALLOWLIST_MAP (LPM Trie)\n<empty>\n"));
    assert!(formatted.contains("\n# BLACKLIST_MAP (Hash Map)\n<empty>\n"));
}
