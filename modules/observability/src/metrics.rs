use prometheus::{Counter, Gauge, Registry};

pub struct MetricsRegistry {
    pub registry: Registry,
    pub cpu_usage: Gauge,
    pub ram_usage: Gauge,
    pub disk_usage: Gauge,
    pub network_rx_bytes: Counter,
    pub network_tx_bytes: Counter,
    pub site_requests: Counter,
    pub container_cpu: Gauge,
}

impl MetricsRegistry {
    pub fn new() -> prometheus::Result<Self> {
        let registry = Registry::new();

        let cpu_usage = Gauge::new("host_cpu_usage_percent", "Host CPU usage percentage")?;
        let ram_usage = Gauge::new("host_ram_usage_bytes", "Host RAM usage in bytes")?;
        let disk_usage = Gauge::new("host_disk_usage_bytes", "Host disk usage in bytes")?;
        let network_rx_bytes =
            Counter::new("host_network_rx_bytes_total", "Host network received bytes")?;
        let network_tx_bytes = Counter::new(
            "host_network_tx_bytes_total",
            "Host network transmitted bytes",
        )?;
        let site_requests = Counter::new("site_requests_total", "Total requests per site")?;
        let container_cpu = Gauge::new("container_cpu_usage_percent", "Container CPU usage")?;

        registry.register(Box::new(cpu_usage.clone()))?;
        registry.register(Box::new(ram_usage.clone()))?;
        registry.register(Box::new(disk_usage.clone()))?;
        registry.register(Box::new(network_rx_bytes.clone()))?;
        registry.register(Box::new(network_tx_bytes.clone()))?;
        registry.register(Box::new(site_requests.clone()))?;
        registry.register(Box::new(container_cpu.clone()))?;

        Ok(Self {
            registry,
            cpu_usage,
            ram_usage,
            disk_usage,
            network_rx_bytes,
            network_tx_bytes,
            site_requests,
            container_cpu,
        })
    }
}
