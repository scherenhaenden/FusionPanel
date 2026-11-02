#![forbid(unsafe_code)]

use std::fs;
use std::io::{self, BufRead};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq)]
pub struct SystemMetrics {
    pub cpu_usage_percent: f64,
    pub memory_used_kb: u64,
    pub memory_available_kb: u64,
    pub disk_io_ops: u64,
}

#[derive(Debug, Clone)]
pub struct MetricsCollector {
    proc_stat_path: PathBuf,
    proc_meminfo_path: PathBuf,
    proc_diskstats_path: PathBuf,
    cgroup_v1_mem_path: PathBuf,
    cgroup_v2_mem_path: PathBuf,
    last_cpu_idle: Option<u64>,
    last_cpu_total: Option<u64>,
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new(
            Path::new("/proc/stat"),
            Path::new("/proc/meminfo"),
            Path::new("/proc/diskstats"),
            Path::new("/sys/fs/cgroup/memory"),
            Path::new("/sys/fs/cgroup"),
        )
    }
}

impl MetricsCollector {
    pub fn new(stat: &Path, meminfo: &Path, diskstats: &Path, cg_v1: &Path, cg_v2: &Path) -> Self {
        Self {
            proc_stat_path: stat.to_path_buf(),
            proc_meminfo_path: meminfo.to_path_buf(),
            proc_diskstats_path: diskstats.to_path_buf(),
            cgroup_v1_mem_path: cg_v1.to_path_buf(),
            cgroup_v2_mem_path: cg_v2.to_path_buf(),
            last_cpu_idle: None,
            last_cpu_total: None,
        }
    }

    pub fn collect(&mut self) -> io::Result<SystemMetrics> {
        let (idle_time, total_time) = self.read_cpu_stats()?;

        let cpu_usage_percent = if let (Some(last_idle), Some(last_total)) =
            (self.last_cpu_idle, self.last_cpu_total)
        {
            let idle_diff = idle_time.saturating_sub(last_idle);
            let total_diff = total_time.saturating_sub(last_total);

            if total_diff > 0 {
                let usage = 1.0 - (idle_diff as f64 / total_diff as f64);
                usage * 100.0
            } else {
                0.0
            }
        } else {
            0.0
        };

        self.last_cpu_idle = Some(idle_time);
        self.last_cpu_total = Some(total_time);

        let (mut mem_used, mut mem_avail) = self.read_memory_stats()?;

        // Handle cgroups gracefully
        let (cg_used, cg_avail) = self.read_cgroup_memory_stats();
        if let Some(cg_u) = cg_used {
            mem_used = cg_u;
        }
        if let Some(cg_a) = cg_avail {
            mem_avail = cg_a;
        }

        let disk_io = self.read_disk_stats()?;

        Ok(SystemMetrics {
            cpu_usage_percent,
            memory_used_kb: mem_used,
            memory_available_kb: mem_avail,
            disk_io_ops: disk_io,
        })
    }

