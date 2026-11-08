#![forbid(unsafe_code)]

use std::time::Duration;
use std::fs;
use std::io;

#[derive(Debug, Clone, PartialEq, Default)]
pub struct DiskStatsEntry {
    pub device_name: String,
    pub reads_completed: u64,
    pub sectors_read: u64,
    pub read_time_ms: u64,
    pub writes_completed: u64,
    pub sectors_written: u64,
    pub write_time_ms: u64,
    pub io_time_ms: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DiskSaturationMetrics {
    pub device_name: String,
    pub utilization_pct: f64,
    pub avg_latency_ms: f64,
    pub is_saturated: bool,
}

pub struct DiskSaturationMonitor {
    pub threshold_pct: f64,
}

impl DiskSaturationMonitor {
    pub fn new(threshold_pct: f64) -> Self {
        Self { threshold_pct }
    }

    pub fn parse_diskstats(contents: &str) -> Vec<DiskStatsEntry> {
        let mut entries = Vec::new();
        for line in contents.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            // Expected format (at least 14 fields for full disk stats, field 3 is device name)
            // 1: major, 2: minor, 3: device name
            // 4: reads completed successfully
            // 5: reads merged
            // 6: sectors read
            // 7: time spent reading (ms)
            // 8: writes completed
            // 9: writes merged
            // 10: sectors written
            // 11: time spent writing (ms)
            // 12: I/Os currently in progress
            // 13: time spent doing I/Os (ms)
            // 14: weighted time spent doing I/Os (ms)

            if parts.len() >= 14 {
                let device_name = parts[2].to_string();
                if device_name.starts_with("loop") || device_name.starts_with("ram") {
                    continue;
                }

                let parse_field = |index: usize| -> u64 {
                    parts.get(index).and_then(|s| s.parse::<u64>().ok()).unwrap_or(0)
                };

                let entry = DiskStatsEntry {
                    device_name,
                    reads_completed: parse_field(3),
                    sectors_read: parse_field(5),
                    read_time_ms: parse_field(6),
                    writes_completed: parse_field(7),
                    sectors_written: parse_field(9),
                    write_time_ms: parse_field(10),
                    io_time_ms: parse_field(12),
                };
                entries.push(entry);
            }
        }
        entries
    }

    pub fn read_system_diskstats() -> io::Result<Vec<DiskStatsEntry>> {
        let contents = fs::read_to_string("/proc/diskstats")?;
        Ok(Self::parse_diskstats(&contents))
    }

    pub fn evaluate_metrics(
        &self,
        prev: &DiskStatsEntry,
        curr: &DiskStatsEntry,
        elapsed: Duration,
    ) -> DiskSaturationMetrics {
        let elapsed_ms = elapsed.as_millis();

        let mut utilization_pct = 0.0;
        if elapsed_ms > 0 {
            let delta_io_time_ms = curr.io_time_ms.saturating_sub(prev.io_time_ms);
            utilization_pct = (delta_io_time_ms as f64 / elapsed_ms as f64) * 100.0;
            // Clamp 0.0..=100.0
            if utilization_pct > 100.0 {
                utilization_pct = 100.0;
            } else if utilization_pct < 0.0 {
                utilization_pct = 0.0;
            }
        }

        let delta_reads = curr.reads_completed.saturating_sub(prev.reads_completed);
        let delta_writes = curr.writes_completed.saturating_sub(prev.writes_completed);
        let total_ops = delta_reads + delta_writes;

        let delta_read_time = curr.read_time_ms.saturating_sub(prev.read_time_ms);
        let delta_write_time = curr.write_time_ms.saturating_sub(prev.write_time_ms);

        let avg_latency_ms = if total_ops > 0 {
            (delta_read_time + delta_write_time) as f64 / total_ops as f64
        } else {
            0.0
        };

        let is_saturated = utilization_pct >= self.threshold_pct;

        DiskSaturationMetrics {
            device_name: curr.device_name.clone(),
            utilization_pct,
            avg_latency_ms,
            is_saturated,
        }
    }
}
