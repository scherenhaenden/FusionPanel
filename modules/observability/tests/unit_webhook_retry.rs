#![forbid(unsafe_code)]

use observability::webhook_retry::{RetryPolicy, WebhookDeliveryTask, WebhookRetryQueue};
use std::time::Instant;

#[test]
fn test_exponential_delay_calculation() {
    let policy = RetryPolicy::new(5, 1000, 10000, 0.0);
    
    // Test base exponential calculation with 0 jitter
    let delay1 = policy.compute_delay(1);
    assert_eq!(delay1.as_millis(), 1000);
    
    let delay2 = policy.compute_delay(2);
    assert_eq!(delay2.as_millis(), 2000);
    
    let delay3 = policy.compute_delay(3);
    assert_eq!(delay3.as_millis(), 4000);
    
    let delay4 = policy.compute_delay(4);
    assert_eq!(delay4.as_millis(), 8000);
    
    let delay5 = policy.compute_delay(5); // Cap at max_delay_ms
    assert_eq!(delay5.as_millis(), 10000);
    
    // Test safe overflow calculation
    let delay6 = policy.compute_delay(70);
    assert_eq!(delay6.as_millis(), 10000);

    // Test jitter factor
    let policy_jitter = RetryPolicy::new(5, 1000, 10000, 0.5); // +/- 50%
    let delay1_jitter = policy_jitter.compute_delay(1);
    assert!(delay1_jitter.as_millis() >= 500 && delay1_jitter.as_millis() <= 1500, "Jitter delay out of bounds: {}", delay1_jitter.as_millis());
}

#[test]
fn test_dead_letter_ejection() {
    let policy = RetryPolicy::new(3, 1000, 10000, 0.0);
    let mut queue = WebhookRetryQueue::new(policy);
    
    let task = WebhookDeliveryTask::new("http://example.com/webhook".to_string(), b"{}".to_vec());
    queue.enqueue(task);
    
    let popped_task = queue.pop_ready(Instant::now()).unwrap();
    
    // First failure -> attempt 1
    queue.record_failure(popped_task, Some(500), Some("Internal Server Error".to_string()));
    assert_eq!(queue.pending.len(), 1);
    assert_eq!(queue.dead_letters.len(), 0);
    
    // Since it's a binary heap, we pop it, modify next_attempt_at, and push it back (though in real code time just passes)
    let mut item = queue.pending.pop().unwrap();
    item.next_attempt_at = Instant::now();
    queue.pending.push(item);

    let popped_task2 = queue.pop_ready(Instant::now()).unwrap();
    
    // Second failure -> attempt 2
    queue.record_failure(popped_task2, Some(502), Some("Bad Gateway".to_string()));
    assert_eq!(queue.pending.len(), 1);
    assert_eq!(queue.dead_letters.len(), 0);
    
    let mut item2 = queue.pending.pop().unwrap();
    item2.next_attempt_at = Instant::now();
    queue.pending.push(item2);

    let popped_task3 = queue.pop_ready(Instant::now()).unwrap();
    
    // Third failure -> attempt 3 (max_retries reached) -> dead-letter queue
    queue.record_failure(popped_task3, Some(504), Some("Gateway Timeout".to_string()));
    assert_eq!(queue.pending.len(), 0);
    assert_eq!(queue.dead_letters.len(), 1);
    
    let dl_task = &queue.dead_letters[0];
    assert_eq!(dl_task.attempt_count, 3);
    assert_eq!(dl_task.last_status, Some(504));
    assert_eq!(dl_task.last_error, Some("Gateway Timeout".to_string()));
}

#[test]
fn test_retry_state_progression() {
    let policy = RetryPolicy::new(5, 100, 1000, 0.0);
    let mut queue = WebhookRetryQueue::new(policy);
    
    let task = WebhookDeliveryTask::new("http://example.com".to_string(), b"test".to_vec());
    queue.enqueue(task);
    
    let popped = queue.pop_ready(Instant::now()).unwrap();
    assert_eq!(popped.attempt_count, 0);
    assert_eq!(popped.last_attempted_at, None);
    assert_eq!(popped.last_status, None);
    assert_eq!(popped.last_error, None);
    
    queue.record_failure(popped, Some(408), Some("Request Timeout".to_string()));
    
    let pending_item = queue.pending.peek().unwrap();
    let pending_task = &pending_item.task;
    assert_eq!(pending_task.attempt_count, 1);
    assert!(pending_task.last_attempted_at.is_some());
    assert!(pending_task.next_attempt_at.is_some());
    assert_eq!(pending_task.last_status, Some(408));
    assert_eq!(pending_task.last_error, Some("Request Timeout".to_string()));
}

#[test]
fn test_hmac_signature() {
    let secret = "my_super_secret";
    let payload = b"{\"hello\": \"world\"}";
    
    let task = WebhookDeliveryTask::new("http://example.com".to_string(), payload.to_vec())
        .with_hmac_signature(secret);
        
    assert!(task.signature.is_some());
    
    let expected_sig = WebhookRetryQueue::generate_signature(secret, payload).unwrap();
    assert_eq!(task.signature.unwrap(), expected_sig);
}
