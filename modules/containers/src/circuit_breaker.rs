#![forbid(unsafe_code)]

use std::collections::VecDeque;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CircuitBreakerState {
    Closed,
    Open,
    HalfOpen,
}

#[derive(Debug, Clone)]
pub struct CrashEvent {
    pub timestamp: Instant,
    pub exit_code: i32,
    pub oom_killed: bool,
}

pub struct ContainerCrashCircuitBreaker {
    threshold: usize,
    window: Duration,
    reset_timeout: Duration,
    state: CircuitBreakerState,
    crashes: VecDeque<CrashEvent>,
    last_trip_time: Option<Instant>,
    last_exit_code: Option<i32>,
    last_oom_killed: Option<bool>,
}

impl ContainerCrashCircuitBreaker {
    pub fn new(threshold: usize, window: Duration, reset_timeout: Duration) -> Self {
        Self {
            threshold,
            window,
            reset_timeout,
            state: CircuitBreakerState::Closed,
            crashes: VecDeque::new(),
            last_trip_time: None,
            last_exit_code: None,
            last_oom_killed: None,
        }
    }

    pub fn record_crash(&mut self, exit_code: i32, oom_killed: bool, now: Instant) {
        self.evict_old_crashes(now);

        match self.state {
            CircuitBreakerState::Closed | CircuitBreakerState::HalfOpen => {
                self.crashes.push_back(CrashEvent {
                    timestamp: now,
                    exit_code,
                    oom_killed,
                });

                if self.crashes.len() >= self.threshold {
                    self.trip(now, exit_code, oom_killed);
                } else if self.state == CircuitBreakerState::HalfOpen {
                    // In a real implementation, a success might be recorded to go back to Closed.
                    // But here we just record crashes. If it crashes in HalfOpen, it could trip again if threshold is reached.
                    // Or, some implementations trip immediately on one failure in HalfOpen. Let's trip immediately on failure in HalfOpen.
                    self.trip(now, exit_code, oom_killed);
                }
            }
            CircuitBreakerState::Open => {
                // If we record a crash while open, maybe update the trip time to keep it open?
                // Or just ignore it since it shouldn't be running.
                self.last_trip_time = Some(now);
                self.last_exit_code = Some(exit_code);
                self.last_oom_killed = Some(oom_killed);
            }
        }
    }

    pub fn state(&mut self, now: Instant) -> CircuitBreakerState {
        if self.state == CircuitBreakerState::Open {
            if let Some(trip_time) = self.last_trip_time {
                if now.duration_since(trip_time) >= self.reset_timeout {
                    self.state = CircuitBreakerState::HalfOpen;
                }
            }
        }
        self.state.clone()
    }

    pub fn reset(&mut self) {
        self.state = CircuitBreakerState::Closed;
        self.crashes.clear();
        self.last_trip_time = None;
        self.last_exit_code = None;
        self.last_oom_killed = None;
    }

    pub fn last_exit_code(&self) -> Option<i32> {
        self.last_exit_code
    }

    pub fn last_oom_killed(&self) -> Option<bool> {
        self.last_oom_killed
    }

    fn trip(&mut self, now: Instant, exit_code: i32, oom_killed: bool) {
        self.state = CircuitBreakerState::Open;
        self.last_trip_time = Some(now);
        self.last_exit_code = Some(exit_code);
        self.last_oom_killed = Some(oom_killed);
        self.crashes.clear();
    }

    fn evict_old_crashes(&mut self, now: Instant) {
        while let Some(front) = self.crashes.front() {
            if now.duration_since(front.timestamp) > self.window {
                self.crashes.pop_front();
            } else {
                break;
            }
        }
    }
}
