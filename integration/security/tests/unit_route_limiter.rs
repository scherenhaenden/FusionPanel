#![forbid(unsafe_code)]

use security::route_limiter::{RouteLimit, RouteRateLimiter};
use std::time::{Duration, Instant};

#[test]
fn test_route_limiter_allows_and_blocks() {
    let limits = vec![
        RouteLimit {
            pattern: "/v1/auth/login".to_string(),
            method: Some("POST".to_string()),
            burst_capacity: 2,
            leak_rate: Duration::from_secs(1),
        },
        RouteLimit {
            pattern: "/v1/health".to_string(),
            method: None,
            burst_capacity: 5,
            leak_rate: Duration::from_millis(100),
        },
    ];

    let mut limiter = RouteRateLimiter::new(limits);
    let now = Instant::now();

    // Check strict limit on POST /v1/auth/login
    assert!(limiter.check("ip-1", "/v1/auth/login", "POST", now).is_ok());
    assert!(limiter.check("ip-1", "/v1/auth/login", "POST", now).is_ok());

    // Third request should be blocked
    let result = limiter.check("ip-1", "/v1/auth/login", "POST", now);
    assert!(result.is_err());

    if let Err(retry_after) = result {
        // Wait time in seconds rounded up should be positive string
        let wait_secs: u64 = retry_after.parse().unwrap();
        assert!(wait_secs > 0);
    }

    // Different IP should still be allowed
    assert!(limiter.check("ip-2", "/v1/auth/login", "POST", now).is_ok());

    // GET to auth/login is not restricted by this specific rule, so it passes
    assert!(limiter.check("ip-1", "/v1/auth/login", "GET", now).is_ok());

    // Check /v1/health allows up to 5 requests
    for _ in 0..5 {
        assert!(limiter.check("ip-1", "/v1/health", "GET", now).is_ok());
    }
    assert!(limiter.check("ip-1", "/v1/health", "GET", now).is_err());
}

#[test]
fn test_burst_recovery() {
    let limits = vec![RouteLimit {
        pattern: "/v1/burst".to_string(),
        method: None,
        burst_capacity: 1,
        leak_rate: Duration::from_secs(2), // 1 token every 2 seconds
    }];

    let mut limiter = RouteRateLimiter::new(limits);
    let start = Instant::now();

    // 1st request should pass
    assert!(limiter.check("ip-1", "/v1/burst", "GET", start).is_ok());

    // 2nd request should be blocked
    let result = limiter.check("ip-1", "/v1/burst", "GET", start);
    assert!(result.is_err());
    let retry_after = result.unwrap_err();
    let wait_secs: u64 = retry_after.parse().unwrap();
    assert!(wait_secs >= 1);

    // Wait for the token to replenish
    let later = start + Duration::from_secs(2); // exactly enough to replenish 1 token
    assert!(limiter.check("ip-1", "/v1/burst", "GET", later).is_ok());

    // Another immediate request should be blocked again
    assert!(limiter.check("ip-1", "/v1/burst", "GET", later).is_err());
}

#[test]
fn test_multiple_methods_matched() {
    let limits = vec![RouteLimit {
        pattern: "/v1/test".to_string(),
        method: None,
        burst_capacity: 1,
        leak_rate: Duration::from_secs(10),
    }];

    let mut limiter = RouteRateLimiter::new(limits);
    let now = Instant::now();

    // The limit has method: None, meaning it applies to all methods.
    // The bucket key includes the method so GET and POST have separate buckets.
    // Wait, let me check the implementation. The current implementation creates
    // bucket key as format!("{}:{}:{}", key, method, limit.pattern);
    // So GET and POST would have distinct buckets. Let's verify this.
    assert!(limiter.check("ip-1", "/v1/test", "GET", now).is_ok());
    assert!(limiter.check("ip-1", "/v1/test", "GET", now).is_err());

    // POST should have its own bucket and be ok
    assert!(limiter.check("ip-1", "/v1/test", "POST", now).is_ok());
    assert!(limiter.check("ip-1", "/v1/test", "POST", now).is_err());
}

#[test]
fn test_prefix_requires_path_segment_boundary_and_prefers_specific_rule() {
    let limits = vec![
        RouteLimit {
            pattern: "/admin".to_string(),
            method: None,
            burst_capacity: 1,
            leak_rate: Duration::from_secs(60),
        },
        RouteLimit {
            pattern: "/admin/users".to_string(),
            method: Some("POST".to_string()),
            burst_capacity: 2,
            leak_rate: Duration::from_secs(60),
        },
    ];
    let mut limiter = RouteRateLimiter::new(limits);
    let now = Instant::now();

    assert!(limiter.check("key", "/administrator", "GET", now).is_ok());
    assert!(limiter.check("key", "/admin/users", "POST", now).is_ok());
    assert!(limiter.check("key", "/admin/users/7", "POST", now).is_ok());
    assert!(limiter.check("key", "/admin/users/7", "POST", now).is_err());
}

#[test]
fn test_zero_rate_is_rejected_without_panicking() {
    let mut limiter = RouteRateLimiter::new(vec![RouteLimit {
        pattern: "/limited".to_string(),
        method: None,
        burst_capacity: 1,
        leak_rate: Duration::ZERO,
    }]);

    assert!(limiter
        .check("key", "/limited", "GET", Instant::now())
        .is_err());
}
