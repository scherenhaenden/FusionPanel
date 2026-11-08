#![forbid(unsafe_code)]

use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RestartPolicy {
    NoRestart,
    Always,
    OnFailure {
        max_retries: u32,
        base_delay: Duration,
        max_delay: Duration,
    },
}

#[derive(Debug)]
pub struct ContainerHealthSupervisor {
    policy: RestartPolicy,
    pub consecutive_failures: u32,
    pub restart_attempts: u32,
    is_healthy: bool,
}

pub enum SupervisorAction {
    None,
    Restart { delay: Duration },
}

impl ContainerHealthSupervisor {
    pub fn new(policy: RestartPolicy) -> Self {
        Self {
            policy,
            consecutive_failures: 0,
            restart_attempts: 0,
            is_healthy: true,
        }
    }

    pub fn transition(&mut self, is_healthy: bool) -> SupervisorAction {
        self.is_healthy = is_healthy;

        if is_healthy {
            self.consecutive_failures = 0;
            self.restart_attempts = 0;
            return SupervisorAction::None;
        }

        self.consecutive_failures += 1;

        match &self.policy {
            RestartPolicy::NoRestart => SupervisorAction::None,
            RestartPolicy::Always => {
                self.restart_attempts += 1;
                SupervisorAction::Restart {
                    delay: Duration::from_secs(0),
                }
            }
            RestartPolicy::OnFailure {
                max_retries,
                base_delay,
                max_delay,
            } => {
                if self.restart_attempts >= *max_retries {
                    return SupervisorAction::None;
                }

                let attempt = self.restart_attempts;
                self.restart_attempts += 1;

                let multiplier = 1u32.checked_shl(attempt).unwrap_or(u32::MAX);
                let delay = base_delay.checked_mul(multiplier).unwrap_or(*max_delay);
                let delay = std::cmp::min(delay, *max_delay);

                SupervisorAction::Restart { delay }
            }
        }
    }
}
