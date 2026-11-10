#![forbid(unsafe_code)]

use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LogicalCpu {
    pub id: u32,
    pub numa_node: u32,
    pub socket: u32,
    pub core: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostTopology {
    pub cpus: Vec<LogicalCpu>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AllocationResult {
    pub cpuset_cpus: String,
    pub cpuset_mems: String,
}

#[derive(Debug)]
pub struct CpuAllocator {
    topology: HostTopology,
    available_cpus: HashSet<u32>,
}

impl CpuAllocator {
    pub fn new(topology: HostTopology) -> Self {
        let available_cpus = topology.cpus.iter().map(|cpu| cpu.id).collect();
        Self {
            topology,
            available_cpus,
        }
    }

    pub fn allocate(&mut self, num_cpus: usize) -> Option<AllocationResult> {
        if num_cpus > self.available_cpus.len() {
            return None;
        }

        // Group available CPUs by NUMA node
        let mut numa_cpus: HashMap<u32, Vec<LogicalCpu>> = HashMap::new();
        for cpu in &self.topology.cpus {
            if self.available_cpus.contains(&cpu.id) {
                numa_cpus
                    .entry(cpu.numa_node)
                    .or_default()
                    .push(cpu.clone());
            }
        }

        // Sort NUMA nodes by available CPU count (descending) to prefer nodes with most CPUs
        let mut numa_nodes: Vec<_> = numa_cpus.keys().cloned().collect();
        numa_nodes.sort_by_key(|node| std::cmp::Reverse(numa_cpus[node].len()));

        let mut allocated = Vec::new();
        let mut allocated_mems = HashSet::new();

        // Try to allocate from NUMA nodes, prioritizing filling one node before moving to another
        for node in numa_nodes {
            if allocated.len() >= num_cpus {
                break;
            }

            if let Some(cpus_in_node) = numa_cpus.get_mut(&node) {
                // Sort CPUs in node by Socket, then Core to allocate logically close CPUs
                cpus_in_node.sort_by_key(|cpu| (cpu.socket, cpu.core, cpu.id));

                while allocated.len() < num_cpus && !cpus_in_node.is_empty() {
                    let cpu = cpus_in_node.remove(0);
                    allocated.push(cpu.id);
                    allocated_mems.insert(cpu.numa_node);
                    self.available_cpus.remove(&cpu.id);
                }
            }
        }

        if allocated.len() < num_cpus {
            // Rollback if unable to satisfy allocation (should not happen due to initial check, but safety first)
            for cpu_id in allocated {
                self.available_cpus.insert(cpu_id);
            }
            return None;
        }

        Some(AllocationResult {
            cpuset_cpus: format_cpuset(&allocated),
            cpuset_mems: format_cpuset(&allocated_mems.into_iter().collect::<Vec<_>>()),
        })
    }

    pub fn free(&mut self, allocated_cpus: &str) -> Result<(), String> {
        let cpus = parse_cpuset(allocated_cpus)?;
        for cpu in cpus {
            if self.topology.cpus.iter().any(|c| c.id == cpu) {
                self.available_cpus.insert(cpu);
            }
        }
        Ok(())
    }
}

pub fn format_cpuset(cpus: &[u32]) -> String {
    if cpus.is_empty() {
        return String::new();
    }

    let mut sorted_cpus = cpus.to_vec();
    sorted_cpus.sort_unstable();

    let mut ranges = Vec::new();
    let mut start = sorted_cpus[0];
    let mut prev = sorted_cpus[0];

    for &cpu in &sorted_cpus[1..] {
        if cpu == prev + 1 {
            prev = cpu;
        } else {
            if start == prev {
                ranges.push(start.to_string());
            } else {
                ranges.push(format!("{}-{}", start, prev));
            }
            start = cpu;
            prev = cpu;
        }
    }

    if start == prev {
        ranges.push(start.to_string());
    } else {
        ranges.push(format!("{}-{}", start, prev));
    }

    ranges.join(",")
}

pub fn parse_cpuset(s: &str) -> Result<Vec<u32>, String> {
    if s.is_empty() {
        return Ok(Vec::new());
    }

    let mut cpus = HashSet::new();
    for part in s.split(',') {
        if let Some(dash_idx) = part.find('-') {
            let start = part[..dash_idx]
                .parse::<u32>()
                .map_err(|_| format!("Invalid number: {}", &part[..dash_idx]))?;
            let end = part[dash_idx + 1..]
                .parse::<u32>()
                .map_err(|_| format!("Invalid number: {}", &part[dash_idx + 1..]))?;
            if start > end {
                return Err(format!("Invalid range: {}", part));
            }
            for cpu in start..=end {
                cpus.insert(cpu);
            }
        } else {
            let cpu = part
                .parse::<u32>()
                .map_err(|_| format!("Invalid number: {}", part))?;
            cpus.insert(cpu);
        }
    }

    let mut cpus_vec: Vec<u32> = cpus.into_iter().collect();
    cpus_vec.sort_unstable();
    Ok(cpus_vec)
}
