#![forbid(unsafe_code)]

use std::collections::VecDeque;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilteredStatus {
    Starting,
    Healthy,
    Unhealthy,
    Flapping,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InternalState {
    Unknown,
    Healthy,
    Unhealthy,
}

pub struct ContainerHealthFilter {
    start_time: Instant,
    grace_period: Duration,
    flap_threshold: usize,
    window_duration: Duration,
    success_threshold: usize,
    failure_threshold: usize,

    consecutive_successes: usize,
    consecutive_failures: usize,
    internal_state: InternalState,
    flips: VecDeque<Instant>,
}

impl ContainerHealthFilter {
    pub fn new(
        start_time: Instant,
        grace_period: Duration,
        flap_threshold: usize,
        window_duration: Duration,
        success_threshold: usize,
        failure_threshold: usize,
    ) -> Self {
        Self {
            start_time,
            grace_period,
            flap_threshold,
            window_duration,
            success_threshold,
            failure_threshold,
            consecutive_successes: 0,
            consecutive_failures: 0,
            internal_state: InternalState::Unknown,
            flips: VecDeque::new(),
        }
    }

    pub fn record_probe(&mut self, is_success: bool, now: Instant) -> FilteredStatus {
        if is_success {
            self.consecutive_successes += 1;
            self.consecutive_failures = 0;
        } else {
            self.consecutive_failures += 1;
            self.consecutive_successes = 0;
        }

        let previous_state = self.internal_state;

        if is_success && self.consecutive_successes >= self.success_threshold {
            self.internal_state = InternalState::Healthy;
        } else if !is_success && self.consecutive_failures >= self.failure_threshold {
            self.internal_state = InternalState::Unhealthy;
        }

        if previous_state != InternalState::Unknown && previous_state != self.internal_state {
            self.flips.push_back(now);
        }

        while let Some(&oldest_flip) = self.flips.front() {
            if now.saturating_duration_since(oldest_flip) > self.window_duration {
                self.flips.pop_front();
            } else {
                break;
            }
        }

        let is_in_grace_period = now.saturating_duration_since(self.start_time) <= self.grace_period;

        if self.flips.len() >= self.flap_threshold {
            FilteredStatus::Flapping
        } else {
            match self.internal_state {
                InternalState::Healthy => FilteredStatus::Healthy,
                InternalState::Unhealthy => {
                    if is_in_grace_period {
                        FilteredStatus::Starting
                    } else {
                        FilteredStatus::Unhealthy
                    }
                }
                InternalState::Unknown => FilteredStatus::Starting,
            }
        }
    }
}
