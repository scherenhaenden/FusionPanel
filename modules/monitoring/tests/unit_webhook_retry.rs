#![forbid(unsafe_code)]

use std::time::Duration;
use monitoring::webhook_retry::{WebhookDispatcher, RetrySchedule};

#[test]
fn test_hmac_signature() {
    let dispatcher = WebhookDispatcher::new("secret".to_string());
    let sig = dispatcher.sign_payload("test payload").expect("Signing failed");

    // Test payload with "secret" HMAC-SHA256:
    // echo -n "test payload" | openssl dgst -sha256 -hmac "secret"
    // -> f1f1fc517bb886ad22c56e51dae135aad082b2e3337bed35e2e44cd299324bd8
    let expected = "sha256=f1f1fc517bb886ad22c56e51dae135aad082b2e3337bed35e2e44cd299324bd8";
    assert_eq!(sig, expected);
}

#[test]
fn test_retry_schedule() {
    let schedule = RetrySchedule::new(
        Duration::from_secs(1),
        2.0,
        Duration::from_secs(60),
        5,
    );

    // attempt 0: delay should be 1.0 * jitter (0.8 - 1.2)
    let delay_0 = schedule.get_delay(0).unwrap().as_secs_f64();
    assert!(delay_0 >= 0.8 && delay_0 <= 1.2);

    // attempt 1: delay should be 1.0 * 2.0 = 2.0 * jitter
    let delay_1 = schedule.get_delay(1).unwrap().as_secs_f64();
    assert!(delay_1 >= 1.6 && delay_1 <= 2.4);

    // attempt 2: delay should be 1.0 * 4.0 = 4.0 * jitter
    let delay_2 = schedule.get_delay(2).unwrap().as_secs_f64();
    assert!(delay_2 >= 3.2 && delay_2 <= 4.8);

    // attempt 3: delay should be 1.0 * 8.0 = 8.0 * jitter
    let delay_3 = schedule.get_delay(3).unwrap().as_secs_f64();
    assert!(delay_3 >= 6.4 && delay_3 <= 9.6);

    // attempt 4: delay should be 1.0 * 16.0 = 16.0 * jitter
    let delay_4 = schedule.get_delay(4).unwrap().as_secs_f64();
    assert!(delay_4 >= 12.8 && delay_4 <= 19.2);

    // attempt 5: max 5 attempts means index 5 is None
    assert!(schedule.get_delay(5).is_none());
    assert!(schedule.get_delay(10).is_none());
}

#[test]
fn test_retry_schedule_max_delay() {
    let schedule = RetrySchedule::new(
        Duration::from_secs(10),
        2.0,
        Duration::from_secs(30), // Max delay
        10,
    );

    // attempt 0: 10 * jitter
    // attempt 1: 20 * jitter (could be up to 24, which is <= 30)
    // attempt 2: 40 * jitter (32 to 48), should be capped at 30

    let delay_2 = schedule.get_delay(2).unwrap();
    assert_eq!(delay_2, Duration::from_secs(30)); // Assuming the cap applies strictly and jitter doesn't reduce below max_delay? Wait, if jitter is applied first and then `.min(max_delay)`, then: 40 * 0.8 = 32. 32.min(30) = 30. Yes, it will be 30.

    // Check attempt 3 just in case
    let delay_3 = schedule.get_delay(3).unwrap();
    assert_eq!(delay_3, Duration::from_secs(30));
}
