use containers::health::{HealthChecker, HealthProbePolicy, HealthState};
use containers::logs::BoundedLogReader;
use std::time::Duration;

#[test]
fn test_health_transition() {
    let policy = HealthProbePolicy {
        interval: Duration::from_secs(10),
        retries: 3,
        timeout: Duration::from_secs(5),
    };

    let mut checker = HealthChecker::new(policy);

    assert_eq!(checker.get_state(), HealthState::Unknown);

    // Test transition to healthy
    checker.transition(true);
    assert_eq!(checker.get_state(), HealthState::Healthy);

    // Fail once, shouldn't transition to unhealthy immediately
    checker.transition(false);
    assert_eq!(checker.get_state(), HealthState::Healthy);

    // Fail again
    checker.transition(false);
    assert_eq!(checker.get_state(), HealthState::Healthy);

    // Fail 3rd time (reaches retries)
    checker.transition(false);
    assert_eq!(checker.get_state(), HealthState::Unhealthy);

    // Succeed again, resets to healthy
    checker.transition(true);
    assert_eq!(checker.get_state(), HealthState::Healthy);
}

#[test]
fn test_log_truncation() {
    let mut reader = BoundedLogReader::new(3);

    reader.append_log("Log 1");
    reader.append_log("Log 2");
    reader.append_log("Log 3");

    assert_eq!(reader.get_logs(), vec!["Log 1", "Log 2", "Log 3"]);

    reader.append_log("Log 4");

    assert_eq!(reader.get_logs(), vec!["Log 2", "Log 3", "Log 4"]);
}

#[test]
fn test_log_redaction() {
    let mut reader = BoundedLogReader::new(10);

    reader.append_log("User logged in");
    reader.append_log("API call with password=supersecret data");
    reader.append_log("Request failed, token: 1234567890abcdef");
    reader.append_log("My secret is safe");
    reader.append_log("apikey = foo-bar-baz");

    let logs = reader.get_logs();

    assert_eq!(logs[0], "User logged in");
    assert_eq!(logs[1], "API call with password = ***REDACTED*** data");
    assert_eq!(logs[2], "Request failed, token = ***REDACTED***");
    assert_eq!(logs[3], "My secret = ***REDACTED*** safe");
    assert_eq!(logs[4], "apikey = ***REDACTED***");
}
