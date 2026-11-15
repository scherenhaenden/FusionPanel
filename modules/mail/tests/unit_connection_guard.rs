#![forbid(unsafe_code)]

use mail::connection_guard::{GuardError, MailConnectionGuard};
use std::time::{Duration, Instant};

#[test]
fn test_acquire_ip_concurrency_limit() {
    let guard = MailConnectionGuard::new(2, 10, 10, 10, Duration::from_secs(60));
    let now = Instant::now();

    let token1 = guard.acquire_ip("192.168.1.1", now).unwrap();
    let token2 = guard.acquire_ip("192.168.1.1", now).unwrap();
    let result = guard.acquire_ip("192.168.1.1", now);

    assert_eq!(result.unwrap_err(), GuardError::TooManyConnections);

    drop(token1);

    // Now we can acquire again
    let _token3 = guard.acquire_ip("192.168.1.1", now).unwrap();
    // But not another one
    let result2 = guard.acquire_ip("192.168.1.1", now);
    assert_eq!(result2.unwrap_err(), GuardError::TooManyConnections);

    drop(token2);
    drop(_token3);
}

#[test]
fn test_acquire_user_concurrency_limit() {
    let guard = MailConnectionGuard::new(10, 2, 10, 10, Duration::from_secs(60));
    let now = Instant::now();

    let token1 = guard.acquire_user("user1@example.com", now).unwrap();
    let token2 = guard.acquire_user("user1@example.com", now).unwrap();
    let result = guard.acquire_user("user1@example.com", now);

    assert_eq!(result.unwrap_err(), GuardError::TooManyConnections);

    drop(token1);

    // Now we can acquire again
    let _token3 = guard.acquire_user("user1@example.com", now).unwrap();
    // But not another one
    let result2 = guard.acquire_user("user1@example.com", now);
    assert_eq!(result2.unwrap_err(), GuardError::TooManyConnections);

    drop(token2);
    drop(_token3);
}

#[test]
fn test_acquire_ip_burst_limit() {
    let burst_window = Duration::from_secs(60);
    let guard = MailConnectionGuard::new(10, 10, 3, 10, burst_window);
    let mut now = Instant::now();

    // 1st request
    let t1 = guard.acquire_ip("10.0.0.1", now).unwrap();
    drop(t1);

    // 2nd request
    now += Duration::from_secs(1);
    let t2 = guard.acquire_ip("10.0.0.1", now).unwrap();
    drop(t2);

    // 3rd request
    now += Duration::from_secs(1);
    let t3 = guard.acquire_ip("10.0.0.1", now).unwrap();
    drop(t3);

    // 4th request (flood rejection)
    now += Duration::from_secs(1);
    let result = guard.acquire_ip("10.0.0.1", now);
    assert_eq!(result.unwrap_err(), GuardError::RateLimitExceeded);

    // Wait until burst window passes for the first request
    now += burst_window;
    let t4 = guard.acquire_ip("10.0.0.1", now).unwrap();
    drop(t4);
}

#[test]
fn test_acquire_user_burst_limit() {
    let burst_window = Duration::from_secs(60);
    let guard = MailConnectionGuard::new(10, 10, 10, 3, burst_window);
    let mut now = Instant::now();

    // 1st request
    let t1 = guard.acquire_user("user2", now).unwrap();
    drop(t1);

    // 2nd request
    now += Duration::from_secs(1);
    let t2 = guard.acquire_user("user2", now).unwrap();
    drop(t2);

    // 3rd request
    now += Duration::from_secs(1);
    let t3 = guard.acquire_user("user2", now).unwrap();
    drop(t3);

    // 4th request (flood rejection)
    now += Duration::from_secs(1);
    let result = guard.acquire_user("user2", now);
    assert_eq!(result.unwrap_err(), GuardError::RateLimitExceeded);

    // Wait until burst window passes for the first request
    now += burst_window;
    let t4 = guard.acquire_user("user2", now).unwrap();
    drop(t4);
}

#[test]
fn test_independent_tracking() {
    let guard = MailConnectionGuard::new(1, 1, 10, 10, Duration::from_secs(60));
    let now = Instant::now();

    let _ip_token1 = guard.acquire_ip("ip1", now).unwrap();
    let _ip_token2 = guard.acquire_ip("ip2", now).unwrap();
    assert_eq!(
        guard.acquire_ip("ip1", now).unwrap_err(),
        GuardError::TooManyConnections
    );

    let _user_token1 = guard.acquire_user("user1", now).unwrap();
    let _user_token2 = guard.acquire_user("user2", now).unwrap();
    assert_eq!(
        guard.acquire_user("user1", now).unwrap_err(),
        GuardError::TooManyConnections
    );
}
