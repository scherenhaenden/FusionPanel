#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum CircuitBreakerError {
    #[error("Circuit breaker is OPEN, restarts are delayed until {0}")]
    Open(u64),
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CircuitBreakerState {
    Closed,
    Open,
    HalfOpen,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitBreakerConfig {
    pub failure_threshold: usize,
    pub failure_window: Duration,
    pub base_backoff: Duration,
    pub max_backoff: Duration,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 3,
            failure_window: Duration::from_secs(60),
            base_backoff: Duration::from_secs(10),
            max_backoff: Duration::from_secs(300),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerCircuitBreaker {
    config: CircuitBreakerConfig,
    state: CircuitBreakerState,
    failure_timestamps: Vec<u64>,
    open_timestamp: Option<u64>,
    consecutive_opens: u32,
}

impl ContainerCircuitBreaker {
    pub fn new(config: CircuitBreakerConfig) -> Result<Self, CircuitBreakerError> {
        if config.failure_threshold == 0 {
            return Err(CircuitBreakerError::InvalidConfig("failure_threshold must be > 0".to_string()));
        }
        Ok(Self {
            config,
            state: CircuitBreakerState::Closed,
            failure_timestamps: Vec::new(),
            open_timestamp: None,
            consecutive_opens: 0,
        })
    }

    pub fn state(&self) -> &CircuitBreakerState {
        &self.state
    }

    pub fn consecutive_opens(&self) -> u32 {
        self.consecutive_opens
    }

    pub fn record_failure(&mut self) {
        self.record_failure_at(Self::current_timestamp());
    }

    pub fn record_failure_at(&mut self, now: u64) {
        match self.state {
            CircuitBreakerState::Closed => {
                self.failure_timestamps.push(now);
                self.clean_old_failures(now);

                if self.failure_timestamps.len() >= self.config.failure_threshold {
                    self.trip(now);
                }
            }
            CircuitBreakerState::HalfOpen => {
                self.trip(now);
            }
            CircuitBreakerState::Open => {
                // Usually not probed in Open, do nothing
            }
        }
    }

    pub fn record_success(&mut self) {
        match self.state {
            CircuitBreakerState::HalfOpen | CircuitBreakerState::Open => {
                self.reset();
            }
            CircuitBreakerState::Closed => {
                self.failure_timestamps.clear();
            }
        }
    }

    pub fn can_attempt(&mut self) -> Result<(), CircuitBreakerError> {
        self.can_attempt_at(Self::current_timestamp())
    }

    pub fn can_attempt_at(&mut self, now: u64) -> Result<(), CircuitBreakerError> {
        match self.state {
            CircuitBreakerState::Closed => Ok(()),
            CircuitBreakerState::Open => {
                if let Some(open_ts) = self.open_timestamp {
                    let delay = self.calculate_backoff(now).as_secs();
                    if now >= open_ts + delay {
                        self.state = CircuitBreakerState::HalfOpen;
                        Ok(())
                    } else {
                        Err(CircuitBreakerError::Open(open_ts + delay))
                    }
                } else {
                    self.state = CircuitBreakerState::HalfOpen;
                    Ok(())
                }
            }
            CircuitBreakerState::HalfOpen => {
                Ok(())
            }
        }
    }

    fn trip(&mut self, now: u64) {
        self.state = CircuitBreakerState::Open;
        self.open_timestamp = Some(now);
        self.consecutive_opens = self.consecutive_opens.saturating_add(1);
        self.failure_timestamps.clear();
    }

    fn reset(&mut self) {
        self.state = CircuitBreakerState::Closed;
        self.open_timestamp = None;
        self.consecutive_opens = 0;
        self.failure_timestamps.clear();
    }

    fn clean_old_failures(&mut self, now: u64) {
        let window_secs = self.config.failure_window.as_secs();
        self.failure_timestamps.retain(|&ts| now.saturating_sub(ts) <= window_secs);
    }

    fn calculate_backoff(&self, now: u64) -> Duration {
        if self.consecutive_opens == 0 {
            return self.config.base_backoff;
        }

        let multiplier = 2_u32.saturating_pow(self.consecutive_opens.saturating_sub(1));
        let base_secs = self.config.base_backoff.as_secs();
        let backoff_secs = base_secs.saturating_mul(multiplier as u64);

        let backoff = Duration::from_secs(backoff_secs);

        // Pseudo-random jitter based on the provided timestamp
        let jitter_factor = (now % 21) as u64; // 0 to 20 percent
        let jitter = backoff.as_secs() * jitter_factor / 100;
        let mut final_backoff = Duration::from_secs(backoff.as_secs() + jitter);

        if final_backoff > self.config.max_backoff {
            final_backoff = self.config.max_backoff;
        }

        final_backoff
    }

    fn current_timestamp() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }
}
