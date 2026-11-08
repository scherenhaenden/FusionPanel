#![forbid(unsafe_code)]

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BalancingStrategy {
    RoundRobin,
    LeastConnections,
    IpHash,
}

#[derive(Debug, Clone)]
pub struct UpstreamBackend {
    pub addr: String,
    pub weight: u32,
    pub is_healthy: bool,
    pub active_connections: usize,
}

impl UpstreamBackend {
    pub fn new(addr: &str, weight: u32) -> Self {
        Self {
            addr: addr.to_string(),
            weight,
            is_healthy: true,
            active_connections: 0,
        }
    }
}

pub struct UpstreamLoadBalancer {
    pub backends: Vec<UpstreamBackend>,
    pub strategy: BalancingStrategy,
    round_robin_index: usize,
}

impl UpstreamLoadBalancer {
    pub fn new(backends: Vec<UpstreamBackend>, strategy: BalancingStrategy) -> Self {
        Self {
            backends,
            strategy,
            round_robin_index: 0,
        }
    }

    pub fn dispatch(&mut self, client_ip: Option<&str>) -> Option<String> {
        let healthy_indices: Vec<usize> = self.backends.iter()
            .enumerate()
            .filter(|(_, b)| b.is_healthy)
            .map(|(i, _)| i)
            .collect();

        if healthy_indices.is_empty() {
            return None;
        }

        let selected_index = match self.strategy {
            BalancingStrategy::RoundRobin => {
                let idx = healthy_indices[self.round_robin_index % healthy_indices.len()];
                self.round_robin_index = self.round_robin_index.wrapping_add(1);
                idx
            }
            BalancingStrategy::LeastConnections => {
                let mut min_conn = usize::MAX;
                let mut selected = healthy_indices[0];

                for &idx in &healthy_indices {
                    if self.backends[idx].active_connections < min_conn {
                        min_conn = self.backends[idx].active_connections;
                        selected = idx;
                    }
                }
                selected
            }
            BalancingStrategy::IpHash => {
                if let Some(ip) = client_ip {
                    let mut hasher = DefaultHasher::new();
                    ip.hash(&mut hasher);
                    let hash = hasher.finish() as usize;
                    healthy_indices[hash % healthy_indices.len()]
                } else {
                    let idx = healthy_indices[self.round_robin_index % healthy_indices.len()];
                    self.round_robin_index = self.round_robin_index.wrapping_add(1);
                    idx
                }
            }
        };

        self.backends[selected_index].active_connections += 1;
        Some(self.backends[selected_index].addr.clone())
    }

    pub fn release(&mut self, addr: &str) {
        if let Some(backend) = self.backends.iter_mut().find(|b| b.addr == addr) {
            backend.active_connections = backend.active_connections.saturating_sub(1);
        }
    }
}
