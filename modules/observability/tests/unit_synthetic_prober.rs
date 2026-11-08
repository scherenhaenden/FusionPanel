#![forbid(unsafe_code)]

use observability::synthetic_prober::{
    AlertState, ProbeConfig, ProbeOutcome, ProbeTarget, SyntheticProber, SyntheticProberConfig,
};

#[test]
fn test_valid_http_config() {
    let config = SyntheticProberConfig {
        target: ProbeTarget::Http {
            url: "https://example.com".to_string(),
            expected_status: 200,
            expected_substring: None,
        },
        config: ProbeConfig {
            timeout_millis: 1000,
            max_retries: 3,
            interval_secs: 60,
        },
        failure_threshold: 3,
        latency_threshold_ms: 500,
    };

    let prober = SyntheticProber::from_config(config);
    assert!(prober.is_ok());
}

#[test]
fn test_invalid_http_config() {
    let config = SyntheticProberConfig {
        target: ProbeTarget::Http {
            url: "ftp://example.com".to_string(), // Invalid scheme
            expected_status: 200,
            expected_substring: None,
        },
        config: ProbeConfig {
            timeout_millis: 1000,
            max_retries: 3,
            interval_secs: 60,
        },
        failure_threshold: 3,
        latency_threshold_ms: 500,
    };

    let prober = SyntheticProber::from_config(config);
    assert!(prober.is_err());
    assert_eq!(
        prober.unwrap_err(),
        "Invalid URL scheme, must be http:// or https://"
    );
}

#[test]
fn test_valid_tcp_config() {
    let config = SyntheticProberConfig {
        target: ProbeTarget::Tcp {
            host: "example.com".to_string(),
            port: 443,
        },
        config: ProbeConfig {
            timeout_millis: 1000,
            max_retries: 3,
            interval_secs: 60,
        },
        failure_threshold: 3,
        latency_threshold_ms: 500,
    };

    let prober = SyntheticProber::from_config(config);
    assert!(prober.is_ok());
}

#[test]
fn test_invalid_tcp_config() {
    let config = SyntheticProberConfig {
        target: ProbeTarget::Tcp {
            host: "example.com".to_string(),
            port: 0, // Invalid port
        },
        config: ProbeConfig {
            timeout_millis: 1000,
            max_retries: 3,
            interval_secs: 60,
        },
        failure_threshold: 3,
        latency_threshold_ms: 500,
    };

    let prober = SyntheticProber::from_config(config);
    assert!(prober.is_err());
    assert_eq!(prober.unwrap_err(), "Invalid TCP port, cannot be 0");
}

#[test]
fn test_invalid_failure_threshold() {
    let config = SyntheticProberConfig {
        target: ProbeTarget::Tcp {
            host: "example.com".to_string(),
            port: 80,
        },
        config: ProbeConfig {
            timeout_millis: 1000,
            max_retries: 3,
            interval_secs: 60,
        },
        failure_threshold: 0, // Invalid threshold
        latency_threshold_ms: 500,
    };

    let prober = SyntheticProber::from_config(config);
    assert!(prober.is_err());
    assert_eq!(
        prober.unwrap_err(),
        "failure_threshold must be greater than 0"
    );
}

#[test]
fn test_outcome_evaluation_success() {
    let mut prober = SyntheticProber::new(
        ProbeTarget::Tcp {
            host: "localhost".to_string(),
            port: 8080,
        },
        ProbeConfig {
            timeout_millis: 1000,
            max_retries: 3,
            interval_secs: 60,
        },
        3,
        500,
    )
    .unwrap();

    let outcome = ProbeOutcome {
        success: true,
        latency_ms: 200, // Below threshold
        error_message: None,
        timestamp: 1234567890,
    };

    assert_eq!(prober.evaluate_outcome(&outcome), AlertState::Normal);
    assert_eq!(prober.consecutive_failures(), 0);
}

#[test]
fn test_outcome_evaluation_latency_failure() {
    let mut prober = SyntheticProber::new(
        ProbeTarget::Tcp {
            host: "localhost".to_string(),
            port: 8080,
        },
        ProbeConfig {
            timeout_millis: 1000,
            max_retries: 3,
            interval_secs: 60,
        },
        3,
        500,
    )
    .unwrap();

    let outcome = ProbeOutcome {
        success: true,
        latency_ms: 600, // Above threshold
        error_message: None,
        timestamp: 1234567890,
    };

    assert_eq!(prober.evaluate_outcome(&outcome), AlertState::Normal); // Still normal because threshold is 3
    assert_eq!(prober.consecutive_failures(), 1);
}

#[test]
fn test_flapping_suppression_and_alerting() {
    let mut prober = SyntheticProber::new(
        ProbeTarget::Tcp {
            host: "localhost".to_string(),
            port: 8080,
        },
        ProbeConfig {
            timeout_millis: 1000,
            max_retries: 3,
            interval_secs: 60,
        },
        3, // failure threshold
        500,
    )
    .unwrap();

    let failure_outcome = ProbeOutcome {
        success: false,
        latency_ms: 0,
        error_message: Some("Connection refused".to_string()),
        timestamp: 1234567890,
    };

    let success_outcome = ProbeOutcome {
        success: true,
        latency_ms: 100,
        error_message: None,
        timestamp: 1234567890,
    };

    // 1st failure
    assert_eq!(prober.evaluate_outcome(&failure_outcome), AlertState::Normal);
    assert_eq!(prober.consecutive_failures(), 1);

    // 2nd failure
    assert_eq!(prober.evaluate_outcome(&failure_outcome), AlertState::Normal);
    assert_eq!(prober.consecutive_failures(), 2);

    // 1 success clears the flapping
    assert_eq!(prober.evaluate_outcome(&success_outcome), AlertState::Normal);
    assert_eq!(prober.consecutive_failures(), 0);

    // 1st failure again
    assert_eq!(prober.evaluate_outcome(&failure_outcome), AlertState::Normal);
    assert_eq!(prober.consecutive_failures(), 1);

    // 2nd failure
    assert_eq!(prober.evaluate_outcome(&failure_outcome), AlertState::Normal);
    assert_eq!(prober.consecutive_failures(), 2);

    // 3rd failure triggers alert
    assert_eq!(
        prober.evaluate_outcome(&failure_outcome),
        AlertState::Alerting
    );
    assert_eq!(prober.consecutive_failures(), 3);

    // 4th failure stays in alert
    assert_eq!(
        prober.evaluate_outcome(&failure_outcome),
        AlertState::Alerting
    );
    assert_eq!(prober.consecutive_failures(), 4);

    // 1 success clears the alert and flapping
    assert_eq!(prober.evaluate_outcome(&success_outcome), AlertState::Normal);
    assert_eq!(prober.consecutive_failures(), 0);
}
