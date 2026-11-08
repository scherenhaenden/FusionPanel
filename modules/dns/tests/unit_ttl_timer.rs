#![forbid(unsafe_code)]

use dns::ttl_timer::{min_remaining_ttl, CachedRecord, SoaStatus, SoaTracker};
use std::time::{Duration, Instant};

#[test]
fn test_cached_record_remaining_ttl() {
    let arrival_time = Instant::now();
    let initial_ttl = Duration::from_secs(300); // 5 minutes
    let record = CachedRecord::new("A 192.0.2.1", arrival_time, initial_ttl);

    // At arrival time, remaining TTL should be initial TTL
    assert_eq!(record.remaining_ttl_at(arrival_time), initial_ttl);
    assert!(!record.is_expired_at(arrival_time));

    // After 100 seconds
    let query_time_1 = arrival_time + Duration::from_secs(100);
    assert_eq!(
        record.remaining_ttl_at(query_time_1),
        Duration::from_secs(200)
    );
    assert!(!record.is_expired_at(query_time_1));

    // At exact expiration
    let query_time_2 = arrival_time + initial_ttl;
    assert_eq!(record.remaining_ttl_at(query_time_2), Duration::ZERO);
    assert!(record.is_expired_at(query_time_2));

    // After expiration (should not underflow, should remain 0)
    let query_time_3 = arrival_time + Duration::from_secs(400);
    assert_eq!(record.remaining_ttl_at(query_time_3), Duration::ZERO);
    assert!(record.is_expired_at(query_time_3));

    // Negative elapsed time (clock skew)
    let query_time_early = arrival_time - Duration::from_secs(10);
    assert_eq!(record.remaining_ttl_at(query_time_early), initial_ttl);
}

#[test]
fn test_min_remaining_ttl_batch() {
    let arrival_time = Instant::now();
    let r1 = CachedRecord::new("A 192.0.2.1", arrival_time, Duration::from_secs(300));
    let r2 = CachedRecord::new(
        "A 192.0.2.2",
        arrival_time + Duration::from_secs(50),
        Duration::from_secs(200),
    );
    let r3 = CachedRecord::new("A 192.0.2.3", arrival_time, Duration::from_secs(400));

    let batch = vec![r1, r2, r3];

    // At arrival_time
    // r1: 300
    // r2: arrived in future (50s), at arrival_time elapsed is 0, remaining is 200
    // r3: 400
    // Min is 200
    assert_eq!(
        min_remaining_ttl(&batch, arrival_time),
        Some(Duration::from_secs(200))
    );

    // At arrival_time + 100s
    // r1: 300 - 100 = 200
    // r2: arrived 50s after arrival_time. (arrival_time + 100s) - (arrival_time + 50s) = 50s elapsed. remaining = 200 - 50 = 150
    // r3: 400 - 100 = 300
    // Min is 150
    let query_time = arrival_time + Duration::from_secs(100);
    assert_eq!(
        min_remaining_ttl(&batch, query_time),
        Some(Duration::from_secs(150))
    );

    // Empty batch
    let empty_batch: Vec<CachedRecord<&str>> = vec![];
    assert_eq!(min_remaining_ttl(&empty_batch, arrival_time), None);
}

#[test]
fn test_soa_tracker_transitions() {
    let start_time = Instant::now();
    let refresh_interval = Duration::from_secs(3600); // 1 hour
    let retry_interval = Duration::from_secs(600);    // 10 minutes
    let expire_interval = Duration::from_secs(86400); // 24 hours

    let mut tracker = SoaTracker::new(
        "example.com".to_string(),
        refresh_interval,
        retry_interval,
        expire_interval,
        start_time,
    );

    // Initially Ok
    assert_eq!(tracker.status_at(start_time), SoaStatus::Ok);

    // Still Ok before refresh
    let query_1 = start_time + Duration::from_secs(3000);
    assert_eq!(tracker.status_at(query_1), SoaStatus::Ok);

    // Needs refresh exactly at refresh_interval
    let query_2 = start_time + refresh_interval;
    assert_eq!(tracker.status_at(query_2), SoaStatus::NeedsRefresh);

    // Needs refresh past refresh_interval
    let query_3 = start_time + refresh_interval + Duration::from_secs(10);
    assert_eq!(tracker.status_at(query_3), SoaStatus::NeedsRefresh);

    // Attempt refresh but fail
    tracker.mark_attempt(query_3);

    // Status should be Ok? No, it should not prompt another attempt immediately.
    // If last_attempt > last_success, we check if elapsed since last_attempt >= retry_interval.
    // Right after attempt:
    assert_eq!(tracker.status_at(query_3), SoaStatus::Ok);

    // Before retry interval
    let query_4 = query_3 + Duration::from_secs(300);
    assert_eq!(tracker.status_at(query_4), SoaStatus::Ok);

    // At retry interval
    let query_5 = query_3 + retry_interval;
    assert_eq!(tracker.status_at(query_5), SoaStatus::NeedsRefresh);

    // Before expire interval
    let query_6 = start_time + expire_interval - Duration::from_secs(1);
    assert_eq!(tracker.status_at(query_6), SoaStatus::NeedsRefresh);

    // At expire interval (relative to last_success)
    let query_7 = start_time + expire_interval;
    assert_eq!(tracker.status_at(query_7), SoaStatus::Expired);

    // Mark success at query_7, everything resets relative to query_7
    tracker.mark_success(query_7);
    assert_eq!(tracker.status_at(query_7), SoaStatus::Ok);

    // Needs refresh again after 1 hour from query_7
    let query_8 = query_7 + refresh_interval;
    assert_eq!(tracker.status_at(query_8), SoaStatus::NeedsRefresh);
}
