#![forbid(unsafe_code)]

use security::jail_aggregator::{JailAggregator, Subnet};
use std::net::IpAddr;
use std::time::{Duration, SystemTime};

#[test]
fn test_ipv4_subnet_conversion() {
    let ip: IpAddr = "192.168.1.100".parse().unwrap();
    let subnet = Subnet::from_ip(ip);
    assert_eq!(subnet, Subnet::V4([192, 168, 1]));
    assert_eq!(subnet.to_cidr_string(), "192.168.1.0/24");
}

#[test]
fn test_ipv6_subnet_conversion() {
    let ip: IpAddr = "2001:db8:85a3:0:0:8a2e:370:7334".parse().unwrap();
    let subnet = Subnet::from_ip(ip);
    assert_eq!(subnet, Subnet::V6([0x20, 0x01, 0x0d, 0xb8, 0x85, 0xa3]));
    assert_eq!(subnet.to_cidr_string(), "2001:db8:85a3::/48");
}

#[test]
fn test_aggregator_triggers_ban() {
    let mut agg = JailAggregator::new(3, Duration::from_secs(60));
    let now = SystemTime::now();

    // Event 1
    let ip1: IpAddr = "10.0.0.1".parse().unwrap();
    assert_eq!(agg.record_ban(ip1, "ssh", now), None);

    // Event 2 (same IP doesn't count towards threshold distinct)
    assert_eq!(agg.record_ban(ip1, "ssh", now), None);

    // Event 3 (different IP, same subnet)
    let ip2: IpAddr = "10.0.0.2".parse().unwrap();
    assert_eq!(agg.record_ban(ip2, "ssh", now), None);

    // Event 4 (different IP, same subnet -> triggers ban)
    let ip3: IpAddr = "10.0.0.3".parse().unwrap();
    assert_eq!(agg.record_ban(ip3, "ssh", now), Some("10.0.0.0/24".to_string()));

    // Further events from this subnet should return None (already banned)
    let ip4: IpAddr = "10.0.0.4".parse().unwrap();
    assert_eq!(agg.record_ban(ip4, "ssh", now), None);
}

#[test]
fn test_aggregator_sliding_window() {
    let mut agg = JailAggregator::new(2, Duration::from_secs(60));
    let start = SystemTime::now();

    // Event 1
    let ip1: IpAddr = "192.168.5.10".parse().unwrap();
    assert_eq!(agg.record_ban(ip1, "postfix", start), None);

    // Fast forward 61 seconds for Event 2
    let later = start + Duration::from_secs(61);
    let ip2: IpAddr = "192.168.5.11".parse().unwrap();

    // The previous event should be filtered out by the sliding window logic in record_ban
    assert_eq!(agg.record_ban(ip2, "postfix", later), None);

    // Another event within 60s of `later`
    let ip3: IpAddr = "192.168.5.12".parse().unwrap();
    assert_eq!(agg.record_ban(ip3, "postfix", later + Duration::from_secs(10)), Some("192.168.5.0/24".to_string()));
}

#[test]
fn test_purge_expired_events() {
    let mut agg = JailAggregator::new(3, Duration::from_secs(60));
    let start = SystemTime::now();

    let ip1: IpAddr = "172.16.0.1".parse().unwrap();
    agg.record_ban(ip1, "nginx-auth", start);

    let ip2: IpAddr = "172.16.0.2".parse().unwrap();
    agg.record_ban(ip2, "nginx-auth", start + Duration::from_secs(10));

    // Fast forward enough so that event 1 is expired, but event 2 is not
    let now = start + Duration::from_secs(65);
    agg.purge_expired_events(now);

    // Event 3
    let ip3: IpAddr = "172.16.0.3".parse().unwrap();
    assert_eq!(agg.record_ban(ip3, "nginx-auth", now), None); // Only ip2 and ip3 are in the window (distinct=2 < 3)
}
