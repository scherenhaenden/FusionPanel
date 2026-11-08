#![forbid(unsafe_code)]

use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Configuration for a specific route and method limit.
#[derive(Debug, Clone)]
pub struct RouteLimit {
    pub pattern: String,        // Prefix or exact match
    pub method: Option<String>, // e.g., "GET", "POST", None means all methods
    pub burst_capacity: usize,
    pub leak_rate: Duration, // Time per token leak
}

/// A token bucket implementation.
#[derive(Debug)]
pub struct TokenBucket {
    capacity: usize,
    leak_rate: Duration,
    tokens: f64,
    last_leak: Instant,
}

impl TokenBucket {
    pub fn new(capacity: usize, leak_rate: Duration, now: Instant) -> Self {
        Self {
            capacity,
            leak_rate,
            tokens: capacity as f64,
            last_leak: now,
        }
    }

    pub fn consume(&mut self, tokens: f64, now: Instant) -> Result<(), Duration> {
        self.replenish(now);

        if self.tokens >= tokens {
            self.tokens -= tokens;
            Ok(())
        } else {
            // Calculate how long to wait for 1 token
            let deficit = tokens - self.tokens;
            let wait_secs = deficit * self.leak_rate.as_secs_f64();
            Err(Duration::from_secs_f64(wait_secs))
        }
    }

    fn replenish(&mut self, now: Instant) {
        let elapsed = now.saturating_duration_since(self.last_leak);
        let tokens_to_add = elapsed.as_secs_f64() / self.leak_rate.as_secs_f64();

        if tokens_to_add > 0.0 {
            self.tokens = (self.tokens + tokens_to_add).min(self.capacity as f64);
            self.last_leak = now;
        }
    }

    fn is_full(&mut self, now: Instant) -> bool {
        self.replenish(now);
        self.tokens >= self.capacity as f64
    }
}

pub struct RouteRateLimiter {
    limits: Vec<RouteLimit>,
    buckets: HashMap<(String, String, String), TokenBucket>,
}

const MAX_BUCKETS: usize = 100_000;

impl RouteRateLimiter {
    pub fn new(limits: Vec<RouteLimit>) -> Self {
        Self {
            limits,
            buckets: HashMap::new(),
        }
    }

    /// Check rate limit for a given key, route path, and method.
    /// The `key` can be an IP address or User ID.
    /// Returns Ok(()) if the request is allowed.
    /// Returns Err(String) if the request is rate-limited, where the String is the value
    /// for the "Retry-After" HTTP header (the wait time in seconds rounded up).
    pub fn check(
        &mut self,
        key: &str,
        path: &str,
        method: &str,
        now: Instant,
    ) -> Result<(), String> {
        let matched_limit = self
            .limits
            .iter()
            .filter(|limit| {
                let method_match = limit
                    .method
                    .as_deref()
                    .map_or(true, |m| m.eq_ignore_ascii_case(method));
                let path_match = path_matches(path, &limit.pattern);
                method_match && path_match
            })
            .max_by_key(|limit| (limit.pattern.len(), limit.method.is_some()));

        if let Some(limit) = matched_limit {
            if limit.leak_rate.is_zero() || limit.burst_capacity == 0 {
                return Err("1".to_string());
            }

            let bucket_key = (
                key.to_string(),
                method.to_ascii_uppercase(),
                limit.pattern.clone(),
            );

            if !self.buckets.contains_key(&bucket_key) && self.buckets.len() >= MAX_BUCKETS {
                self.buckets.retain(|_, bucket| !bucket.is_full(now));
                if self.buckets.len() >= MAX_BUCKETS {
                    return Err("1".to_string());
                }
            }

            let bucket = self
                .buckets
                .entry(bucket_key)
                .or_insert_with(|| TokenBucket::new(limit.burst_capacity, limit.leak_rate, now));

            match bucket.consume(1.0, now) {
                Ok(()) => Ok(()),
                Err(wait_duration) => {
                    let wait_secs = wait_duration.as_secs_f64().ceil() as u64;
                    Err(wait_secs.to_string())
                }
            }
        } else {
            // No limit configured for this route
            Ok(())
        }
    }
}

fn path_matches(path: &str, pattern: &str) -> bool {
    path == pattern
        || path
            .strip_prefix(pattern)
            .is_some_and(|suffix| pattern.ends_with('/') || suffix.starts_with('/'))
}
