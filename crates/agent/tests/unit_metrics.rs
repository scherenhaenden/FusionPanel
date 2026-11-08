#![forbid(unsafe_code)]

use agent::metrics::MetricsCollector;
use std::fs;
use std::io::Write;
use tempfile::TempDir;

#[test]
fn test_metrics_collector() {
    let temp_dir = TempDir::new().unwrap();
    let stat_path = temp_dir.path().join("stat");
    let meminfo_path = temp_dir.path().join("meminfo");
    let diskstats_path = temp_dir.path().join("diskstats");
    let cgroup_v1_path = temp_dir.path().join("cgroup_v1");
    let cgroup_v2_path = temp_dir.path().join("cgroup_v2");

    fs::create_dir_all(&cgroup_v1_path).unwrap();
    fs::create_dir_all(&cgroup_v2_path).unwrap();

    // Mock /proc/stat
    let mut stat_file = fs::File::create(&stat_path).unwrap();
    writeln!(stat_file, "cpu  1000 0 500 8000 100 0 0 0 0 0").unwrap();

    // Mock /proc/meminfo
    let mut meminfo_file = fs::File::create(&meminfo_path).unwrap();
    writeln!(meminfo_file, "MemTotal:       16000000 kB\nMemFree:         8000000 kB\nMemAvailable:   10000000 kB\nBuffers:          500000 kB\nCached:          1500000 kB").unwrap();

    // Mock /proc/diskstats
    let mut diskstats_file = fs::File::create(&diskstats_path).unwrap();
    writeln!(
        diskstats_file,
        "   8       0 sda 1000 0 0 0 500 0 0 0 0 0 0"
    )
    .unwrap();

    let mut collector = MetricsCollector::new(
        &stat_path,
        &meminfo_path,
        &diskstats_path,
        &cgroup_v1_path,
        &cgroup_v2_path,
    );

    // Collect base without cgroups populated
    let metrics1 = collector.collect().unwrap();
    assert_eq!(metrics1.cpu_usage_percent, 0.0);
    assert_eq!(metrics1.memory_used_kb, 6000000);
    assert_eq!(metrics1.memory_available_kb, 10000000);
    assert_eq!(metrics1.disk_io_ops, 1500);

    // Mock cgroup v2
    let mut cg_v2_current = fs::File::create(cgroup_v2_path.join("memory.current")).unwrap();
    writeln!(cg_v2_current, "2048000").unwrap(); // 2000 KB used
    let mut cg_v2_max = fs::File::create(cgroup_v2_path.join("memory.max")).unwrap();
    writeln!(cg_v2_max, "4096000").unwrap(); // 4000 KB total, avail = 2000 KB

    // Update /proc/stat
    let mut stat_file = fs::File::create(&stat_path).unwrap();
    writeln!(stat_file, "cpu  1100 0 600 8100 100 0 0 0 0 0").unwrap();

    let metrics2 = collector.collect().unwrap();
    assert!((metrics2.cpu_usage_percent - 66.666).abs() < 0.01);

    // Check that cgroup v2 overrides mem used/avail
    assert_eq!(metrics2.memory_used_kb, 2000);
    assert_eq!(metrics2.memory_available_kb, 2000);
}
