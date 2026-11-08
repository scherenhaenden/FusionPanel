#![forbid(unsafe_code)]

use containers::circuit_breaker::{CircuitBreakerState, ContainerCrashCircuitBreaker};
use std::time::{Duration, Instant};

#[test]
fn test_initial_state() {
    let mut cb = ContainerCrashCircuitBreaker::new(5, Duration::from_secs(60), Duration::from_secs(300));
    let now = Instant::now();
    assert_eq!(cb.state(now), CircuitBreakerState::Closed);
    assert_eq!(cb.last_exit_code(), None);
    assert_eq!(cb.last_oom_killed(), None);
}

#[test]
fn test_does_not_trip_below_threshold() {
    let mut cb = ContainerCrashCircuitBreaker::new(5, Duration::from_secs(60), Duration::from_secs(300));
    let mut now = Instant::now();

    for _ in 0..4 {
        cb.record_crash(1, false, now);
        now += Duration::from_secs(1);
    }

    assert_eq!(cb.state(now), CircuitBreakerState::Closed);
}

#[test]
fn test_trips_at_threshold() {
    let mut cb = ContainerCrashCircuitBreaker::new(3, Duration::from_secs(60), Duration::from_secs(300));
    let mut now = Instant::now();

    cb.record_crash(1, false, now);
    now += Duration::from_secs(1);
    cb.record_crash(2, true, now);
    now += Duration::from_secs(1);
    cb.record_crash(137, true, now);

    assert_eq!(cb.state(now), CircuitBreakerState::Open);
    assert_eq!(cb.last_exit_code(), Some(137));
    assert_eq!(cb.last_oom_killed(), Some(true));
}

#[test]
fn test_sliding_window_eviction() {
    let mut cb = ContainerCrashCircuitBreaker::new(3, Duration::from_secs(10), Duration::from_secs(300));
    let start = Instant::now();
    let mut now = start;

    cb.record_crash(1, false, now);
    now += Duration::from_secs(5);
    cb.record_crash(1, false, now);

    // Evict the first crash by moving time forward past the window from the first crash
    now += Duration::from_secs(6); // Total time is 11 secs from start

    // This is the 3rd crash overall, but the first one should be evicted
    cb.record_crash(1, false, now);

    assert_eq!(cb.state(now), CircuitBreakerState::Closed);

    // Now if we have two more quick crashes within the window, it should trip (total 3 within 10s window)
    now += Duration::from_secs(1);
    cb.record_crash(1, false, now);
    assert_eq!(cb.state(now), CircuitBreakerState::Open);
}

#[test]
fn test_reset_manual() {
    let mut cb = ContainerCrashCircuitBreaker::new(2, Duration::from_secs(60), Duration::from_secs(300));
    let now = Instant::now();

    cb.record_crash(1, false, now);
    cb.record_crash(1, false, now);

    assert_eq!(cb.state(now), CircuitBreakerState::Open);

    cb.reset();
    assert_eq!(cb.state(now), CircuitBreakerState::Closed);
    assert_eq!(cb.last_exit_code(), None);
}

#[test]
fn test_auto_half_open_transition() {
    let mut cb = ContainerCrashCircuitBreaker::new(2, Duration::from_secs(60), Duration::from_secs(100));
    let mut now = Instant::now();

    cb.record_crash(1, false, now);
    cb.record_crash(1, false, now);

    assert_eq!(cb.state(now), CircuitBreakerState::Open);

    now += Duration::from_secs(50);
    assert_eq!(cb.state(now), CircuitBreakerState::Open);

    now += Duration::from_secs(51);
    assert_eq!(cb.state(now), CircuitBreakerState::HalfOpen);
}

#[test]
fn test_half_open_failure_trips_again() {
    let mut cb = ContainerCrashCircuitBreaker::new(2, Duration::from_secs(60), Duration::from_secs(100));
    let mut now = Instant::now();

    cb.record_crash(1, false, now);
    cb.record_crash(1, false, now);
    assert_eq!(cb.state(now), CircuitBreakerState::Open);

    now += Duration::from_secs(101);
    assert_eq!(cb.state(now), CircuitBreakerState::HalfOpen);

    // One crash in HalfOpen should trip it immediately
    cb.record_crash(137, true, now);
    assert_eq!(cb.state(now), CircuitBreakerState::Open);
    assert_eq!(cb.last_exit_code(), Some(137));
}
