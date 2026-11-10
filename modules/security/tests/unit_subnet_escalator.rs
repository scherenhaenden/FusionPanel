#![forbid(unsafe_code)]

use std::net::{IpAddr, Ipv4Addr};
use std::str::FromStr;
use std::time::Duration;
use security::subnet_escalator::{Cidr, EscalationResult, SubnetEscalator, get_subnet};

#[test]
fn test_ipv4_escalation() {
    let mut escalator = SubnetEscalator::new(5, 3600, 10, vec![]);

    // Ban 4 unique IPs in 192.168.1.0/24
    for i in 1..=4 {
        let ip = IpAddr::V4(Ipv4Addr::new(192, 168, 1, i));
        assert_eq!(
            escalator.register_ban(ip, i as u64 * 100, Duration::from_secs(60)),
            EscalationResult::NotEscalated
        );
    }

    // 5th ban should trigger escalation
    let ip = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 5));
    let res = escalator.register_ban(ip, 500, Duration::from_secs(60));

    if let EscalationResult::Escalated { subnet, duration } = res {
        assert_eq!(subnet, get_subnet(ip));
        assert_eq!(duration, Duration::from_secs(600));
    } else {
        panic!("Expected escalation");
    }
}

#[test]
fn test_ipv6_escalation() {
    let mut escalator = SubnetEscalator::new(3, 3600, 10, vec![]);

    let base_ipv6 = "2001:db8:abcd:0012:0000:0000:0000:000";

    for i in 1..=2 {
        let ip_str = format!("{}{}", base_ipv6, i);
        let ip = IpAddr::from_str(&ip_str).unwrap();
        assert_eq!(
            escalator.register_ban(ip, i as u64 * 100, Duration::from_secs(60)),
            EscalationResult::NotEscalated
        );
    }

    let ip_str = format!("{}{}", base_ipv6, 3);
    let ip = IpAddr::from_str(&ip_str).unwrap();
    let res = escalator.register_ban(ip, 300, Duration::from_secs(60));

    if let EscalationResult::Escalated { subnet, duration } = res {
        assert_eq!(subnet, get_subnet(ip));
        assert_eq!(subnet.prefix, 64);
        assert_eq!(duration, Duration::from_secs(600));
    } else {
        panic!("Expected escalation");
    }
}

#[test]
fn test_sliding_window_expiration() {
    let mut escalator = SubnetEscalator::new(3, 3600, 10, vec![]);

    let ip1 = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));
    let ip2 = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 2));
    let ip3 = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 3));

    assert_eq!(escalator.register_ban(ip1, 100, Duration::from_secs(60)), EscalationResult::NotEscalated);
    assert_eq!(escalator.register_ban(ip2, 200, Duration::from_secs(60)), EscalationResult::NotEscalated);

    // Wait for the window to pass (3600 secs)
    // Event 3 at time 4000. Window is [4000-3600, 4000] = [400, 4000]
    // Both 100 and 200 are expired!
    assert_eq!(escalator.register_ban(ip3, 4000, Duration::from_secs(60)), EscalationResult::NotEscalated);

    // Now we need 2 more to escalate
    let ip4 = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 4));
    let ip5 = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 5));

    assert_eq!(escalator.register_ban(ip4, 4001, Duration::from_secs(60)), EscalationResult::NotEscalated);
    let res = escalator.register_ban(ip5, 4002, Duration::from_secs(60));
    assert!(matches!(res, EscalationResult::Escalated { .. }));
}

#[test]
fn test_whitelist() {
    let whitelist = vec![
        Cidr::from_str("192.168.1.0/24").unwrap(),
        Cidr::from_str("10.0.0.0/8").unwrap(),
    ];
    let mut escalator = SubnetEscalator::new(2, 3600, 10, whitelist);

    // IP in whitelisted /24
    let ip1 = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 10));
    let ip2 = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 11));
    assert_eq!(escalator.register_ban(ip1, 100, Duration::from_secs(60)), EscalationResult::NotEscalated);
    assert_eq!(escalator.register_ban(ip2, 200, Duration::from_secs(60)), EscalationResult::NotEscalated);

    // IP in whitelisted /8
    let ip3 = IpAddr::V4(Ipv4Addr::new(10, 5, 5, 5));
    let ip4 = IpAddr::V4(Ipv4Addr::new(10, 5, 5, 6));
    assert_eq!(escalator.register_ban(ip3, 100, Duration::from_secs(60)), EscalationResult::NotEscalated);
    assert_eq!(escalator.register_ban(ip4, 200, Duration::from_secs(60)), EscalationResult::NotEscalated);

    // IP not in whitelist
    let ip5 = IpAddr::V4(Ipv4Addr::new(172, 16, 0, 1));
    let ip6 = IpAddr::V4(Ipv4Addr::new(172, 16, 0, 2));
    assert_eq!(escalator.register_ban(ip5, 100, Duration::from_secs(60)), EscalationResult::NotEscalated);
    assert!(matches!(
        escalator.register_ban(ip6, 200, Duration::from_secs(60)),
        EscalationResult::Escalated { .. }
    ));
}

#[test]
fn test_same_ip_multiple_bans() {
    let mut escalator = SubnetEscalator::new(3, 3600, 10, vec![]);

    let ip = IpAddr::V4(Ipv4Addr::new(192, 168, 2, 1));

    // Ban same IP 5 times
    for i in 1..=5 {
        assert_eq!(
            escalator.register_ban(ip, i as u64 * 100, Duration::from_secs(60)),
            EscalationResult::NotEscalated
        );
    }

    // Ban a second unique IP
    let ip2 = IpAddr::V4(Ipv4Addr::new(192, 168, 2, 2));
    assert_eq!(escalator.register_ban(ip2, 600, Duration::from_secs(60)), EscalationResult::NotEscalated);

    // Ban a third unique IP (triggers escalation)
    let ip3 = IpAddr::V4(Ipv4Addr::new(192, 168, 2, 3));
    let res = escalator.register_ban(ip3, 700, Duration::from_secs(60));
    assert!(matches!(res, EscalationResult::Escalated { .. }));
}
