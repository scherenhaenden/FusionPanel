#![forbid(unsafe_code)]

use std::time::{Duration, Instant};

#[path = "../src/health_flapping.rs"]
mod health_flapping;

use health_flapping::{ContainerHealthFilter, FilteredStatus};

#[test]
fn test_grace_period() {
    let start_time = Instant::now();
    let mut filter = ContainerHealthFilter::new(
        start_time,
        Duration::from_secs(10),
        3,
        Duration::from_secs(60),
        1,
        1,
    );

    // Initial state is Starting
    let status = filter.record_probe(false, start_time + Duration::from_secs(1));
    assert_eq!(status, FilteredStatus::Starting);

    // Still in grace period
    let status = filter.record_probe(false, start_time + Duration::from_secs(9));
    assert_eq!(status, FilteredStatus::Starting);

    // Past grace period
    let status = filter.record_probe(false, start_time + Duration::from_secs(11));
    assert_eq!(status, FilteredStatus::Unhealthy);
}

#[test]
fn test_consecutive_success_and_failures() {
    let start_time = Instant::now();
    let mut filter = ContainerHealthFilter::new(
        start_time,
        Duration::from_secs(0), // No grace period
        10,
        Duration::from_secs(60),
        2, // success threshold
        3, // failure threshold
    );

    let mut now = start_time;

    // Fail 1 - unknown
    now += Duration::from_secs(1);
    assert_eq!(filter.record_probe(false, now), FilteredStatus::Starting);

    // Fail 2 - unknown
    now += Duration::from_secs(1);
    assert_eq!(filter.record_probe(false, now), FilteredStatus::Starting);

    // Fail 3 - Unhealthy
    now += Duration::from_secs(1);
    assert_eq!(filter.record_probe(false, now), FilteredStatus::Unhealthy);

    // Success 1 - Still Unhealthy
    now += Duration::from_secs(1);
    assert_eq!(filter.record_probe(true, now), FilteredStatus::Unhealthy);

    // Success 2 - Healthy
    now += Duration::from_secs(1);
    assert_eq!(filter.record_probe(true, now), FilteredStatus::Healthy);
}

#[test]
fn test_flapping_threshold() {
    let start_time = Instant::now();
    let mut filter = ContainerHealthFilter::new(
        start_time,
        Duration::from_secs(0),
        3,
        Duration::from_secs(60),
        1,
        1,
    );

    let mut now = start_time;

    // Set to unhealthy (no flip since previous state is Unknown)
    now += Duration::from_secs(1);
    assert_eq!(filter.record_probe(false, now), FilteredStatus::Unhealthy);

    // Flip 1: Unhealthy -> Healthy
    now += Duration::from_secs(1);
    assert_eq!(filter.record_probe(true, now), FilteredStatus::Healthy);

    // Flip 2: Healthy -> Unhealthy
    now += Duration::from_secs(1);
    assert_eq!(filter.record_probe(false, now), FilteredStatus::Unhealthy);

    // Flip 3: Unhealthy -> Healthy (Hits threshold of 3 flips)
    now += Duration::from_secs(1);
    assert_eq!(filter.record_probe(true, now), FilteredStatus::Flapping);
}

#[test]
fn test_flapping_recovery() {
    let start_time = Instant::now();
    let window = Duration::from_secs(60);
    let mut filter = ContainerHealthFilter::new(
        start_time,
        Duration::from_secs(0),
        3,
        window,
        1,
        1,
    );

    let mut now = start_time;

    // Set to Unhealthy initially
    now += Duration::from_secs(1);
    filter.record_probe(false, now);

    // Flip 1
    now += Duration::from_secs(10);
    filter.record_probe(true, now);

    // Flip 2
    now += Duration::from_secs(10);
    filter.record_probe(false, now);

    // Flip 3 - should flap
    now += Duration::from_secs(10);
    assert_eq!(filter.record_probe(true, now), FilteredStatus::Flapping);

    // Wait until flip 1 is out of window
    // Flip 1 was at start_time + 11s.
    // Window is 60s. We need `now` > Flip 1 + 60s -> start_time + 71s.
    now = start_time + Duration::from_secs(75);

    // Let's do another probe that doesn't change state (Healthy -> Healthy)
    // This will evict old flips. We have 3 flips currently.
    // The first flip will be removed, leaving 2 flips in the window.
    // Flapping threshold is 3, so we are no longer flapping.
    assert_eq!(filter.record_probe(true, now), FilteredStatus::Healthy);
}
