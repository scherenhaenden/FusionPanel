#![forbid(unsafe_code)]

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct CgroupMemoryController {
    cgroup_path: PathBuf,
}

#[derive(Debug, Default, PartialEq, Eq, Clone)]
pub struct MemoryEvents {
    pub low: u64,
    pub high: u64,
    pub max: u64,
    pub oom: u64,
    pub oom_kill: u64,
    pub oom_group_kill: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryLimit {
    Bytes(u64),
    Max,
}

impl std::fmt::Display for MemoryLimit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MemoryLimit::Bytes(b) => write!(f, "{}", b),
            MemoryLimit::Max => write!(f, "max"),
        }
    }
}

impl CgroupMemoryController {
    pub fn new<P: AsRef<Path>>(cgroup_path: P) -> Self {
        Self {
            cgroup_path: cgroup_path.as_ref().to_path_buf(),
        }
    }

    fn write_limit(&self, filename: &str, limit: &MemoryLimit) -> io::Result<()> {
        let path = self.cgroup_path.join(filename);
        fs::write(path, limit.to_string().as_bytes())
    }

    pub fn set_memory_max(&self, limit: MemoryLimit) -> io::Result<()> {
        self.write_limit("memory.max", &limit)
    }

    pub fn set_memory_high(&self, limit: MemoryLimit) -> io::Result<()> {
        self.write_limit("memory.high", &limit)
    }

    pub fn set_memory_low(&self, limit: MemoryLimit) -> io::Result<()> {
        self.write_limit("memory.low", &limit)
    }

    pub fn set_memory_swap_max(&self, limit: MemoryLimit) -> io::Result<()> {
        self.write_limit("memory.swap.max", &limit)
    }

    pub fn read_memory_events(&self) -> io::Result<MemoryEvents> {
        let path = self.cgroup_path.join("memory.events");
        let content = fs::read_to_string(path)?;
        let mut events = MemoryEvents::default();

        for line in content.lines() {
            let mut parts = line.split_whitespace();
            if let (Some(key), Some(val_str)) = (parts.next(), parts.next()) {
                if let Ok(val) = val_str.parse::<u64>() {
                    match key {
                        "low" => events.low = val,
                        "high" => events.high = val,
                        "max" => events.max = val,
                        "oom" => events.oom = val,
                        "oom_kill" => events.oom_kill = val,
                        "oom_group_kill" => events.oom_group_kill = val,
                        _ => {}
                    }
                }
            }
        }
        Ok(events)
    }
}
