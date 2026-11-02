use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthState {
    Healthy,
    Unhealthy,
    Unknown,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HealthProbePolicy {
    pub interval: Duration,
    pub retries: u32,
    pub timeout: Duration,
}

#[derive(Debug)]
pub struct HealthChecker {
    pub policy: HealthProbePolicy,
    pub state: HealthState,
    consecutive_failures: u32,
}

impl HealthChecker {
    pub fn new(policy: HealthProbePolicy) -> Self {
        Self {
            policy,
            state: HealthState::Unknown,
            consecutive_failures: 0,
        }
    }

    pub fn transition(&mut self, is_healthy: bool) {
        if is_healthy {
            self.state = HealthState::Healthy;
            self.consecutive_failures = 0;
        } else {
            self.consecutive_failures += 1;
            if self.consecutive_failures >= self.policy.retries {
                self.state = HealthState::Unhealthy;
            }
        }
    }

    pub fn get_state(&self) -> HealthState {
        self.state
    }
}
