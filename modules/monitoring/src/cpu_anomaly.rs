#![forbid(unsafe_code)]

#[derive(Debug, Default, Clone)]
pub struct CpuStats {
    pub user: u64,
    pub nice: u64,
    pub system: u64,
    pub idle: u64,
    pub iowait: u64,
    pub irq: u64,
    pub softirq: u64,
    pub steal: u64,
}

impl CpuStats {
    pub fn total(&self) -> u64 {
        self.user + self.nice + self.system + self.idle + self.iowait + self.irq + self.softirq + self.steal
    }
}

#[derive(Debug, Default, Clone)]
pub struct LoadAvg {
    pub one_min: f64,
    pub five_min: f64,
    pub fifteen_min: f64,
}

#[derive(Debug, PartialEq, Clone)]
pub enum Anomaly {
    HighIoWait(f64),
    HighStealTime(f64),
    RunawaySpike(f64),
}

pub struct CpuAnomalyDetector {
    prev_stats: Option<CpuStats>,
    logical_core_count: usize,
}

impl CpuAnomalyDetector {
    pub fn new(logical_core_count: usize) -> Self {
        Self {
            prev_stats: None,
            logical_core_count: logical_core_count.max(1),
        }
    }

    pub fn parse_stat(stat_str: &str) -> Option<CpuStats> {
        for line in stat_str.lines() {
            if line.starts_with("cpu ") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 9 {
                    return Some(CpuStats {
                        user: parts[1].parse().unwrap_or(0),
                        nice: parts[2].parse().unwrap_or(0),
                        system: parts[3].parse().unwrap_or(0),
                        idle: parts[4].parse().unwrap_or(0),
                        iowait: parts[5].parse().unwrap_or(0),
                        irq: parts[6].parse().unwrap_or(0),
                        softirq: parts[7].parse().unwrap_or(0),
                        steal: parts[8].parse().unwrap_or(0),
                    });
                }
            }
        }
        None
    }

    pub fn parse_loadavg(loadavg_str: &str) -> Option<LoadAvg> {
        let parts: Vec<&str> = loadavg_str.split_whitespace().collect();
        if parts.len() >= 3 {
            return Some(LoadAvg {
                one_min: parts[0].parse().unwrap_or(0.0),
                five_min: parts[1].parse().unwrap_or(0.0),
                fifteen_min: parts[2].parse().unwrap_or(0.0),
            });
        }
        None
    }

    pub fn analyze(&mut self, stat_str: &str, loadavg_str: &str) -> Vec<Anomaly> {
        let mut anomalies = Vec::new();

        let current_stats = Self::parse_stat(stat_str);
        let load_avg = Self::parse_loadavg(loadavg_str);

        if let (Some(curr), Some(prev)) = (&current_stats, &self.prev_stats) {
            let total_delta = curr.total().saturating_sub(prev.total()) as f64;
            if total_delta > 0.0 {
                let iowait_delta = curr.iowait.saturating_sub(prev.iowait) as f64;
                let steal_delta = curr.steal.saturating_sub(prev.steal) as f64;

                let iowait_percent = (iowait_delta / total_delta) * 100.0;
                let steal_percent = (steal_delta / total_delta) * 100.0;

                if iowait_percent > 30.0 {
                    anomalies.push(Anomaly::HighIoWait(iowait_percent));
                }

                if steal_percent > 15.0 {
                    anomalies.push(Anomaly::HighStealTime(steal_percent));
                }
            }
        }

        if let Some(stats) = current_stats {
            self.prev_stats = Some(stats);
        }

        if let Some(load) = load_avg {
            let saturation = load.one_min / (self.logical_core_count as f64);
            if saturation > 1.5 && load.one_min > load.five_min {
                anomalies.push(Anomaly::RunawaySpike(saturation));
            }
        }

        anomalies
    }
}
