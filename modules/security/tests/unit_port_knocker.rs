#![forbid(unsafe_code)]

use std::time::{Duration, Instant};
use security::port_knocker::{KnockConfig, PortKnocker, KnockProtocol, KnockResult};

#[test]
fn test_successful_knock_sequence() {
    let config = KnockConfig {
        sequence: vec![(1000, KnockProtocol::Tcp), (2000, KnockProtocol::Udp), (3000, KnockProtocol::Tcp)],
        knock_timeout: Duration::from_secs(5),
        grant_duration: Duration::from_secs(60),
    };
    let mut knocker = PortKnocker::new(config);
    let now = Instant::now();
    let ip = "192.168.1.100";

    // First knock
    assert!(matches!(knocker.process_knock(ip, 1000, KnockProtocol::Tcp, now), KnockResult::Advanced));
    assert!(!knocker.is_granted(ip, now));

    // Second knock
    assert!(matches!(knocker.process_knock(ip, 2000, KnockProtocol::Udp, now + Duration::from_secs(1)), KnockResult::Advanced));
    assert!(!knocker.is_granted(ip, now + Duration::from_secs(1)));

    // Third knock - sequence complete
    assert!(matches!(knocker.process_knock(ip, 3000, KnockProtocol::Tcp, now + Duration::from_secs(2)), KnockResult::AccessGranted));
    assert!(knocker.is_granted(ip, now + Duration::from_secs(2)));
}

#[test]
fn test_incorrect_port_resets_sequence() {
    let config = KnockConfig {
        sequence: vec![(1000, KnockProtocol::Tcp), (2000, KnockProtocol::Udp), (3000, KnockProtocol::Tcp)],
        knock_timeout: Duration::from_secs(5),
        grant_duration: Duration::from_secs(60),
    };
    let mut knocker = PortKnocker::new(config);
    let now = Instant::now();
    let ip = "192.168.1.100";

    assert!(matches!(knocker.process_knock(ip, 1000, KnockProtocol::Tcp, now), KnockResult::Advanced));

    // Incorrect port
    assert!(matches!(knocker.process_knock(ip, 9999, KnockProtocol::Tcp, now + Duration::from_secs(1)), KnockResult::Reset));

    // Must start over
    assert!(matches!(knocker.process_knock(ip, 2000, KnockProtocol::Udp, now + Duration::from_secs(2)), KnockResult::Reset));
    assert!(matches!(knocker.process_knock(ip, 1000, KnockProtocol::Tcp, now + Duration::from_secs(3)), KnockResult::Advanced));
}

#[test]
fn test_incorrect_protocol_resets_sequence() {
    let config = KnockConfig {
        sequence: vec![(1000, KnockProtocol::Tcp), (2000, KnockProtocol::Udp)],
        knock_timeout: Duration::from_secs(5),
        grant_duration: Duration::from_secs(60),
    };
    let mut knocker = PortKnocker::new(config);
    let now = Instant::now();
    let ip = "192.168.1.100";

    assert!(matches!(knocker.process_knock(ip, 1000, KnockProtocol::Tcp, now), KnockResult::Advanced));

    // Incorrect protocol
    assert!(matches!(knocker.process_knock(ip, 2000, KnockProtocol::Tcp, now + Duration::from_secs(1)), KnockResult::Reset));
}

#[test]
fn test_knock_timeout() {
    let config = KnockConfig {
        sequence: vec![(1000, KnockProtocol::Tcp), (2000, KnockProtocol::Udp)],
        knock_timeout: Duration::from_secs(5),
        grant_duration: Duration::from_secs(60),
    };
    let mut knocker = PortKnocker::new(config);
    let now = Instant::now();
    let ip = "192.168.1.100";

    assert!(matches!(knocker.process_knock(ip, 1000, KnockProtocol::Tcp, now), KnockResult::Advanced));

    // Exceed timeout
    let later = now + Duration::from_secs(6);
    assert!(matches!(knocker.process_knock(ip, 2000, KnockProtocol::Udp, later), KnockResult::Reset));
}

