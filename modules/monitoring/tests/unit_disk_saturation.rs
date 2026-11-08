#![forbid(unsafe_code)]

use std::time::Duration;
use monitoring::disk_saturation::{DiskStatsEntry, DiskSaturationMonitor};

#[test]
fn test_parse_diskstats() {
    let mock_data = "
   8       0 sda 1000 0 2048 500 500 0 1024 300 0 800 800
   8       1 sda1 100 0 256 50 50 0 128 30 0 80 80
   7       0 loop0 10 0 20 5 5 0 10 5 0 10 10
   1       0 ram0 10 0 20 5 5 0 10 5 0 10 10
";
    let entries = DiskSaturationMonitor::parse_diskstats(mock_data);

    // Should filter out loop0 and ram0
    assert_eq!(entries.len(), 2);

    assert_eq!(entries[0].device_name, "sda");
    assert_eq!(entries[0].reads_completed, 1000);
    assert_eq!(entries[0].sectors_read, 2048);
    assert_eq!(entries[0].read_time_ms, 500);
    assert_eq!(entries[0].writes_completed, 500);
    assert_eq!(entries[0].sectors_written, 1024);
    assert_eq!(entries[0].write_time_ms, 300);
    assert_eq!(entries[0].io_time_ms, 800);
}

#[test]
fn test_evaluate_metrics_saturation() {
    let monitor = DiskSaturationMonitor::new(90.0);
    let elapsed = Duration::from_millis(1000); // 1 second

    let prev = DiskStatsEntry {
        device_name: "sda".to_string(),
        reads_completed: 100,
        sectors_read: 200,
        read_time_ms: 50,
        writes_completed: 100,
        sectors_written: 200,
        write_time_ms: 50,
        io_time_ms: 100,
    };

    let curr = DiskStatsEntry {
        device_name: "sda".to_string(),
        reads_completed: 150, // +50
        sectors_read: 300,
        read_time_ms: 100, // +50
        writes_completed: 150, // +50
        sectors_written: 300,
        write_time_ms: 100, // +50
        io_time_ms: 1050, // +950ms IO time over 1000ms elapsed = 95%
    };

    let metrics = monitor.evaluate_metrics(&prev, &curr, elapsed);
    assert_eq!(metrics.device_name, "sda");
    assert_eq!(metrics.utilization_pct, 95.0);

    // Total ops = 50 + 50 = 100. Total time = 50 + 50 = 100. 100 / 100 = 1.0 ms/op
    assert_eq!(metrics.avg_latency_ms, 1.0);
    assert!(metrics.is_saturated);
}

#[test]
fn test_evaluate_metrics_clamping() {
    let monitor = DiskSaturationMonitor::new(90.0);
    let elapsed = Duration::from_millis(1000);

    let prev = DiskStatsEntry {
        device_name: "sda".to_string(),
        io_time_ms: 100,
        ..Default::default()
    };

    // io_time_ms increased by 1500 in 1000ms. 150% -> clamped to 100%
    let curr = DiskStatsEntry {
        device_name: "sda".to_string(),
        io_time_ms: 1600,
        ..Default::default()
    };

    let metrics = monitor.evaluate_metrics(&prev, &curr, elapsed);
    assert_eq!(metrics.utilization_pct, 100.0);
}

#[test]
fn test_evaluate_metrics_div_by_zero() {
    let monitor = DiskSaturationMonitor::new(90.0);

    let prev = DiskStatsEntry::default();
    let curr = DiskStatsEntry::default();

    // 0 ops
    let metrics = monitor.evaluate_metrics(&prev, &curr, Duration::from_millis(1000));
    assert_eq!(metrics.avg_latency_ms, 0.0);

    // 0 elapsed time
    let metrics2 = monitor.evaluate_metrics(&prev, &curr, Duration::from_millis(0));
    assert_eq!(metrics2.utilization_pct, 0.0);
}

#[test]
fn test_evaluate_metrics_wraparound() {
    let monitor = DiskSaturationMonitor::new(90.0);

    // Simulate counter reset / wraparound
    let prev = DiskStatsEntry {
        device_name: "sda".to_string(),
        io_time_ms: u64::MAX - 100,
        ..Default::default()
    };
    let curr = DiskStatsEntry {
        device_name: "sda".to_string(),
        io_time_ms: 50, // Wrapped or reset. saturating_sub should prevent panic.
        ..Default::default()
    };

    let metrics = monitor.evaluate_metrics(&prev, &curr, Duration::from_millis(1000));
    // saturating_sub(u64::MAX - 100) from 50 should be 0.
    assert_eq!(metrics.utilization_pct, 0.0);
}
