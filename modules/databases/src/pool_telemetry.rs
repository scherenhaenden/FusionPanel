#![forbid(unsafe_code)]

use std::collections::VecDeque;
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PoolStats {
    pub active_connections: usize,
    pub idle_connections: usize,
    pub waiting_connections: usize,
    pub max_connections: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LatencyPercentiles {
    pub p50: Duration,
    pub p95: Duration,
    pub p99: Duration,
}

pub struct PoolTelemetry {
    pub stats: PoolStats,
    latencies: VecDeque<Duration>,
    max_history: usize,
}

impl PoolTelemetry {
    pub fn new(max_connections: usize, max_history: usize) -> Self {
        Self {
            stats: PoolStats {
                active_connections: 0,
                idle_connections: 0,
                waiting_connections: 0,
                max_connections,
            },
            latencies: VecDeque::with_capacity(max_history),
            max_history,
        }
    }

    pub fn update_stats(&mut self, active: usize, idle: usize, waiting: usize) {
        self.stats.active_connections = active;
        self.stats.idle_connections = idle;
        self.stats.waiting_connections = waiting;
    }

    pub fn record_latency(&mut self, latency: Duration) {
        if self.max_history == 0 {
            return;
        }
        if self.latencies.len() >= self.max_history {
            self.latencies.pop_front();
        }
        self.latencies.push_back(latency);
    }

    pub fn calculate_percentiles(&self) -> Option<LatencyPercentiles> {
        if self.latencies.is_empty() {
            return None;
        }

        let mut sorted: Vec<Duration> = self.latencies.iter().copied().collect();
        sorted.sort_unstable();

        let len = sorted.len();

        let rank = |p: f64| -> Duration {
            let float_idx = p * (len - 1) as f64;
            let lower = float_idx.floor() as usize;
            let upper = float_idx.ceil() as usize;

            if lower == upper {
                sorted[lower]
            } else {
                let weight = float_idx - lower as f64;
                let l_val = sorted[lower].as_nanos() as f64;
                let u_val = sorted[upper].as_nanos() as f64;
                let interpolated = l_val + (u_val - l_val) * weight;
                Duration::from_nanos(interpolated as u64)
            }
        };

        Some(LatencyPercentiles {
            p50: rank(0.50),
            p95: rank(0.95),
            p99: rank(0.99),
        })
    }

    pub fn check_pool_exhaustion(&self) -> Option<String> {
        if self.stats.active_connections >= self.stats.max_connections
            && self.stats.max_connections > 0
        {
            Some(format!(
                "WARNING: Database connection pool exhausted. Active connections ({}) reached or exceeded max ({}).",
                self.stats.active_connections, self.stats.max_connections
            ))
        } else if self.stats.waiting_connections > 0 {
            Some(format!(
                "WARNING: Database connection pool under pressure. {} queries waiting for a connection.",
                self.stats.waiting_connections
            ))
        } else {
            None
        }
    }
}
