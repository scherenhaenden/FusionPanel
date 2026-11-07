#![forbid(unsafe_code)]

use security::bastion::{BastionConfig, BastionError, BastionManager};
use security::firewall::{Action, FirewallPolicy, FirewallRule, PortRange, Protocol};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::time::{Duration, SystemTime};

fn create_base_policy() -> FirewallPolicy {
    let mut policy = FirewallPolicy {
        rules: vec![],
        default_action: Action::Deny,
    };

    // Static rule that should not be removed
    let static_rule = FirewallRule {
        name: "static-allow-web".to_string(),
        action: Action::Allow,
        protocol: Protocol::Tcp,
        port_range: Some(PortRange::single(80)),
        source_ip: None,
    };
    policy.rules.push(static_rule);
    policy
}

#[test]
fn test_add_and_cleanup_leases() {
    let config = BastionConfig {
        default_lease_duration: Duration::from_secs(60),
        max_active_leases: 2,
        ssh_port: 22,
        rule_name_prefix: "bastion-allow-".to_string(),
    };
    let mut manager = BastionManager::new(config);
    let now = SystemTime::now();

    let ip1 = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100));
    let ip2 = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 101));
    let ip3 = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 102));

    // Add first lease
    assert!(manager.add_lease(ip1, now).is_ok());
    assert_eq!(manager.get_active_leases(now).len(), 1);

    // Add second lease
    assert!(manager.add_lease(ip2, now).is_ok());
    assert_eq!(manager.get_active_leases(now).len(), 2);

    // Third lease should fail (max active)
    assert!(matches!(
        manager.add_lease(ip3, now),
        Err(BastionError::MaxLeasesReached)
    ));

    // Cleanup should not remove anything yet
    manager.cleanup(now);
    assert_eq!(manager.get_active_leases(now).len(), 2);

    // Fast forward past expiration
    let future = now + Duration::from_secs(61);
    manager.cleanup(future);
    assert_eq!(manager.get_active_leases(future).len(), 0);

    // Now we can add ip3
    assert!(manager.add_lease(ip3, future).is_ok());
    assert_eq!(manager.get_active_leases(future).len(), 1);
}

#[test]
fn test_sync_firewall_policy() {
    let config = BastionConfig::default();
    let mut manager = BastionManager::new(config);
    let now = SystemTime::now();

    let ip1 = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));
    let ip2 = IpAddr::V6(Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 1));

    manager.add_lease(ip1, now).unwrap();
    manager.add_lease(ip2, now).unwrap();

    let mut policy = create_base_policy();

    // Initial sync
    manager.sync_firewall(&mut policy, now).unwrap();

    // The policy should now have the static rule + 2 bastion rules
    assert_eq!(policy.rules.len(), 3);

    // Check that rules were prepended
    let rule_names: Vec<String> = policy.rules.iter().map(|r| r.name.clone()).collect();
    assert!(rule_names.contains(&"bastion-allow-10.0.0.1".to_string()));
    assert!(rule_names.contains(&"bastion-allow-2001:db8::1".to_string()));
    assert_eq!(rule_names.last().unwrap(), "static-allow-web");

    // Fast forward to expire leases
    let future = now + Duration::from_secs(4000);
    manager.sync_firewall(&mut policy, future).unwrap();

    // The policy should only have the static rule left
    assert_eq!(policy.rules.len(), 1);
    assert_eq!(policy.rules[0].name, "static-allow-web");
}

#[test]
fn test_sync_firewall_policy_idempotent() {
    let config = BastionConfig::default();
    let mut manager = BastionManager::new(config);
    let now = SystemTime::now();

    let ip1 = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));

    manager.add_lease(ip1, now).unwrap();

    let mut policy = create_base_policy();

    // First sync
    manager.sync_firewall(&mut policy, now).unwrap();
    assert_eq!(policy.rules.len(), 2);

    // Second sync without changes should yield the same number of rules
    manager.sync_firewall(&mut policy, now).unwrap();
    assert_eq!(policy.rules.len(), 2);
}
