#![forbid(unsafe_code)]

use databases::pool_telemetry::{PoolStats, PoolTelemetry};
use std::time::Duration;

#[test]
fn test_pool_telemetry_initialization() {
    let telemetry = PoolTelemetry::new(10, 100);
    assert_eq!(telemetry.stats.max_connections, 10);
    assert_eq!(telemetry.stats.active_connections, 0);
    assert_eq!(telemetry.stats.idle_connections, 0);
    assert_eq!(telemetry.stats.waiting_connections, 0);
    assert!(telemetry.calculate_percentiles().is_none());
}

#[test]
fn test_update_stats() {
    let mut telemetry = PoolTelemetry::new(10, 100);
    telemetry.update_stats(5, 3, 2);
    assert_eq!(
        telemetry.stats,
        PoolStats {
            active_connections: 5,
            idle_connections: 3,
            waiting_connections: 2,
            max_connections: 10,
        }
    );
}

#[test]
fn test_calculate_percentiles_empty() {
    let telemetry = PoolTelemetry::new(10, 100);
    assert!(telemetry.calculate_percentiles().is_none());
}

#[test]
fn test_calculate_percentiles_single() {
    let mut telemetry = PoolTelemetry::new(10, 100);
    telemetry.record_latency(Duration::from_millis(50));
    let percentiles = telemetry.calculate_percentiles().unwrap();
    assert_eq!(percentiles.p50, Duration::from_millis(50));
    assert_eq!(percentiles.p95, Duration::from_millis(50));
    assert_eq!(percentiles.p99, Duration::from_millis(50));
}

#[test]
fn test_calculate_percentiles_multiple() {
    let mut telemetry = PoolTelemetry::new(10, 100);

    // Add 100 latencies from 1ms to 100ms
    for i in 1..=100 {
        telemetry.record_latency(Duration::from_millis(i));
    }

    let percentiles = telemetry.calculate_percentiles().unwrap();

    // The exact percentile calculation may interpolate depending on logic.
    // 0.50 * 99 = 49.5 -> between 50 and 51
    assert_eq!(percentiles.p50.as_millis(), 50); // 50ms + 0.5 * (51ms - 50ms) = 50.5 => 50ms in u64 depending on interpolation

    // We check approximately depending on how interpolation is implemented.
    assert!(percentiles.p50.as_millis() >= 50 && percentiles.p50.as_millis() <= 51);
    assert!(percentiles.p95.as_millis() >= 95 && percentiles.p95.as_millis() <= 96);
    assert!(percentiles.p99.as_millis() >= 99 && percentiles.p99.as_millis() <= 100);
}

#[test]
fn test_history_limit() {
    let mut telemetry = PoolTelemetry::new(10, 5);

    // Add 10 latencies, 1 to 10
    for i in 1..=10 {
        telemetry.record_latency(Duration::from_millis(i));
    }

    // Only the last 5 should remain (6, 7, 8, 9, 10)
    let percentiles = telemetry.calculate_percentiles().unwrap();
    assert!(percentiles.p50 >= Duration::from_millis(6));
}

#[test]
fn test_pool_exhaustion_ok() {
    let mut telemetry = PoolTelemetry::new(10, 100);
    telemetry.update_stats(5, 5, 0);
    assert!(telemetry.check_pool_exhaustion().is_none());
}

#[test]
fn test_pool_exhaustion_max_reached() {
    let mut telemetry = PoolTelemetry::new(10, 100);
    telemetry.update_stats(10, 0, 0);
    let warning = telemetry.check_pool_exhaustion();
    assert!(warning.is_some());
    assert!(warning.unwrap().contains("pool exhausted"));
    // check_pool_exhaustion returns: "WARNING: Database connection pool exhausted. Active connections (10) reached or exceeded max (10)."
}

#[test]
fn test_pool_exhaustion_under_pressure() {
    let mut telemetry = PoolTelemetry::new(10, 100);
    telemetry.update_stats(9, 0, 2);
    let warning = telemetry.check_pool_exhaustion();
    assert!(warning.is_some());
    assert!(warning.unwrap().contains("under pressure"));
}
