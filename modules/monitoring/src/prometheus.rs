#![forbid(unsafe_code)]

use crate::collector::HostMetrics;
use std::fmt::Write;

#[derive(Debug, Clone, PartialEq)]
pub struct ContainerMetrics {
    pub container_id: String,
    pub cpu_usage_percent: f32,
    pub memory_usage_bytes: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DatabaseMetrics {
    pub db_name: String,
    pub active_connections: u32,
    pub query_latency_ms: f32,
}

pub fn format_metrics(
    host_metrics: &HostMetrics,
    container_metrics: &[ContainerMetrics],
    database_metrics: &[DatabaseMetrics],
) -> String {
    let mut out = String::new();

    // Host Metrics
    writeln!(
        out,
        "# HELP host_cpu_usage_percent Host CPU usage in percent."
    )
    .unwrap();
    writeln!(out, "# TYPE host_cpu_usage_percent gauge").unwrap();
    writeln!(
        out,
        "host_cpu_usage_percent {}",
        host_metrics.cpu_usage_percent
    )
    .unwrap();

    writeln!(
        out,
        "# HELP host_memory_usage_percent Host memory usage in percent."
    )
    .unwrap();
    writeln!(out, "# TYPE host_memory_usage_percent gauge").unwrap();
    writeln!(
        out,
        "host_memory_usage_percent {}",
        host_metrics.memory_usage_percent
    )
    .unwrap();

    writeln!(
        out,
        "# HELP host_disk_usage_percent Host disk usage in percent."
    )
    .unwrap();
    writeln!(out, "# TYPE host_disk_usage_percent gauge").unwrap();
    writeln!(
        out,
        "host_disk_usage_percent {}",
        host_metrics.disk_usage_percent
    )
    .unwrap();

    writeln!(
        out,
        "# HELP host_network_rx_bytes Host network received bytes."
    )
    .unwrap();
    writeln!(out, "# TYPE host_network_rx_bytes counter").unwrap();
    writeln!(
        out,
        "host_network_rx_bytes {}",
        host_metrics.network_rx_bytes
    )
    .unwrap();

    writeln!(
        out,
        "# HELP host_network_tx_bytes Host network transmitted bytes."
    )
    .unwrap();
    writeln!(out, "# TYPE host_network_tx_bytes counter").unwrap();
    writeln!(
        out,
        "host_network_tx_bytes {}",
        host_metrics.network_tx_bytes
    )
    .unwrap();

    // Container Metrics
    if !container_metrics.is_empty() {
        writeln!(
            out,
            "# HELP container_cpu_usage_percent Container CPU usage in percent."
        )
        .unwrap();
        writeln!(out, "# TYPE container_cpu_usage_percent gauge").unwrap();
        for c in container_metrics {
            writeln!(
                out,
                "container_cpu_usage_percent{{container_id=\"{}\"}} {}",
                c.container_id, c.cpu_usage_percent
            )
            .unwrap();
        }

        writeln!(
            out,
            "# HELP container_memory_usage_bytes Container memory usage in bytes."
        )
        .unwrap();
        writeln!(out, "# TYPE container_memory_usage_bytes gauge").unwrap();
        for c in container_metrics {
            writeln!(
                out,
                "container_memory_usage_bytes{{container_id=\"{}\"}} {}",
                c.container_id, c.memory_usage_bytes
            )
            .unwrap();
        }
    }

    // Database Metrics
    if !database_metrics.is_empty() {
        writeln!(
            out,
            "# HELP database_active_connections Database active connections."
        )
        .unwrap();
        writeln!(out, "# TYPE database_active_connections gauge").unwrap();
        for d in database_metrics {
            writeln!(
                out,
                "database_active_connections{{db_name=\"{}\"}} {}",
                d.db_name, d.active_connections
            )
            .unwrap();
        }

        writeln!(
            out,
            "# HELP database_query_latency_ms Database query latency in ms."
        )
        .unwrap();
        writeln!(out, "# TYPE database_query_latency_ms gauge").unwrap();
        for d in database_metrics {
            writeln!(
                out,
                "database_query_latency_ms{{db_name=\"{}\"}} {}",
                d.db_name, d.query_latency_ms
            )
            .unwrap();
        }
    }

    writeln!(out, "# EOF").unwrap();

    out
}
