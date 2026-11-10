#![forbid(unsafe_code)]

use std::fs;
use std::path::Path;

#[derive(Debug, Default, Clone, PartialEq)]
pub struct SocketStats {
    pub established: u32,
    pub time_wait: u32,
    pub close_wait: u32,
    pub orphan: u32,
    pub alloc: u32,
}

#[derive(Debug, PartialEq, Clone)]
pub enum SocketAnomaly {
    ExhaustionRisk(f64),
    HighChurn(f64),
}

pub struct SocketMonitor {
    prev_stats: Option<SocketStats>,
    max_sockets: u32,
}

impl SocketMonitor {
    pub fn new(max_sockets: u32) -> Self {
        Self {
            prev_stats: None,
            max_sockets: max_sockets.max(1),
        }
    }

    pub fn parse_sockstat(content: &str) -> Option<SocketStats> {
        let mut stats = SocketStats::default();
        let mut found = false;

        for line in content.lines() {
            if line.starts_with("TCP:") {
                found = true;
                let parts: Vec<&str> = line.split_whitespace().collect();
                for i in 1..parts.len() {
                    if i + 1 < parts.len() {
                        let val = parts[i + 1].parse::<u32>().unwrap_or(0);
                        match parts[i] {
                            "orphan" => stats.orphan = val,
                            "tw" => stats.time_wait = val,
                            "alloc" => stats.alloc = val,
                            _ => {}
                        }
                    }
                }
            }
        }

        if found {
            Some(stats)
        } else {
            None
        }
    }

    pub fn parse_tcp(content: &str, stats: &mut SocketStats) {
        for line in content.lines().skip(1) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() > 3 {
                // st is the 4th column (index 3)
                let st = parts[3];
                match st {
                    "01" => stats.established += 1,
                    "08" => stats.close_wait += 1,
                    // Time-wait might be in /proc/net/tcp, but we got it from sockstat
                    // Orphan is tracked in sockstat
                    _ => {}
                }
            }
        }
    }

    pub fn analyze(&mut self, sockstat_path: &Path, tcp_path: &Path) -> Result<Vec<SocketAnomaly>, String> {
        let sockstat_content = fs::read_to_string(sockstat_path)
            .map_err(|e| format!("Failed to read sockstat: {}", e))?;
        let tcp_content = fs::read_to_string(tcp_path)
            .map_err(|e| format!("Failed to read tcp: {}", e))?;

        let mut current_stats = Self::parse_sockstat(&sockstat_content)
            .ok_or_else(|| "Failed to parse sockstat".to_string())?;

        Self::parse_tcp(&tcp_content, &mut current_stats);

        let mut anomalies = Vec::new();

        // Check exhaustion
        let usage_ratio = current_stats.alloc as f64 / self.max_sockets as f64;
        if usage_ratio > 0.8 {
            anomalies.push(SocketAnomaly::ExhaustionRisk(usage_ratio));
        }

        // Check churn velocity
        if let Some(prev) = &self.prev_stats {
            let churn = current_stats.time_wait as f64 - prev.time_wait as f64;
            if churn > 1000.0 {
                anomalies.push(SocketAnomaly::HighChurn(churn));
            }
        }

        self.prev_stats = Some(current_stats);

        Ok(anomalies)
    }
}
