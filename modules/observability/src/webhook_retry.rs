#![forbid(unsafe_code)]

use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::time::{Duration, Instant};
use uuid::Uuid;

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Clone)]
pub struct WebhookDeliveryTask {
    pub id: Uuid,
    pub endpoint: String,
    pub payload: Vec<u8>,
    pub signature: Option<String>,
    pub attempt_count: u32,
    pub last_attempted_at: Option<Instant>,
    pub next_attempt_at: Option<Instant>,
    pub last_status: Option<u16>,
    pub last_error: Option<String>,
}

impl WebhookDeliveryTask {
    pub fn new(endpoint: String, payload: Vec<u8>) -> Self {
        Self {
            id: Uuid::new_v4(),
            endpoint,
            payload,
            signature: None,
            attempt_count: 0,
            last_attempted_at: None,
            next_attempt_at: None,
            last_status: None,
            last_error: None,
        }
    }

    pub fn with_hmac_signature(mut self, secret: &str) -> Self {
        if let Ok(mut mac) = HmacSha256::new_from_slice(secret.as_bytes()) {
            mac.update(&self.payload);
            let result = mac.finalize();
            let hex_sig = hex::encode(result.into_bytes());
            self.signature = Some(hex_sig);
        }
        self
    }
}

#[derive(Debug, Clone)]
pub struct RetryPolicy {
    pub max_retries: u32,
    pub base_delay_ms: u64,
    pub max_delay_ms: u64,
    pub jitter_factor: f64, // E.g., 0.2 means +/- 20%
}

impl RetryPolicy {
    pub fn new(
        max_retries: u32,
        base_delay_ms: u64,
        max_delay_ms: u64,
        jitter_factor: f64,
    ) -> Self {
        Self {
            max_retries,
            base_delay_ms,
            max_delay_ms,
            jitter_factor,
        }
    }

    pub fn compute_delay(&self, attempt: u32) -> Duration {
        if attempt == 0 {
            return Duration::from_millis(0);
        }

        let shift = attempt.saturating_sub(1) as u32;
        let multiplier = 2u64.checked_pow(shift).unwrap_or(u64::MAX);
        let exponential_delay = self.base_delay_ms.saturating_mul(multiplier);
        let delay = std::cmp::min(exponential_delay, self.max_delay_ms);

        let jitter = (delay as f64) * self.jitter_factor;
        let rand_val: f64 = rand::random();
        let random_jitter = (rand_val * 2.0 * jitter) - jitter;

        let final_delay = (delay as f64) + random_jitter;

        Duration::from_millis(final_delay.max(0.0) as u64)
    }
}

pub struct WebhookRetryQueue {
    policy: RetryPolicy,
    pub pending: std::collections::BinaryHeap<QueueItem>,
    pub dead_letters: Vec<WebhookDeliveryTask>,
}

#[derive(Clone)]
pub struct QueueItem {
    pub next_attempt_at: Instant,
    pub task: WebhookDeliveryTask,
}

impl PartialEq for QueueItem {
    fn eq(&self, other: &Self) -> bool {
        self.next_attempt_at == other.next_attempt_at
    }
}
impl Eq for QueueItem {}
impl PartialOrd for QueueItem {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for QueueItem {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Reverse order so BinaryHeap is a min-heap (earliest next_attempt_at first)
        other.next_attempt_at.cmp(&self.next_attempt_at)
    }
}

impl WebhookRetryQueue {
    pub fn new(policy: RetryPolicy) -> Self {
        Self {
            policy,
            pending: std::collections::BinaryHeap::new(),
            dead_letters: Vec::new(),
        }
    }

    pub fn enqueue(&mut self, mut task: WebhookDeliveryTask) {
        let now = Instant::now();
        task.next_attempt_at = Some(now);
        self.pending.push(QueueItem {
            next_attempt_at: now,
            task,
        });
    }

    pub fn pop_ready(&mut self, now: Instant) -> Option<WebhookDeliveryTask> {
        if let Some(top) = self.pending.peek() {
            if top.next_attempt_at <= now {
                return self.pending.pop().map(|i| i.task);
            }
        }
        None
    }

    pub fn record_success(&mut self, _task_id: Uuid) {
        // Success means the task was handled and does not return to the queue.
    }

    pub fn record_failure(&mut self, mut task: WebhookDeliveryTask, status: Option<u16>, error: Option<String>) {
        task.attempt_count += 1;
        task.last_attempted_at = Some(Instant::now());
        task.last_status = status;
        task.last_error = error;

        if task.attempt_count >= self.policy.max_retries {
            self.dead_letters.push(task);
        } else {
            let delay = self.policy.compute_delay(task.attempt_count);
            let next_attempt_at = Instant::now() + delay;
            task.next_attempt_at = Some(next_attempt_at);
            self.pending.push(QueueItem {
                next_attempt_at,
                task,
            });
        }
    }

    pub fn generate_signature(secret: &str, payload: &[u8]) -> Option<String> {
        if let Ok(mut mac) = HmacSha256::new_from_slice(secret.as_bytes()) {
            mac.update(payload);
            let result = mac.finalize();
            Some(hex::encode(result.into_bytes()))
        } else {
            None
        }
    }
}

impl std::fmt::Debug for QueueItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("QueueItem")
            .field("next_attempt_at", &self.next_attempt_at)
            .field("task", &self.task)
            .finish()
    }
}