    fn read_cpu_stats(&self) -> io::Result<(u64, u64)> {
        let file = match fs::File::open(&self.proc_stat_path) {
            Ok(f) => f,
            Err(_) => return Ok((0, 0)),
        };
        let reader = io::BufReader::new(file);

        for line in reader.lines() {
            let line = line?;
            if line.starts_with("cpu ") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 5 {
                    let user: u64 = parts[1].parse().unwrap_or(0);
                    let nice: u64 = parts[2].parse().unwrap_or(0);
                    let system: u64 = parts[3].parse().unwrap_or(0);
                    let idle: u64 = parts[4].parse().unwrap_or(0);
                    let iowait: u64 = parts.get(5).and_then(|v| v.parse().ok()).unwrap_or(0);
                    let irq: u64 = parts.get(6).and_then(|v| v.parse().ok()).unwrap_or(0);
                    let softirq: u64 = parts.get(7).and_then(|v| v.parse().ok()).unwrap_or(0);
                    let steal: u64 = parts.get(8).and_then(|v| v.parse().ok()).unwrap_or(0);

                    let idle_time = idle + iowait;
                    let non_idle_time = user + nice + system + irq + softirq + steal;
                    let total_time = idle_time + non_idle_time;

                    return Ok((idle_time, total_time));
                }
            }
        }
        Ok((0, 0))
    }

    fn read_memory_stats(&self) -> io::Result<(u64, u64)> {
        let file = match fs::File::open(&self.proc_meminfo_path) {
            Ok(f) => f,
            Err(_) => return Ok((0, 0)),
        };
        let reader = io::BufReader::new(file);

        let mut mem_total = 0;
        let mut mem_free = 0;
        let mut mem_available = 0;
        let mut buffers = 0;
        let mut cached = 0;

        for line in reader.lines() {
            let line = line?;
            if line.starts_with("MemTotal:") {
                mem_total = parse_kb(&line);
            } else if line.starts_with("MemFree:") {
                mem_free = parse_kb(&line);
            } else if line.starts_with("MemAvailable:") {
                mem_available = parse_kb(&line);
            } else if line.starts_with("Buffers:") {
                buffers = parse_kb(&line);
            } else if line.starts_with("Cached:") {
                cached = parse_kb(&line);
            }
        }

        if mem_available == 0 {
            mem_available = mem_free + buffers + cached;
        }

        let mem_used = mem_total.saturating_sub(mem_available);

        Ok((mem_used, mem_available))
    }

    fn read_cgroup_memory_stats(&self) -> (Option<u64>, Option<u64>) {
        // Try v2 first
        let v2_current = self.cgroup_v2_mem_path.join("memory.current");
        let v2_max = self.cgroup_v2_mem_path.join("memory.max");

        if v2_current.exists() {
            if let Ok(current_str) = fs::read_to_string(&v2_current) {
                if let Ok(current_bytes) = current_str.trim().parse::<u64>() {
                    let used_kb = current_bytes / 1024;

                    let avail_kb = if let Ok(max_str) = fs::read_to_string(&v2_max) {
                        if max_str.trim() == "max" {
                            None
                        } else if let Ok(max_bytes) = max_str.trim().parse::<u64>() {
                            Some((max_bytes / 1024).saturating_sub(used_kb))
                        } else {
                            None
                        }
                    } else {
                        None
                    };
                    return (Some(used_kb), avail_kb);
                }
            }
        }

        // Try v1 next
        let v1_usage = self.cgroup_v1_mem_path.join("memory.usage_in_bytes");
        let v1_limit = self.cgroup_v1_mem_path.join("memory.limit_in_bytes");

        if v1_usage.exists() {
            if let Ok(usage_str) = fs::read_to_string(&v1_usage) {
                if let Ok(usage_bytes) = usage_str.trim().parse::<u64>() {
                    let used_kb = usage_bytes / 1024;

                    let avail_kb = if let Ok(limit_str) = fs::read_to_string(&v1_limit) {
                        if let Ok(limit_bytes) = limit_str.trim().parse::<u64>() {
                            // Check for max limits common in v1
                            if limit_bytes > 9007199254740991 {
                                None
                            } else {
                                Some((limit_bytes / 1024).saturating_sub(used_kb))
                            }
                        } else {
                            None
                        }
                    } else {
                        None
                    };
                    return (Some(used_kb), avail_kb);
                }
            }
        }

        (None, None)
    }

    fn read_disk_stats(&self) -> io::Result<u64> {
        let file = match fs::File::open(&self.proc_diskstats_path) {
            Ok(f) => f,
            Err(_) => return Ok(0),
        };
        let reader = io::BufReader::new(file);

        let mut total_ops = 0;

        for line in reader.lines() {
            let line = line?;
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 8 {
                let reads: u64 = parts[3].parse().unwrap_or(0);
                let writes: u64 = parts[7].parse().unwrap_or(0);

                let dev_name = parts[2];
                if dev_name.starts_with("loop") || dev_name.starts_with("ram") {
                    continue;
                }

                total_ops += reads + writes;
            }
        }

        Ok(total_ops)
    }
}

fn parse_kb(line: &str) -> u64 {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() >= 2 {
        parts[1].parse().unwrap_or(0)
    } else {
        0
    }
}
