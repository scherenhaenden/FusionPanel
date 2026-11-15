#![forbid(unsafe_code)]

use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DnsHealthMetrics {
    pub udp_latency: Option<Duration>,
    pub tcp_latency: Option<Duration>,
    pub bind9_process_up: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum RouteState {
    #[default]
    Withdraw,
    Up,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnycastProberConfig {
    pub max_udp_latency: Duration,
    pub max_tcp_latency: Duration,
    pub consecutive_success_threshold: u32,
    pub consecutive_failure_threshold: u32,
}

impl Default for AnycastProberConfig {
    fn default() -> Self {
        Self {
            max_udp_latency: Duration::from_millis(100),
            max_tcp_latency: Duration::from_millis(200),
            consecutive_success_threshold: 3,
            consecutive_failure_threshold: 3,
        }
    }
}

pub struct AnycastHealthProber {
    pub config: AnycastProberConfig,
    pub current_state: RouteState,
    pub consecutive_successes: u32,
    pub consecutive_failures: u32,
}

impl AnycastHealthProber {
    pub fn new(config: AnycastProberConfig) -> Self {
        Self {
            config,
            current_state: RouteState::Withdraw,
            consecutive_successes: 0,
            consecutive_failures: 0,
        }
    }

    pub fn evaluate_metrics(&mut self, metrics: &DnsHealthMetrics) -> RouteState {
        let is_healthy = self.is_healthy(metrics);

        if is_healthy {
            self.consecutive_successes = self.consecutive_successes.saturating_add(1);
            self.consecutive_failures = 0;

            if self.consecutive_successes >= self.config.consecutive_success_threshold {
                self.current_state = RouteState::Up;
            }
        } else {
            self.consecutive_failures = self.consecutive_failures.saturating_add(1);
            self.consecutive_successes = 0;

            if self.consecutive_failures >= self.config.consecutive_failure_threshold {
                self.current_state = RouteState::Withdraw;
            }
        }

        self.current_state.clone()
    }

    fn is_healthy(&self, metrics: &DnsHealthMetrics) -> bool {
        if !metrics.bind9_process_up {
            return false;
        }

        let mut dns_ok = false;

        if let Some(udp_lat) = metrics.udp_latency {
            if udp_lat <= self.config.max_udp_latency {
                dns_ok = true;
            }
        }

        if !dns_ok {
            if let Some(tcp_lat) = metrics.tcp_latency {
                if tcp_lat <= self.config.max_tcp_latency {
                    dns_ok = true;
                }
            }
        }

        dns_ok
    }
}
