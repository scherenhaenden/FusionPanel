#![forbid(unsafe_code)]

use sysinfo::System;

/// Calculates system resource load, warns on OOM danger, and finds processes to throttle.
#[derive(Debug)]
pub struct SystemGovernor {
    sys: System,
    cpu_weight: f32,
    mem_weight: f32,
}

impl SystemGovernor {
    pub fn new(cpu_weight: f32, mem_weight: f32) -> Result<Self, String> {
        let mut sys = System::new_all();
        sys.refresh_all();
        Ok(Self {
            sys,
            cpu_weight,
            mem_weight,
        })
    }

    /// Refresh the internal system state
    pub fn refresh(&mut self) -> Result<(), String> {
        self.sys.refresh_all();
        Ok(())
    }

    /// Computes a weighted load score based on global CPU and Memory usage.
    /// Score is (cpu_usage_percent * cpu_weight) + (mem_usage_percent * mem_weight)
    pub fn compute_load_score(&mut self) -> Result<f32, String> {
        self.refresh()?;
        let cpu_usage = self.sys.global_cpu_info().cpu_usage();

        let total_mem = self.sys.total_memory();
        let used_mem = self.sys.used_memory();
        let mem_usage = if total_mem > 0 {
            (used_mem as f32 / total_mem as f32) * 100.0
        } else {
            0.0
        };

        Ok((cpu_usage * self.cpu_weight) + (mem_usage * self.mem_weight))
    }

    /// Warns if the memory usage is dangerously high (e.g. >= 90%).
    /// Returns true if system is near OOM.
    pub fn is_near_oom(&mut self, threshold_percent: f32) -> Result<bool, String> {
        self.refresh()?;
        let total_mem = self.sys.total_memory();
        let used_mem = self.sys.used_memory();

        let mem_usage = if total_mem > 0 {
            (used_mem as f32 / total_mem as f32) * 100.0
        } else {
            0.0
        };

        Ok(mem_usage >= threshold_percent)
    }

    /// Identifies low-priority workers to throttle.
    /// Returns a list of process IDs that consume significant resources
    /// or match known low-priority patterns (e.g., background workers, batch processors).
    /// For this simple governor, we just return processes with high memory footprint.
    pub fn identify_throttle_candidates(
        &mut self,
        min_memory_bytes: u64,
    ) -> Result<Vec<u32>, String> {
        self.refresh()?;
        let mut candidates = Vec::new();

        for (pid, process) in self.sys.processes() {
            if process.memory() > min_memory_bytes {
                // For safety and logic boundaries we only target a simulated subset or by purely threshold
                candidates.push(pid.as_u32());
            }
        }

        Ok(candidates)
    }
}
