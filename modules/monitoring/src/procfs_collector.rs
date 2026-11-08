#![forbid(unsafe_code)]

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct CpuStat {
    pub user: u64,
    pub nice: u64,
    pub system: u64,
    pub idle: u64,
    pub iowait: u64,
    pub irq: u64,
    pub softirq: u64,
    pub steal: u64,
}

impl CpuStat {
    pub fn total(&self) -> u64 {
        self.user + self.nice + self.system + self.idle + self.iowait + self.irq + self.softirq + self.steal
    }

    pub fn idle_total(&self) -> u64 {
        self.idle + self.iowait
    }

    pub fn parse(content: &str) -> Option<Self> {
        for line in content.lines() {
            if line.starts_with("cpu ") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 9 {
                    return Some(CpuStat {
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
}

pub fn compute_cpu_utilization(first: &CpuStat, second: &CpuStat) -> f64 {
    let total_d = second.total().saturating_sub(first.total()) as f64;
    let idle_d = second.idle_total().saturating_sub(first.idle_total()) as f64;

    if total_d == 0.0 {
        return 0.0;
    }

    ((total_d - idle_d) / total_d) * 100.0
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct MemInfo {
    pub mem_total: u64,
    pub mem_free: u64,
    pub mem_available: u64,
    pub buffers: u64,
    pub cached: u64,
    pub swap_total: u64,
    pub swap_free: u64,
}

impl MemInfo {
    pub fn parse(content: &str) -> Self {
        let mut info = MemInfo::default();
        for line in content.lines() {
            let mut parts = line.split_whitespace();
            if let Some(key) = parts.next() {
                if let Some(value) = parts.next() {
                    if let Ok(val) = value.parse::<u64>() {
                        match key {
                            "MemTotal:" => info.mem_total = val,
                            "MemFree:" => info.mem_free = val,
                            "MemAvailable:" => info.mem_available = val,
                            "Buffers:" => info.buffers = val,
                            "Cached:" => info.cached = val,
                            "SwapTotal:" => info.swap_total = val,
                            "SwapFree:" => info.swap_free = val,
                            _ => {}
                        }
                    }
                }
            }
        }
        info
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct DiskStats {
    pub major: u32,
    pub minor: u32,
    pub device_name: String,
    pub reads_completed: u64,
    pub sectors_read: u64,
    pub writes_completed: u64,
    pub sectors_written: u64,
    pub io_time_ms: u64,
}

impl DiskStats {
    pub fn parse_all(content: &str) -> Vec<Self> {
        let mut stats = Vec::new();
        for line in content.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 14 {
                if let (
                    Ok(major),
                    Ok(minor),
                    Ok(reads_completed),
                    Ok(sectors_read),
                    Ok(writes_completed),
                    Ok(sectors_written),
                    Ok(io_time_ms)
                ) = (
                    parts[0].parse(),
                    parts[1].parse(),
                    parts[3].parse(),
                    parts[5].parse(),
                    parts[7].parse(),
                    parts[9].parse(),
                    parts[12].parse(),
                ) {
                    stats.push(DiskStats {
                        major,
                        minor,
                        device_name: parts[2].to_string(),
                        reads_completed,
                        sectors_read,
                        writes_completed,
                        sectors_written,
                        io_time_ms,
                    });
                }
            }
        }
        stats
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct NetDevStats {
    pub interface_name: String,
    pub rx_bytes: u64,
    pub rx_packets: u64,
    pub rx_errs: u64,
    pub tx_bytes: u64,
    pub tx_packets: u64,
    pub tx_errs: u64,
}

impl NetDevStats {
    pub fn parse_all(content: &str) -> Vec<Self> {
        let mut stats = Vec::new();
        for line in content.lines().skip(2) {
            if let Some(colon_idx) = line.find(':') {
                let iface_name = line[..colon_idx].trim().to_string();
                let rest = &line[colon_idx + 1..];
                let parts: Vec<&str> = rest.split_whitespace().collect();
                
                if parts.len() >= 16 {
                    if let (
                        Ok(rx_bytes),
                        Ok(rx_packets),
                        Ok(rx_errs),
                        Ok(tx_bytes),
                        Ok(tx_packets),
                        Ok(tx_errs)
                    ) = (
                        parts[0].parse(),
                        parts[1].parse(),
                        parts[2].parse(),
                        parts[8].parse(),
                        parts[9].parse(),
                        parts[10].parse(),
                    ) {
                        stats.push(NetDevStats {
                            interface_name: iface_name,
                            rx_bytes,
                            rx_packets,
                            rx_errs,
                            tx_bytes,
                            tx_packets,
                            tx_errs,
                        });
                    }
                }
            }
        }
        stats
    }
}
