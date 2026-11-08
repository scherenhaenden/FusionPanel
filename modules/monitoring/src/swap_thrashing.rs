#![forbid(unsafe_code)]

use std::fs;
use std::sync::Mutex;
use std::time::Instant;

#[derive(Debug, PartialEq, Clone)]
pub enum ThrashingLevel {
    Healthy,
    ModeratePressure,
    SevereThrashing,
}

#[derive(Debug, Clone)]
pub struct SwapHealthReport {
    pub level: ThrashingLevel,
    pub swap_in_rate: f64,
    pub swap_out_rate: f64,
    pub swap_total: u64,
    pub swap_free: u64,
    pub mem_available: u64,
    pub recommendations: Vec<String>,
}

#[derive(Debug, Default, Clone)]
struct VmStat {
    pswpin: u64,
    pswpout: u64,
}

#[derive(Debug, Default, Clone)]
struct MemInfo {
    swap_total: Option<u64>,
    swap_free: Option<u64>,
    mem_available: Option<u64>,
}

pub struct SwapThrashingDetector {
    vmstat_path: String,
    meminfo_path: String,
    last_stat: Mutex<Option<(VmStat, Instant)>>,
}

impl Default for SwapThrashingDetector {
    fn default() -> Self {
        Self::new("/proc/vmstat", "/proc/meminfo")
    }
}

impl SwapThrashingDetector {
    pub fn new(vmstat_path: &str, meminfo_path: &str) -> Self {
        Self {
            vmstat_path: vmstat_path.to_string(),
            meminfo_path: meminfo_path.to_string(),
            last_stat: Mutex::new(None),
        }
    }

    fn parse_vmstat(content: &str) -> Option<VmStat> {
        let mut pswpin = None;
        let mut pswpout = None;

        for line in content.lines() {
            let mut parts = line.split_whitespace();
            if let (Some(key), Some(value)) = (parts.next(), parts.next()) {
                if key == "pswpin" {
                    pswpin = value.parse::<u64>().ok();
                } else if key == "pswpout" {
                    pswpout = value.parse::<u64>().ok();
                }
            }
        }

        if let (Some(in_val), Some(out_val)) = (pswpin, pswpout) {
            Some(VmStat {
                pswpin: in_val,
                pswpout: out_val,
            })
        } else {
            None
        }
    }

    fn parse_meminfo(content: &str) -> Option<MemInfo> {
        let mut swap_total = None;
        let mut swap_free = None;
        let mut mem_available = None;

        for line in content.lines() {
            let mut parts = line.split_whitespace();
            if let (Some(key), Some(value)) = (parts.next(), parts.next()) {
                let key = key.trim_end_matches(':');
                if let Ok(val) = value.parse::<u64>() {
                    // Values in meminfo are typically in kB
                    let bytes = val.saturating_mul(1024);
                    match key {
                        "SwapTotal" => swap_total = Some(bytes),
                        "SwapFree" => swap_free = Some(bytes),
                        "MemAvailable" => mem_available = Some(bytes),
                        _ => {}
                    }
                }
            }
        }

        if swap_total.is_none() && swap_free.is_none() && mem_available.is_none() {
            None
        } else {
            Some(MemInfo {
                swap_total,
                swap_free,
                mem_available,
            })
        }
    }

    pub fn analyze(&self) -> Result<SwapHealthReport, String> {
        self.analyze_at(Instant::now())
    }

    pub fn analyze_at(&self, now: Instant) -> Result<SwapHealthReport, String> {
        let vmstat_content = fs::read_to_string(&self.vmstat_path)
            .map_err(|e| format!("Failed to read {}: {}", self.vmstat_path, e))?;
        let meminfo_content = fs::read_to_string(&self.meminfo_path)
            .map_err(|e| format!("Failed to read {}: {}", self.meminfo_path, e))?;

        self.analyze_content(&vmstat_content, &meminfo_content, now)
    }

    pub fn analyze_content(
        &self,
        vmstat_content: &str,
        meminfo_content: &str,
        now: Instant,
    ) -> Result<SwapHealthReport, String> {
        let curr_vmstat = Self::parse_vmstat(vmstat_content)
            .ok_or_else(|| "Failed to parse required keys from vmstat".to_string())?;

        let meminfo = Self::parse_meminfo(meminfo_content)
            .unwrap_or_default();

        let swap_total = meminfo.swap_total.unwrap_or(0);
        let swap_free = meminfo.swap_free.unwrap_or(0);
        let mem_available = meminfo.mem_available.unwrap_or(0);

        let mut swap_in_rate = 0.0;
        let mut swap_out_rate = 0.0;

        let mut last_stat_guard = self.last_stat.lock().map_err(|_| "Mutex poisoned")?;

        if let Some((prev_vmstat, prev_time)) = last_stat_guard.as_ref() {
            let duration = now.saturating_duration_since(*prev_time).as_secs_f64();
            if duration > 0.0 {
                if curr_vmstat.pswpin >= prev_vmstat.pswpin {
                    swap_in_rate = (curr_vmstat.pswpin - prev_vmstat.pswpin) as f64 / duration;
                }
                if curr_vmstat.pswpout >= prev_vmstat.pswpout {
                    swap_out_rate = (curr_vmstat.pswpout - prev_vmstat.pswpout) as f64 / duration;
                }
            }
        }

        *last_stat_guard = Some((curr_vmstat, now));
        drop(last_stat_guard);

        let total_rate = swap_in_rate + swap_out_rate;

        let level = if total_rate > 1000.0 {
            ThrashingLevel::SevereThrashing
        } else if total_rate > 100.0 {
            ThrashingLevel::ModeratePressure
        } else {
            ThrashingLevel::Healthy
        };

        let mut recommendations = Vec::new();
        match level {
            ThrashingLevel::SevereThrashing => {
                recommendations.push("System is actively thrashing. Immediate action required to free memory or increase capacity.".to_string());
            }
            ThrashingLevel::ModeratePressure => {
                recommendations.push("System is under memory pressure. Monitor swap usage and consider optimizing memory.".to_string());
            }
            ThrashingLevel::Healthy => {
                // No recommendation needed
            }
        }

        if mem_available > 0 && mem_available < 100 * 1024 * 1024 {
            recommendations.push("Available memory is critically low (< 100MB).".to_string());
        }
        if swap_total > 0 && swap_free < swap_total / 10 {
            recommendations.push("Swap space is running out (< 10% free).".to_string());
        }

        Ok(SwapHealthReport {
            level,
            swap_in_rate,
            swap_out_rate,
            swap_total,
            swap_free,
            mem_available,
            recommendations,
        })
    }
}
