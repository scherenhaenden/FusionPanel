#![forbid(unsafe_code)]

use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PsiError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Parse error: {0}")]
    Parse(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct PsiMetrics {
    pub avg10: f64,
    pub avg60: f64,
    pub avg300: f64,
    pub total: u64,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct PressureState {
    pub some: Option<PsiMetrics>,
    pub full: Option<PsiMetrics>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct MemoryEvents {
    pub oom: u64,
    pub oom_kill: u64,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct CgroupEvents {
    pub populated: bool,
    pub frozen: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AlertType {
    None,
    Throttle,
    Evict,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PressureThresholds {
    pub some_throttle: f64,
    pub full_throttle: f64,
    pub some_evict: f64,
    pub full_evict: f64,
}

impl Default for PressureThresholds {
    fn default() -> Self {
        Self {
            some_throttle: 20.0,
            full_throttle: 5.0,
            some_evict: 50.0,
            full_evict: 20.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CgroupV2PressureMonitor {
    cgroup_dir: PathBuf,
    last_memory_events: MemoryEvents,
}

impl CgroupV2PressureMonitor {
    pub fn new<P: AsRef<Path>>(cgroup_dir: P) -> Self {
        Self {
            cgroup_dir: cgroup_dir.as_ref().to_path_buf(),
            last_memory_events: MemoryEvents::default(),
        }
    }

    fn parse_psi_file(path: &Path) -> Result<PressureState, PsiError> {
        let content = fs::read_to_string(path)?;
        let mut state = PressureState::default();

        for line in content.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.is_empty() {
                continue;
            }

            let kind = parts[0];
            if kind != "some" && kind != "full" {
                continue;
            }

            let mut avg10 = 0.0;
            let mut avg60 = 0.0;
            let mut avg300 = 0.0;
            let mut total = 0;

            for part in &parts[1..] {
                if let Some((key, val)) = part.split_once('=') {
                    match key {
                        "avg10" => {
                            avg10 = val
                                .parse()
                                .map_err(|_| PsiError::Parse(format!("Invalid avg10: {}", val)))?
                        }
                        "avg60" => {
                            avg60 = val
                                .parse()
                                .map_err(|_| PsiError::Parse(format!("Invalid avg60: {}", val)))?
                        }
                        "avg300" => {
                            avg300 = val
                                .parse()
                                .map_err(|_| PsiError::Parse(format!("Invalid avg300: {}", val)))?
                        }
                        "total" => {
                            total = val
                                .parse()
                                .map_err(|_| PsiError::Parse(format!("Invalid total: {}", val)))?
                        }
                        _ => {}
                    }
                }
            }

            let metrics = PsiMetrics {
                avg10,
                avg60,
                avg300,
                total,
            };
            if kind == "some" {
                state.some = Some(metrics);
            } else {
                state.full = Some(metrics);
            }
        }

        Ok(state)
    }

    pub fn cpu_pressure(&self) -> Result<PressureState, PsiError> {
        Self::parse_psi_file(&self.cgroup_dir.join("cpu.pressure"))
    }

    pub fn memory_pressure(&self) -> Result<PressureState, PsiError> {
        Self::parse_psi_file(&self.cgroup_dir.join("memory.pressure"))
    }

    pub fn io_pressure(&self) -> Result<PressureState, PsiError> {
        Self::parse_psi_file(&self.cgroup_dir.join("io.pressure"))
    }

    pub fn read_memory_events(&self) -> Result<MemoryEvents, PsiError> {
        let path = self.cgroup_dir.join("memory.events");
        let content = fs::read_to_string(path)?;
        let mut events = MemoryEvents::default();

        for line in content.lines() {
            if let Some((key, val)) = line.split_once(' ') {
                let key = key.trim();
                let val = val.trim();
                match key {
                    "oom" => {
                        events.oom = val
                            .parse()
                            .map_err(|_| PsiError::Parse(format!("Invalid oom: {}", val)))?
                    }
                    "oom_kill" => {
                        events.oom_kill = val
                            .parse()
                            .map_err(|_| PsiError::Parse(format!("Invalid oom_kill: {}", val)))?
                    }
                    _ => {}
                }
            }
        }

        Ok(events)
    }

    pub fn read_cgroup_events(&self) -> Result<CgroupEvents, PsiError> {
        let path = self.cgroup_dir.join("cgroup.events");
        let content = fs::read_to_string(path)?;
        let mut events = CgroupEvents::default();

        for line in content.lines() {
            if let Some((key, val)) = line.split_once(' ') {
                let key = key.trim();
                let val = val.trim();
                match key {
                    "populated" => events.populated = val == "1",
                    "frozen" => events.frozen = val == "1",
                    _ => {}
                }
            }
        }

        Ok(events)
    }

    pub fn evaluate_health(
        &mut self,
        thresholds: &PressureThresholds,
    ) -> Result<(f64, AlertType), PsiError> {
        let mem_events = self.read_memory_events().unwrap_or_default();
        let oom_inc = mem_events.oom.saturating_sub(self.last_memory_events.oom);
        let oom_kill_inc = mem_events
            .oom_kill
            .saturating_sub(self.last_memory_events.oom_kill);
        self.last_memory_events = mem_events.clone();

        if oom_kill_inc > 0 {
            return Ok((0.0, AlertType::Evict));
        }

        let mut max_some_avg10 = 0.0_f64;
        let mut max_full_avg10 = 0.0_f64;

        let cpu = self.cpu_pressure().unwrap_or_default();
        let mem = self.memory_pressure().unwrap_or_default();
        let io = self.io_pressure().unwrap_or_default();

        for p in &[&cpu, &mem, &io] {
            if let Some(some) = &p.some {
                if some.avg10 > max_some_avg10 {
                    max_some_avg10 = some.avg10;
                }
            }
            if let Some(full) = &p.full {
                if full.avg10 > max_full_avg10 {
                    max_full_avg10 = full.avg10;
                }
            }
        }

        let health_score = (100.0 - max_some_avg10).clamp(0.0, 100.0);

        let alert = if oom_inc > 0
            || max_full_avg10 >= thresholds.full_evict
            || max_some_avg10 >= thresholds.some_evict
        {
            AlertType::Evict
        } else if max_full_avg10 >= thresholds.full_throttle
            || max_some_avg10 >= thresholds.some_throttle
        {
            AlertType::Throttle
        } else {
            AlertType::None
        };

        Ok((health_score, alert))
    }
}
