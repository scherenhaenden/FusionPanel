#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProbeTarget {
    Http {
        url: String,
        expected_status: u16,
        expected_substring: Option<String>,
    },
    Tcp {
        host: String,
        port: u16,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProbeConfig {
    pub timeout_millis: u64,
    pub max_retries: u32,
    pub interval_secs: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProbeOutcome {
    pub success: bool,
    pub latency_ms: u64,
    pub error_message: Option<String>,
    pub timestamp: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AlertState {
    Normal,
    Alerting,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyntheticProberConfig {
    pub target: ProbeTarget,
    pub config: ProbeConfig,
    pub failure_threshold: u32,
    pub latency_threshold_ms: u64,
}

#[derive(Debug)]
pub struct SyntheticProber {
    pub target: ProbeTarget,
    pub config: ProbeConfig,
    pub failure_threshold: u32,
    pub latency_threshold_ms: u64,
    consecutive_failures: u32,
    is_alerting: bool,
}

impl SyntheticProber {
    pub fn new(target: ProbeTarget, config: ProbeConfig, failure_threshold: u32, latency_threshold_ms: u64) -> Result<Self, String> {
        match &target {
            ProbeTarget::Http { url, .. } => {
                if !url.starts_with("http://") && !url.starts_with("https://") {
                    return Err("Invalid URL scheme, must be http:// or https://".to_string());
                }
            }
            ProbeTarget::Tcp { port, .. } => {
                if *port == 0 {
                    return Err("Invalid TCP port, cannot be 0".to_string());
                }
            }
        }

        if failure_threshold == 0 {
            return Err("failure_threshold must be greater than 0".to_string());
        }

        Ok(Self {
            target,
            config,
            failure_threshold,
            latency_threshold_ms,
            consecutive_failures: 0,
            is_alerting: false,
        })
    }

    pub fn from_config(config: SyntheticProberConfig) -> Result<Self, String> {
        Self::new(config.target, config.config, config.failure_threshold, config.latency_threshold_ms)
    }

    pub fn evaluate_outcome(&mut self, outcome: &ProbeOutcome) -> AlertState {
        let is_failure = !outcome.success || outcome.latency_ms > self.latency_threshold_ms;

        if is_failure {
            self.consecutive_failures += 1;
        } else {
            // A single success clears the consecutive failures and alert state
            self.consecutive_failures = 0;
            self.is_alerting = false;
        }

        if self.consecutive_failures >= self.failure_threshold {
            self.is_alerting = true;
        }

        if self.is_alerting {
            AlertState::Alerting
        } else {
            AlertState::Normal
        }
    }

    pub fn consecutive_failures(&self) -> u32 {
        self.consecutive_failures
    }
}
