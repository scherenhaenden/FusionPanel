#![forbid(unsafe_code)]

use monitoring::collector::HostMetrics;
use monitoring::prometheus::{format_metrics, ContainerMetrics, DatabaseMetrics};

#[test]
fn test_format_metrics_empty_collections() {
    let host = HostMetrics {
        cpu_usage_percent: 12.5,
        memory_usage_percent: 50.0,
        disk_usage_percent: 75.0,
        network_rx_bytes: 1000,
        network_tx_bytes: 2000,
    };

    let output = format_metrics(&host, &[], &[]);

    assert!(output.contains("# HELP host_cpu_usage_percent"));
    assert!(output.contains("# TYPE host_cpu_usage_percent gauge"));
    assert!(output.contains("host_cpu_usage_percent 12.5"));

    assert!(!output.contains("container_cpu_usage_percent"));
    assert!(!output.contains("database_active_connections"));

    assert!(output.ends_with("# EOF\n"));
}

#[test]
fn test_format_metrics_with_containers_and_dbs() {
    let host = HostMetrics {
        cpu_usage_percent: 12.5,
        memory_usage_percent: 50.0,
        disk_usage_percent: 75.0,
        network_rx_bytes: 1000,
        network_tx_bytes: 2000,
    };

    let containers = vec![
        ContainerMetrics {
            container_id: "cont1".to_string(),
            cpu_usage_percent: 5.0,
            memory_usage_bytes: 1024,
        },
        ContainerMetrics {
            container_id: "cont2".to_string(),
            cpu_usage_percent: 8.0,
            memory_usage_bytes: 2048,
        },
    ];

    let databases = vec![
        DatabaseMetrics {
            db_name: "db1".to_string(),
            active_connections: 10,
            query_latency_ms: 15.5,
        }
    ];

    let output = format_metrics(&host, &containers, &databases);

    assert!(output.contains("container_cpu_usage_percent{container_id=\"cont1\"} 5"));
    assert!(output.contains("container_memory_usage_bytes{container_id=\"cont2\"} 2048"));

    assert!(output.contains("database_active_connections{db_name=\"db1\"} 10"));
    assert!(output.contains("database_query_latency_ms{db_name=\"db1\"} 15.5"));

    assert!(output.ends_with("# EOF\n"));
}
