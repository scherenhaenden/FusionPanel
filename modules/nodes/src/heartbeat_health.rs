#![forbid(unsafe_code)]

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeStatus {
    Healthy,
    Degraded,
    Unreachable,
    Quarantined,
}

#[derive(Debug, Clone)]
pub struct NodeHealthConfig {
    pub degraded_missed_heartbeats: u32,
    pub unreachable_missed_heartbeats: u32,
    pub quarantined_missed_heartbeats: u32,
    pub recovery_success_threshold: u32,
}

impl Default for NodeHealthConfig {
    fn default() -> Self {
        Self {
            degraded_missed_heartbeats: 1,
            unreachable_missed_heartbeats: 3,
            quarantined_missed_heartbeats: 10,
            recovery_success_threshold: 3,
        }
    }
}

#[derive(Debug, Clone)]
pub struct NodeHealthTracker {
    config: NodeHealthConfig,
    status: NodeStatus,
    consecutive_missed_heartbeats: u32,
    consecutive_successful_heartbeats: u32,
    last_cpu_load: f64,
    last_ram_utilization: f64,
    total_missed_heartbeats: u64,
}

impl NodeHealthTracker {
    pub fn new(config: NodeHealthConfig) -> Self {
        Self {
            config,
            status: NodeStatus::Healthy,
            consecutive_missed_heartbeats: 0,
            consecutive_successful_heartbeats: 0,
            last_cpu_load: 0.0,
            last_ram_utilization: 0.0,
            total_missed_heartbeats: 0,
        }
    }

    pub fn record_successful_heartbeat(&mut self, cpu_load: f64, ram_utilization: f64) {
        self.last_cpu_load = cpu_load.clamp(0.0, 100.0);
        self.last_ram_utilization = ram_utilization.clamp(0.0, 100.0);
        self.consecutive_missed_heartbeats = 0;
        self.consecutive_successful_heartbeats = self.consecutive_successful_heartbeats.saturating_add(1);

        self.update_status();
    }

    pub fn record_missed_heartbeat(&mut self) {
        self.consecutive_missed_heartbeats = self.consecutive_missed_heartbeats.saturating_add(1);
        self.total_missed_heartbeats = self.total_missed_heartbeats.saturating_add(1);
        self.consecutive_successful_heartbeats = 0;

        self.update_status();
    }

    fn update_status(&mut self) {
        if self.status == NodeStatus::Quarantined {
            return;
        }

        // Evaluate escalation
        if self.consecutive_missed_heartbeats >= self.config.quarantined_missed_heartbeats {
            self.status = NodeStatus::Quarantined;
        } else if self.consecutive_missed_heartbeats >= self.config.unreachable_missed_heartbeats {
            self.status = NodeStatus::Unreachable;
        } else if self.consecutive_missed_heartbeats >= self.config.degraded_missed_heartbeats {
            if self.status == NodeStatus::Healthy {
                self.status = NodeStatus::Degraded;
            }
        }

        // Evaluate recovery
        if self.consecutive_successful_heartbeats >= self.config.recovery_success_threshold {
            self.status = NodeStatus::Healthy;
        }
    }

    pub fn status(&self) -> NodeStatus {
        self.status
    }

    pub fn health_score(&self) -> f64 {
        if self.status == NodeStatus::Quarantined || self.status == NodeStatus::Unreachable {
            return 0.0;
        }

        let mut score = 100.0;

        if self.last_cpu_load > 70.0 {
            score -= (self.last_cpu_load - 70.0) * 0.5;
        }

        if self.last_ram_utilization > 80.0 {
            score -= (self.last_ram_utilization - 80.0) * 1.0;
        }

        let missed_penalty = (self.consecutive_missed_heartbeats as f64) * 10.0;
        let status_penalty = if self.status == NodeStatus::Degraded {
            (self.config.degraded_missed_heartbeats as f64) * 10.0
        } else {
            0.0
        };

        score -= missed_penalty.max(status_penalty);

        score.clamp(0.0, 100.0)
    }

    pub fn manual_quarantine(&mut self) {
        self.status = NodeStatus::Quarantined;
    }
}
