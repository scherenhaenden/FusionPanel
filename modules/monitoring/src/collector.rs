use sysinfo::System;

#[derive(Debug, Clone, PartialEq)]
pub struct HostMetrics {
    pub cpu_usage_percent: f32,
    pub memory_usage_percent: f32,
    pub disk_usage_percent: f32,
    pub network_rx_bytes: u64,
    pub network_tx_bytes: u64,
}

#[derive(Debug)]
pub struct MetricCollector {
    sys: System,
}

impl MetricCollector {
    pub fn new() -> Result<Self, String> {
        // Fail closed with BLOCKED if required system metrics providers (/proc) are restricted.
        if !std::path::Path::new("/proc/stat").exists() {
            return Err("BLOCKED: /proc/stat is not available or restricted".to_string());
        }
        
        let mut sys = System::new_all();
        sys.refresh_all();
        Ok(Self { sys })
    }

    pub fn collect(&mut self) -> Result<HostMetrics, String> {
        if !std::path::Path::new("/proc/stat").exists() {
            return Err("BLOCKED: /proc/stat is not available or restricted".to_string());
        }

        self.sys.refresh_all();

        let cpu_usage_percent = self.sys.global_cpu_info().cpu_usage();

        let total_mem = self.sys.total_memory();
        let used_mem = self.sys.used_memory();
        let memory_usage_percent = if total_mem > 0 {
            (used_mem as f64 / total_mem as f64 * 100.0) as f32
        } else {
            0.0
        };

        let disks = sysinfo::Disks::new_with_refreshed_list();
        let mut total_disk_space = 0;
        let mut total_disk_used = 0;

        for disk in disks.list() {
            total_disk_space += disk.total_space();
            total_disk_used += disk.total_space() - disk.available_space();
        }

        let disk_usage_percent = if total_disk_space > 0 {
            (total_disk_used as f64 / total_disk_space as f64 * 100.0) as f32
        } else {
            0.0
        };

        let networks = sysinfo::Networks::new_with_refreshed_list();
        let mut network_rx_bytes = 0;
        let mut network_tx_bytes = 0;

        for (_, network_data) in networks.iter() {
            network_rx_bytes += network_data.total_received();
            network_tx_bytes += network_data.total_transmitted();
        }

        Ok(HostMetrics {
            cpu_usage_percent,
            memory_usage_percent,
            disk_usage_percent,
            network_rx_bytes,
            network_tx_bytes,
        })
    }
}
