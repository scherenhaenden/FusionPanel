use mail::error::MailError;

#[path = "../src/quota.rs"]
pub mod quota;

use quota::{MailboxQuota, QuotaThresholds, QuotaTracker, QuotaUsage, QuotaWarning};

#[test]
fn test_compute_status() {
    let q = MailboxQuota {
        max_bytes: 1000,
        max_messages: 10,
    };
    let tracker = QuotaTracker::new(q, QuotaThresholds::default());

    let usage = QuotaUsage {
        used_bytes: 500,
        used_messages: 2,
    };

    let status = tracker.compute_status(&usage);
    assert_eq!(status.available_bytes, 500);
    assert_eq!(status.available_messages, 8);
    assert_eq!(status.bytes_usage_percent, 50.0);
    assert_eq!(status.messages_usage_percent, 20.0);
    assert!(!status.is_exceeded);
}

#[test]
fn test_is_exceeded() {
    let q = MailboxQuota {
        max_bytes: 1000,
        max_messages: 10,
    };
    let tracker = QuotaTracker::new(q, QuotaThresholds::default());

    let usage = QuotaUsage {
        used_bytes: 1001,
        used_messages: 2,
    };

    let status = tracker.compute_status(&usage);
    assert_eq!(status.available_bytes, 0);
    assert!(status.is_exceeded);
}

#[test]
fn test_check_warning() {
    let q = MailboxQuota {
        max_bytes: 1000,
        max_messages: 10,
    };
    let tracker = QuotaTracker::new(q, QuotaThresholds::default());

    // Below warning
    let usage = QuotaUsage {
        used_bytes: 799,
        used_messages: 2,
    };
    assert_eq!(tracker.check_warning(&usage), QuotaWarning::None);

    // Warning
    let usage = QuotaUsage {
        used_bytes: 800,
        used_messages: 2,
    };
    assert_eq!(tracker.check_warning(&usage), QuotaWarning::Warning(80.0));

    let usage = QuotaUsage {
        used_bytes: 899,
        used_messages: 2,
    };
    assert_eq!(tracker.check_warning(&usage), QuotaWarning::Warning(89.9));

    // Critical
    let usage = QuotaUsage {
        used_bytes: 900,
        used_messages: 2,
    };
    assert_eq!(tracker.check_warning(&usage), QuotaWarning::Critical(90.0));

    let usage = QuotaUsage {
        used_bytes: 999,
        used_messages: 2,
    };
    assert_eq!(tracker.check_warning(&usage), QuotaWarning::Critical(99.9));

    // Exceeded
    let usage = QuotaUsage {
        used_bytes: 1000,
        used_messages: 11,
    };
    assert_eq!(tracker.check_warning(&usage), QuotaWarning::Exceeded);
}

#[test]
fn test_check_delivery_success() {
    let q = MailboxQuota {
        max_bytes: 1000,
        max_messages: 10,
    };
    let tracker = QuotaTracker::new(q, QuotaThresholds::default());

    let usage = QuotaUsage {
        used_bytes: 900,
        used_messages: 9,
    };

    assert!(tracker.check_delivery(&usage, 100).is_ok());
}

#[test]
fn test_check_delivery_exceeds_bytes() {
    let q = MailboxQuota {
        max_bytes: 1000,
        max_messages: 10,
    };
    let tracker = QuotaTracker::new(q, QuotaThresholds::default());

    let usage = QuotaUsage {
        used_bytes: 900,
        used_messages: 9,
    };

    let result = tracker.check_delivery(&usage, 101);
    assert!(matches!(result, Err(MailError::Conflict(_))));
}

#[test]
fn test_check_delivery_exceeds_messages() {
    let q = MailboxQuota {
        max_bytes: 1000,
        max_messages: 10,
    };
    let tracker = QuotaTracker::new(q, QuotaThresholds::default());

    let usage = QuotaUsage {
        used_bytes: 900,
        used_messages: 10,
    };

    let result = tracker.check_delivery(&usage, 50);
    assert!(matches!(result, Err(MailError::Conflict(_))));
}

#[test]
fn test_zero_quota() {
    let q = MailboxQuota {
        max_bytes: 0,
        max_messages: 0,
    };
    let tracker = QuotaTracker::new(q, QuotaThresholds::default());

    let usage = QuotaUsage {
        used_bytes: 0,
        used_messages: 0,
    };

    let status = tracker.compute_status(&usage);
    assert_eq!(status.bytes_usage_percent, 0.0);
    assert_eq!(status.messages_usage_percent, 0.0);
    assert!(!status.is_exceeded);

    // Any delivery should fail because quota is 0
    let result = tracker.check_delivery(&usage, 1);
    assert!(matches!(result, Err(MailError::Conflict(_))));
}