#[test]
fn test_grant_expiration() {
    let config = KnockConfig {
        sequence: vec![(1000, KnockProtocol::Tcp)],
        knock_timeout: Duration::from_secs(5),
        grant_duration: Duration::from_secs(10),
    };
    let mut knocker = PortKnocker::new(config);
    let now = Instant::now();
    let ip = "192.168.1.100";

    assert!(matches!(knocker.process_knock(ip, 1000, KnockProtocol::Tcp, now), KnockResult::AccessGranted));
    assert!(knocker.is_granted(ip, now));

    // Still granted
    assert!(knocker.is_granted(ip, now + Duration::from_secs(9)));

    // Expired
    assert!(!knocker.is_granted(ip, now + Duration::from_secs(11)));
}

#[test]
fn test_multiple_ips_concurrently() {
    let config = KnockConfig {
        sequence: vec![(1000, KnockProtocol::Tcp), (2000, KnockProtocol::Udp)],
        knock_timeout: Duration::from_secs(5),
        grant_duration: Duration::from_secs(60),
    };
    let mut knocker = PortKnocker::new(config);
    let now = Instant::now();

    let ip1 = "10.0.0.1";
    let ip2 = "10.0.0.2";

    assert!(matches!(knocker.process_knock(ip1, 1000, KnockProtocol::Tcp, now), KnockResult::Advanced));
    assert!(matches!(knocker.process_knock(ip2, 1000, KnockProtocol::Tcp, now + Duration::from_secs(1)), KnockResult::Advanced));

    assert!(matches!(knocker.process_knock(ip1, 2000, KnockProtocol::Udp, now + Duration::from_secs(2)), KnockResult::AccessGranted));

    assert!(knocker.is_granted(ip1, now + Duration::from_secs(2)));
    assert!(!knocker.is_granted(ip2, now + Duration::from_secs(2)));

    assert!(matches!(knocker.process_knock(ip2, 2000, KnockProtocol::Udp, now + Duration::from_secs(3)), KnockResult::AccessGranted));
    assert!(knocker.is_granted(ip2, now + Duration::from_secs(3)));
}

#[test]
fn test_restart_sequence_if_incorrect() {
    let config = KnockConfig {
        sequence: vec![(1000, KnockProtocol::Tcp), (2000, KnockProtocol::Udp), (3000, KnockProtocol::Tcp)],
        knock_timeout: Duration::from_secs(5),
        grant_duration: Duration::from_secs(60),
    };
    let mut knocker = PortKnocker::new(config);
    let now = Instant::now();
    let ip = "192.168.1.100";

    assert!(matches!(knocker.process_knock(ip, 1000, KnockProtocol::Tcp, now), KnockResult::Advanced));

    // Incorrect, but matches the first knock again! This is a valid use case where user realizes mistake and restarts
    // Our logic handles it by resetting to index 1.
    assert!(matches!(knocker.process_knock(ip, 1000, KnockProtocol::Tcp, now + Duration::from_secs(1)), KnockResult::Advanced));

    // Should now correctly proceed to index 2
    assert!(matches!(knocker.process_knock(ip, 2000, KnockProtocol::Udp, now + Duration::from_secs(2)), KnockResult::Advanced));
    assert!(matches!(knocker.process_knock(ip, 3000, KnockProtocol::Tcp, now + Duration::from_secs(3)), KnockResult::AccessGranted));
}

#[test]
fn test_generate_firewall_rules() {
    let config = KnockConfig {
        sequence: vec![(1000, KnockProtocol::Tcp)],
        knock_timeout: Duration::from_secs(5),
        grant_duration: Duration::from_secs(60),
    };
    let mut knocker = PortKnocker::new(config);
    let now = Instant::now();
    let ip1 = "192.168.1.100";
    let ip2 = "10.0.0.1";

    knocker.process_knock(ip1, 1000, KnockProtocol::Tcp, now);
    knocker.process_knock(ip2, 1000, KnockProtocol::Tcp, now);

    let rules = knocker.generate_firewall_rules(now);
    assert_eq!(rules.len(), 2);
    assert_eq!(rules[0], "iptables -I INPUT -s 10.0.0.1 -p tcp --dport 22 -j ACCEPT");
    assert_eq!(rules[1], "iptables -I INPUT -s 192.168.1.100 -p tcp --dport 22 -j ACCEPT");

    // Test expiration
    let rules = knocker.generate_firewall_rules(now + Duration::from_secs(61));
    assert_eq!(rules.len(), 0);
}
