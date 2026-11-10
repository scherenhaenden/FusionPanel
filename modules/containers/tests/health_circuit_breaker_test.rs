#![forbid(unsafe_code)]

use containers::health_circuit_breaker::{
    CircuitBreakerConfig, CircuitBreakerError, CircuitBreakerState, ContainerCircuitBreaker,
};
use std::time::Duration;

#[test]
fn test_initial_state() {
    let config = CircuitBreakerConfig::default();
    let mut cb = ContainerCircuitBreaker::new(config).unwrap();

    assert_eq!(*cb.state(), CircuitBreakerState::Closed);
    assert_eq!(cb.consecutive_opens(), 0);
    assert!(cb.can_attempt().is_ok());
}

#[test]
fn test_invalid_config() {
    let config = CircuitBreakerConfig {
        failure_threshold: 0,
        ..Default::default()
    };

    let err = ContainerCircuitBreaker::new(config).unwrap_err();
    assert_eq!(
        err,
        CircuitBreakerError::InvalidConfig("failure_threshold must be > 0".to_string())
    );
}

#[test]
fn test_trip_and_half_open() {
    let config = CircuitBreakerConfig {
        failure_threshold: 3,
        failure_window: Duration::from_secs(60),
        base_backoff: Duration::from_secs(10),
        max_backoff: Duration::from_secs(300),
    };
    let mut cb = ContainerCircuitBreaker::new(config).unwrap();
    let start_time = 1000;

    // 1st failure
    cb.record_failure_at(start_time);
    assert_eq!(*cb.state(), CircuitBreakerState::Closed);

    // 2nd failure
    cb.record_failure_at(start_time + 10);
    assert_eq!(*cb.state(), CircuitBreakerState::Closed);

    // 3rd failure - trips!
    cb.record_failure_at(start_time + 20);
    assert_eq!(*cb.state(), CircuitBreakerState::Open);
    assert_eq!(cb.consecutive_opens(), 1);

    // Trying to attempt immediately fails
    let err = cb.can_attempt_at(start_time + 20).unwrap_err();
    match err {
        CircuitBreakerError::Open(delay) => {
            // delay should be >= start_time + 20 + 10
            assert!(delay >= start_time + 20 + 10);
        }
        _ => panic!("Expected Open error"),
    }

    // Time advances past backoff
    let advance_time = start_time + 20 + 15;

    assert!(cb.can_attempt_at(advance_time).is_ok());
    assert_eq!(*cb.state(), CircuitBreakerState::HalfOpen);

    // Succeeding in HalfOpen resets
    cb.record_success();
    assert_eq!(*cb.state(), CircuitBreakerState::Closed);
    assert_eq!(cb.consecutive_opens(), 0);
}

#[test]
fn test_sliding_window_expiration() {
    let config = CircuitBreakerConfig {
        failure_threshold: 3,
        failure_window: Duration::from_secs(60),
        ..Default::default()
    };
    let mut cb = ContainerCircuitBreaker::new(config).unwrap();

    let t1 = 1000;
    cb.record_failure_at(t1);

    let t2 = t1 + 30; // 30s later (1030)
    cb.record_failure_at(t2);

    let t3 = t1 + 65; // t1 has expired, t3 is 1065
    cb.record_failure_at(t3);
    assert_eq!(*cb.state(), CircuitBreakerState::Closed); // Only 2 active failures (t2, t3)

    let t4 = t1 + 80; // 1080. t2 is 1030. 1080 - 1030 = 50 <= 60. So t2, t3, t4 are active.
    cb.record_failure_at(t4);
    assert_eq!(*cb.state(), CircuitBreakerState::Open);
}

#[test]
fn test_exponential_backoff() {
    let config = CircuitBreakerConfig {
        failure_threshold: 1,
        failure_window: Duration::from_secs(60),
        base_backoff: Duration::from_secs(10),
        max_backoff: Duration::from_secs(100),
    };
    let mut cb = ContainerCircuitBreaker::new(config).unwrap();

    let mut current_time = 1000;

    // 1st trip
    cb.record_failure_at(current_time);
    assert_eq!(*cb.state(), CircuitBreakerState::Open);

    let err1 = cb.can_attempt_at(current_time).unwrap_err();
    let delay1 = match err1 {
        CircuitBreakerError::Open(d) => d - current_time,
        _ => panic!(),
    };
    assert!(delay1 >= 10 && delay1 <= 12);

    current_time += delay1;
    assert!(cb.can_attempt_at(current_time).is_ok());

    // 2nd trip
    cb.record_failure_at(current_time);
    let err2 = cb.can_attempt_at(current_time).unwrap_err();
    let delay2 = match err2 {
        CircuitBreakerError::Open(d) => d - current_time,
        _ => panic!(),
    };
    assert!(delay2 >= 20 && delay2 <= 24);

    current_time += delay2;
    assert!(cb.can_attempt_at(current_time).is_ok());

    // 3rd trip
    cb.record_failure_at(current_time);
    let err3 = cb.can_attempt_at(current_time).unwrap_err();
    let delay3 = match err3 {
        CircuitBreakerError::Open(d) => d - current_time,
        _ => panic!(),
    };
    assert!(delay3 >= 40 && delay3 <= 48);
}

#[test]
fn test_backoff_max_clamp() {
    let config = CircuitBreakerConfig {
        failure_threshold: 1,
        failure_window: Duration::from_secs(60),
        base_backoff: Duration::from_secs(10),
        max_backoff: Duration::from_secs(30),
    };
    let mut cb = ContainerCircuitBreaker::new(config).unwrap();

    let current_time = 1000;

    // Trip multiple times to exceed max_backoff
    for i in 0..5 {
        cb.record_failure_at(current_time + i); // Will be in Open/HalfOpen state
        if i < 4 { // Don't advance on the last one
            cb.can_attempt_at(current_time + i + 100).unwrap();
        }
    }

    let err = cb.can_attempt_at(current_time + 4).unwrap_err();
    let delay = match err {
        CircuitBreakerError::Open(d) => d - (current_time + 4),
        _ => panic!(),
    };

    assert_eq!(delay, 30);
}
