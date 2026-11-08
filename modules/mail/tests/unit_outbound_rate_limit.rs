#![forbid(unsafe_code)]

use mail::outbound_rate_limit::{OutboundRateLimiter, QuotaConfig, RateLimitDecision, SubmissionRequest};
use std::collections::HashMap;

#[test]
fn test_allowance_under_quota() {
    let mut mailbox_limits = HashMap::new();
    mailbox_limits.insert("user@example.com".to_string(), QuotaConfig { limit: 10, window_secs: 3600 });
    
    let limiter = OutboundRateLimiter::new(mailbox_limits, HashMap::new());
    
    let req = SubmissionRequest {
        sender: "user@example.com".to_string(),
        recipient_count: 5,
        timestamp: 1000,
    };
    
    let decision = limiter.evaluate(&req);
    assert_eq!(decision, RateLimitDecision::Allowed { remaining: 5, reset_after_secs: 3600 });
    
    let req2 = SubmissionRequest {
        sender: "user@example.com".to_string(),
        recipient_count: 4,
        timestamp: 1000,
    };
    
    let decision2 = limiter.evaluate(&req2);
    assert_eq!(decision2, RateLimitDecision::Allowed { remaining: 1, reset_after_secs: 3600 });
}

#[test]
fn test_rejection_over_quota() {
    let mut mailbox_limits = HashMap::new();
    mailbox_limits.insert("user@example.com".to_string(), QuotaConfig { limit: 10, window_secs: 3600 });
    
    let limiter = OutboundRateLimiter::new(mailbox_limits, HashMap::new());
    
    let req = SubmissionRequest {
        sender: "user@example.com".to_string(),
        recipient_count: 8,
        timestamp: 1000,
    };
    
    assert!(matches!(limiter.evaluate(&req), RateLimitDecision::Allowed { .. }));
    
    let req2 = SubmissionRequest {
        sender: "user@example.com".to_string(),
        recipient_count: 3,
        timestamp: 1005,
    };
    
    let decision2 = limiter.evaluate(&req2);
    assert_eq!(decision2, RateLimitDecision::Exceeded { reset_after_secs: 3595, limit: 10 });
}

#[test]
fn test_recipient_multiplier() {
    let mut mailbox_limits = HashMap::new();
    mailbox_limits.insert("user@example.com".to_string(), QuotaConfig { limit: 5, window_secs: 3600 });
    
    let limiter = OutboundRateLimiter::new(mailbox_limits, HashMap::new());
    
    let req = SubmissionRequest {
        sender: "user@example.com".to_string(),
        recipient_count: 6,
        timestamp: 1000,
    };
    
    let decision = limiter.evaluate(&req);
    // If no previous requests exist, calculate_reset should return the full window_secs
    assert_eq!(decision, RateLimitDecision::Exceeded { reset_after_secs: 3600, limit: 5 });
}

#[test]
fn test_reset_after_duration() {
    let mut mailbox_limits = HashMap::new();
    mailbox_limits.insert("user@example.com".to_string(), QuotaConfig { limit: 5, window_secs: 3600 });
    
    let limiter = OutboundRateLimiter::new(mailbox_limits, HashMap::new());
    
    let req1 = SubmissionRequest {
        sender: "user@example.com".to_string(),
        recipient_count: 5,
        timestamp: 1000,
    };
    assert!(matches!(limiter.evaluate(&req1), RateLimitDecision::Allowed { .. }));
    
    let req2 = SubmissionRequest {
        sender: "user@example.com".to_string(),
        recipient_count: 1,
        timestamp: 4500, // Still within 3600 of 1000
    };
    let dec2 = limiter.evaluate(&req2);
    assert_eq!(dec2, RateLimitDecision::Exceeded { reset_after_secs: 100, limit: 5 });
    
    let req3 = SubmissionRequest {
        sender: "user@example.com".to_string(),
        recipient_count: 1,
        timestamp: 4601, // Past the 3600 window of 1000
    };
    assert!(matches!(limiter.evaluate(&req3), RateLimitDecision::Allowed { remaining: 4, reset_after_secs: 3600 }));
}

#[test]
fn test_domain_limit_enforcement() {
    let mut mailbox_limits = HashMap::new();
    mailbox_limits.insert("user1@example.com".to_string(), QuotaConfig { limit: 10, window_secs: 3600 });
    mailbox_limits.insert("user2@example.com".to_string(), QuotaConfig { limit: 10, window_secs: 3600 });
    
    let mut domain_limits = HashMap::new();
    domain_limits.insert("example.com".to_string(), QuotaConfig { limit: 15, window_secs: 3600 });
    
    let limiter = OutboundRateLimiter::new(mailbox_limits, domain_limits);
    
    let req1 = SubmissionRequest {
        sender: "user1@example.com".to_string(),
        recipient_count: 10,
        timestamp: 1000,
    };
    let dec1 = limiter.evaluate(&req1);
    assert_eq!(dec1, RateLimitDecision::Allowed { remaining: 0, reset_after_secs: 3600 });
    
    let req2 = SubmissionRequest {
        sender: "user2@example.com".to_string(),
        recipient_count: 6,
        timestamp: 1005,
    };
    let dec2 = limiter.evaluate(&req2);
    assert_eq!(dec2, RateLimitDecision::Exceeded { reset_after_secs: 3595, limit: 15 });
}
