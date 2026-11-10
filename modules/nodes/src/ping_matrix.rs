#![forbid(unsafe_code)]

use std::collections::{HashMap, HashSet, VecDeque};
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct RttStats {
    pub samples: VecDeque<Duration>,
    pub max_samples: usize,
    pub average_rtt: Duration,
}

impl RttStats {
    pub fn new(max_samples: usize) -> Self {
        Self {
            samples: VecDeque::with_capacity(max_samples),
            max_samples: max_samples.max(1),
            average_rtt: Duration::ZERO,
        }
    }

    pub fn add_sample(&mut self, rtt: Duration) {
        if self.samples.len() == self.max_samples {
            self.samples.pop_front();
        }
        self.samples.push_back(rtt);

        let sum: Duration = self.samples.iter().sum();
        self.average_rtt = sum / (self.samples.len() as u32);
    }
}

#[derive(Debug, Clone)]
pub struct PingMatrix {
    nodes: HashSet<String>,
    matrix: HashMap<String, HashMap<String, RttStats>>,
    max_samples: usize,
}

impl Default for PingMatrix {
    fn default() -> Self {
        Self::new(10)
    }
}

impl PingMatrix {
    pub fn new(max_samples: usize) -> Self {
        Self {
            nodes: HashSet::new(),
            matrix: HashMap::new(),
            max_samples,
        }
    }

    pub fn add_node(&mut self, node_id: &str) {
        self.nodes.insert(node_id.to_string());
    }

    pub fn remove_node(&mut self, node_id: &str) {
        self.nodes.remove(node_id);
        self.matrix.remove(node_id);
        for peers in self.matrix.values_mut() {
            peers.remove(node_id);
        }
    }

    pub fn update(&mut self, source: &str, destination: &str, rtt: Duration) {
        self.nodes.insert(source.to_string());
        self.nodes.insert(destination.to_string());

        self.matrix
            .entry(source.to_string())
            .or_default()
            .entry(destination.to_string())
            .or_insert_with(|| RttStats::new(self.max_samples))
            .add_sample(rtt);
    }

    pub fn remove_edge(&mut self, source: &str, destination: &str) {
        if let Some(peers) = self.matrix.get_mut(source) {
            peers.remove(destination);
        }
    }

    pub fn get_average_rtt(&self, source: &str, destination: &str) -> Option<Duration> {
        self.matrix
            .get(source)
            .and_then(|peers| peers.get(destination))
            .map(|stats| stats.average_rtt)
    }

    /// Identify peers that have an average RTT above the given threshold.
    pub fn high_latency_peers(&self, threshold: Duration) -> Vec<(String, String)> {
        let mut high_latency = Vec::new();
        for (src, peers) in &self.matrix {
            for (dest, stats) in peers {
                if stats.average_rtt > threshold {
                    high_latency.push((src.clone(), dest.clone()));
                }
            }
        }
        high_latency.sort();
        high_latency
    }

    /// Identify isolated peers.
    /// A peer is considered isolated if it has no recorded RTT to or from any other node.
    pub fn isolated_peers(&self) -> Vec<String> {
        let mut isolated = Vec::new();
        for node in &self.nodes {
            let mut has_edge = false;

            // Check outgoing edges
            if let Some(peers) = self.matrix.get(node) {
                if peers.keys().any(|dest| dest != node) {
                    has_edge = true;
                }
            }

            // Check incoming edges
            if !has_edge {
                for (src, peers) in &self.matrix {
                    if src != node && peers.contains_key(node) {
                        has_edge = true;
                        break;
                    }
                }
            }

            if !has_edge {
                isolated.push(node.clone());
            }
        }
        isolated.sort();
        isolated
    }
}
