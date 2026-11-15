#![forbid(unsafe_code)]

use databases::thread_pool_guard::{ThreadPoolGuard, ThreadPoolMetrics, ThreadPoolState};

#[test]
fn test_parse_metrics() {
    let vars = vec![
        ("ThreadPool_idle_threads".to_string(), "5".to_string()),
        ("ThreadPool_active_threads".to_string(), "10".to_string()),
        ("ThreadPool_queued_requests".to_string(), "2".to_string()),
        ("Other_Var".to_string(), "123".to_string()),
    ];

    let metrics = ThreadPoolMetrics::parse(&vars);

    assert_eq!(metrics.idle_threads, 5);
    assert_eq!(metrics.active_threads, 10);
    assert_eq!(metrics.queued_requests, 2);
}

#[test]
fn test_parse_metrics_missing() {
    let vars = vec![];
    let metrics = ThreadPoolMetrics::parse(&vars);
    assert_eq!(metrics.idle_threads, 0);
    assert_eq!(metrics.active_threads, 0);
    assert_eq!(metrics.queued_requests, 0);
}

#[test]
fn test_parse_metrics_invalid() {
    let vars = vec![
        ("ThreadPool_idle_threads".to_string(), "invalid".to_string()),
    ];
    let metrics = ThreadPoolMetrics::parse(&vars);
    assert_eq!(metrics.idle_threads, 0);
}

#[test]
fn test_evaluate_healthy() {
    let metrics = ThreadPoolMetrics {
        idle_threads: 5,
        active_threads: 10,
        queued_requests: 2,
    };

    let guard = ThreadPoolGuard::evaluate(metrics.clone());
    assert_eq!(guard.state, ThreadPoolState::Healthy);
    assert_eq!(guard.recommended_size, None);
}

#[test]
fn test_evaluate_starvation() {
    let metrics = ThreadPoolMetrics {
        idle_threads: 0,
        active_threads: 10,
        queued_requests: 5,
    };

    let guard = ThreadPoolGuard::evaluate(metrics.clone());
    assert_eq!(guard.state, ThreadPoolState::WorkerStarvation);
    assert_eq!(guard.recommended_size, Some(15));
}

#[test]
fn test_evaluate_starvation_cap() {
    let metrics = ThreadPoolMetrics {
        idle_threads: 0,
        active_threads: 10,
        queued_requests: 100,
    };

    let guard = ThreadPoolGuard::evaluate(metrics.clone());
    assert_eq!(guard.state, ThreadPoolState::WorkerStarvation);
    // 10 + 16 (max recommend cap based on logic: min(16, queued_requests))
    assert_eq!(guard.recommended_size, Some(26));
}

#[test]
fn test_evaluate_latency_spike() {
    let metrics = ThreadPoolMetrics {
        idle_threads: 1, // Not 0, so not starvation by the exact rule if idle > 0
        active_threads: 10,
        queued_requests: 25, // > 10 * 2
    };

    let guard = ThreadPoolGuard::evaluate(metrics.clone());
    assert_eq!(guard.state, ThreadPoolState::QueueLatencySpike);
    assert_eq!(guard.recommended_size, None);
}

#[test]
fn test_evaluate_latency_spike_starvation_precedence() {
    let metrics = ThreadPoolMetrics {
        idle_threads: 0,
        active_threads: 10,
        queued_requests: 25, // > 10 * 2
    };

    // Should be starvation because idle is 0
    let guard = ThreadPoolGuard::evaluate(metrics.clone());
    assert_eq!(guard.state, ThreadPoolState::WorkerStarvation);
    assert_eq!(guard.recommended_size, Some(26));
}
