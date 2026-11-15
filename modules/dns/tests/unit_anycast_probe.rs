#![forbid(unsafe_code)]

use dns::anycast_probe::{
    AnycastHealthProber, AnycastProberConfig, DnsHealthMetrics, RouteState,
};
use std::time::Duration;

#[test]
fn test_healthy_udp() {
    let mut prober = AnycastHealthProber::new(AnycastProberConfig::default());

    let metrics = DnsHealthMetrics {
        udp_latency: Some(Duration::from_millis(50)),
        tcp_latency: None,
        bind9_process_up: true,
    };

    assert_eq!(prober.evaluate_metrics(&metrics), RouteState::Withdraw);
    assert_eq!(prober.evaluate_metrics(&metrics), RouteState::Withdraw);
    assert_eq!(prober.evaluate_metrics(&metrics), RouteState::Up);
}

#[test]
fn test_healthy_tcp_fallback() {
    let mut prober = AnycastHealthProber::new(AnycastProberConfig::default());

    let metrics = DnsHealthMetrics {
        udp_latency: Some(Duration::from_millis(500)), // Over max UDP latency
        tcp_latency: Some(Duration::from_millis(150)),
        bind9_process_up: true,
    };

    assert_eq!(prober.evaluate_metrics(&metrics), RouteState::Withdraw);
    assert_eq!(prober.evaluate_metrics(&metrics), RouteState::Withdraw);
    assert_eq!(prober.evaluate_metrics(&metrics), RouteState::Up);
}

#[test]
fn test_unhealthy_bind9_down() {
    let mut prober = AnycastHealthProber::new(AnycastProberConfig::default());
    prober.current_state = RouteState::Up;

    let metrics = DnsHealthMetrics {
        udp_latency: Some(Duration::from_millis(50)),
        tcp_latency: None,
        bind9_process_up: false,
    };

    assert_eq!(prober.evaluate_metrics(&metrics), RouteState::Up);
    assert_eq!(prober.evaluate_metrics(&metrics), RouteState::Up);
    assert_eq!(prober.evaluate_metrics(&metrics), RouteState::Withdraw);
}

#[test]
fn test_unhealthy_latency_too_high() {
    let mut prober = AnycastHealthProber::new(AnycastProberConfig::default());
    prober.current_state = RouteState::Up;

    let metrics = DnsHealthMetrics {
        udp_latency: Some(Duration::from_millis(200)), // Over max UDP latency
        tcp_latency: Some(Duration::from_millis(300)), // Over max TCP latency
        bind9_process_up: true,
    };

    assert_eq!(prober.evaluate_metrics(&metrics), RouteState::Up);
    assert_eq!(prober.evaluate_metrics(&metrics), RouteState::Up);
    assert_eq!(prober.evaluate_metrics(&metrics), RouteState::Withdraw);
}

#[test]
fn test_recovery_after_failure() {
    let mut prober = AnycastHealthProber::new(AnycastProberConfig::default());

    let good_metrics = DnsHealthMetrics {
        udp_latency: Some(Duration::from_millis(50)),
        tcp_latency: None,
        bind9_process_up: true,
    };

    let bad_metrics = DnsHealthMetrics {
        udp_latency: Some(Duration::from_millis(200)),
        tcp_latency: Some(Duration::from_millis(300)),
        bind9_process_up: true,
    };

    // Become healthy
    prober.evaluate_metrics(&good_metrics);
    prober.evaluate_metrics(&good_metrics);
    assert_eq!(prober.evaluate_metrics(&good_metrics), RouteState::Up);

    // Fail
    prober.evaluate_metrics(&bad_metrics);
    prober.evaluate_metrics(&bad_metrics);
    assert_eq!(prober.evaluate_metrics(&bad_metrics), RouteState::Withdraw);

    // Recover
    prober.evaluate_metrics(&good_metrics);
    prober.evaluate_metrics(&good_metrics);
    assert_eq!(prober.evaluate_metrics(&good_metrics), RouteState::Up);
}

#[test]
fn test_no_udp_latency_but_tcp_latency_ok() {
    let mut prober = AnycastHealthProber::new(AnycastProberConfig::default());

    let metrics = DnsHealthMetrics {
        udp_latency: None,
        tcp_latency: Some(Duration::from_millis(150)),
        bind9_process_up: true,
    };

    assert_eq!(prober.evaluate_metrics(&metrics), RouteState::Withdraw);
    assert_eq!(prober.evaluate_metrics(&metrics), RouteState::Withdraw);
    assert_eq!(prober.evaluate_metrics(&metrics), RouteState::Up);
}
