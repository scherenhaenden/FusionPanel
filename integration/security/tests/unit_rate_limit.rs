#![forbid(unsafe_code)]

#[path = "../src/rate_limit.rs"]
pub mod rate_limit;

use rate_limit::RateLimiter;
use std::net::IpAddr;
use std::str::FromStr;
use std::time::{Duration, Instant};
use ipnet::IpNet;

#[test]
fn test_ip_based_allowance_and_rejection() {
    let mut limiter = RateLimiter::new(
        vec![],
        Duration::from_secs(60),
        2,
        Duration::from_secs(60),
        5,
        3,
        Duration::from_secs(300),
    );

    let ip: IpAddr = "192.168.1.1".parse().unwrap();
    let now = Instant::now();

    assert!(limiter.check_ip(ip, now), "First request should be allowed");
    assert!(limiter.check_ip(ip, now), "Second request should be allowed");
    assert!(!limiter.check_ip(ip, now), "Third request should be rejected");

    let future = now + Duration::from_secs(61);
    assert!(limiter.check_ip(ip, future), "Request after window reset should be allowed");
}

#[test]
fn test_key_based_allowance_and_rejection() {
    let mut limiter = RateLimiter::new(
        vec![],
        Duration::from_secs(60),
        5,
        Duration::from_secs(60),
        2,
        3,
        Duration::from_secs(300),
    );

    let key = "user1";
    let now = Instant::now();

    assert!(limiter.check_key(key, now), "First request should be allowed");
    assert!(limiter.check_key(key, now), "Second request should be allowed");
    assert!(!limiter.check_key(key, now), "Third request should be rejected");

    let future = now + Duration::from_secs(61);
    assert!(limiter.check_key(key, future), "Request after window reset should be allowed");
}

#[test]
fn test_login_failure_progressive_delay() {
    let mut limiter = RateLimiter::new(
        vec![],
        Duration::from_secs(60),
        100,
        Duration::from_secs(60),
        100,
        2, // threshold
        Duration::from_secs(10), // base delay
    );

    let ip: IpAddr = "10.0.0.1".parse().unwrap();
    let now = Instant::now();

    // 1st failure
    limiter.record_login_failure(ip, now);
    assert!(limiter.check_login_allowed(ip, now), "Allowed after 1 fail");

    // 2nd failure
    limiter.record_login_failure(ip, now);
    assert!(limiter.check_login_allowed(ip, now), "Allowed after 2 fails (at threshold)");

    // 3rd failure (exceeds threshold)
    limiter.record_login_failure(ip, now);
    assert!(!limiter.check_login_allowed(ip, now), "Blocked after 3 fails");

    // Check progressive delay (base_delay * 1 = 10s)
    let delay1 = now + Duration::from_secs(11);
    assert!(limiter.check_login_allowed(ip, delay1), "Allowed after 10s delay");

    // 4th failure
    limiter.record_login_failure(ip, now);
    assert!(!limiter.check_login_allowed(ip, now), "Blocked again");

    // Check progressive delay (base_delay * 2 = 20s)
    let delay2 = now + Duration::from_secs(11);
    assert!(!limiter.check_login_allowed(ip, delay2), "Still blocked after 10s (needs 20s)");

    let delay3 = now + Duration::from_secs(21);
    assert!(limiter.check_login_allowed(ip, delay3), "Allowed after 20s delay");

    // Success resets failures
    limiter.record_login_success(ip);
    assert!(limiter.check_login_allowed(ip, now), "Allowed immediately after success");
}

#[test]
fn test_cidr_whitelisting() {
    let whitelist = vec![
        IpNet::from_str("127.0.0.0/8").unwrap(),
        IpNet::from_str("10.0.0.0/16").unwrap(),
    ];
    let mut limiter = RateLimiter::new(
        whitelist,
        Duration::from_secs(60),
        1,
        Duration::from_secs(60),
        1,
        1,
        Duration::from_secs(10),
    );

    let whitelisted_ip1: IpAddr = "127.0.0.1".parse().unwrap();
    let whitelisted_ip2: IpAddr = "10.0.5.5".parse().unwrap();
    let normal_ip: IpAddr = "192.168.1.1".parse().unwrap();
    let now = Instant::now();

    // Whitelisted IPs should never be blocked by check_ip
    assert!(limiter.check_ip(whitelisted_ip1, now));
    assert!(limiter.check_ip(whitelisted_ip1, now));
    assert!(limiter.check_ip(whitelisted_ip2, now));
    assert!(limiter.check_ip(whitelisted_ip2, now));

    // Normal IP should be limited
    assert!(limiter.check_ip(normal_ip, now));
    assert!(!limiter.check_ip(normal_ip, now));

    // Whitelisted IPs should not be blocked for login failures
    limiter.record_login_failure(whitelisted_ip1, now);
    limiter.record_login_failure(whitelisted_ip1, now);
    limiter.record_login_failure(whitelisted_ip1, now);
    assert!(limiter.check_login_allowed(whitelisted_ip1, now));
}
