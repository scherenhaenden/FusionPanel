#![forbid(unsafe_code)]

use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct CachedRecord<T> {
    pub data: T,
    pub arrival_time: Instant,
    pub initial_ttl: Duration,
}

impl<T> CachedRecord<T> {
    /// Creates a new `CachedRecord` with the given data, arrival time, and initial TTL.
    pub fn new(data: T, arrival_time: Instant, initial_ttl: Duration) -> Self {
        Self {
            data,
            arrival_time,
            initial_ttl,
        }
    }

    /// Calculates the remaining TTL at the given query time.
    pub fn remaining_ttl_at(&self, query_time: Instant) -> Duration {
        let elapsed = query_time
            .checked_duration_since(self.arrival_time)
            .unwrap_or(Duration::ZERO);
        self.initial_ttl.saturating_sub(elapsed)
    }

    /// Checks if the record is expired at the given query time.
    pub fn is_expired_at(&self, query_time: Instant) -> bool {
        self.remaining_ttl_at(query_time) == Duration::ZERO
    }
}

/// Calculates the minimum remaining TTL for a batch of records at the given query time.
pub fn min_remaining_ttl<T>(records: &[CachedRecord<T>], query_time: Instant) -> Option<Duration> {
    records.iter().map(|r| r.remaining_ttl_at(query_time)).min()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SoaStatus {
    Ok,
    NeedsRefresh,
    Expired,
}

#[derive(Debug, Clone)]
pub struct SoaTracker {
    pub zone_name: String,
    pub refresh_interval: Duration,
    pub retry_interval: Duration,
    pub expire_interval: Duration,
    pub last_success: Instant,
    pub last_attempt: Instant,
}

impl SoaTracker {
    /// Creates a new `SoaTracker` for a secondary zone.
    pub fn new(
        zone_name: String,
        refresh_interval: Duration,
        retry_interval: Duration,
        expire_interval: Duration,
        now: Instant,
    ) -> Self {
        Self {
            zone_name,
            refresh_interval,
            retry_interval,
            expire_interval,
            last_success: now,
            last_attempt: now,
        }
    }

    /// Evaluates the SOA status at the given query time.
    pub fn status_at(&self, query_time: Instant) -> SoaStatus {
        let elapsed_since_success = query_time
            .checked_duration_since(self.last_success)
            .unwrap_or(Duration::ZERO);

        if elapsed_since_success >= self.expire_interval {
            return SoaStatus::Expired;
        }

        if elapsed_since_success >= self.refresh_interval {
            if self.last_attempt > self.last_success {
                let elapsed_since_attempt = query_time
                    .checked_duration_since(self.last_attempt)
                    .unwrap_or(Duration::ZERO);
                if elapsed_since_attempt >= self.retry_interval {
                    return SoaStatus::NeedsRefresh;
                }
            } else {
                return SoaStatus::NeedsRefresh;
            }
        }

        SoaStatus::Ok
    }

    /// Marks that a refresh attempt was made at the given time.
    pub fn mark_attempt(&mut self, now: Instant) {
        self.last_attempt = now;
    }

    /// Marks that a successful refresh occurred at the given time.
    pub fn mark_success(&mut self, now: Instant) {
        self.last_success = now;
        self.last_attempt = now;
    }
}
