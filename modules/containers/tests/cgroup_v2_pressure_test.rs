#![forbid(unsafe_code)]

use containers::cgroup_v2_pressure::{AlertType, CgroupV2PressureMonitor, PressureThresholds};
use std::fs;
use tempfile::tempdir;

#[test]
fn test_parse_psi_files() {
    let dir = tempdir().unwrap();

    let cpu_content = "\
some avg10=2.34 avg60=1.23 avg300=0.45 total=12345
full avg10=1.12 avg60=0.56 avg300=0.12 total=6789
";
    fs::write(dir.path().join("cpu.pressure"), cpu_content).unwrap();

    let monitor = CgroupV2PressureMonitor::new(dir.path());
    let state = monitor
        .cpu_pressure()
        .expect("Failed to parse cpu.pressure");

    let some = state.some.expect("Expected some metrics");
    assert_eq!(some.avg10, 2.34);
    assert_eq!(some.avg60, 1.23);
    assert_eq!(some.avg300, 0.45);
    assert_eq!(some.total, 12345);

    let full = state.full.expect("Expected full metrics");
    assert_eq!(full.avg10, 1.12);
    assert_eq!(full.avg60, 0.56);
    assert_eq!(full.avg300, 0.12);
    assert_eq!(full.total, 6789);
}

#[test]
fn test_parse_memory_events() {
    let dir = tempdir().unwrap();

    let mem_content = "\
low 1
high 2
max 3
oom 42
oom_kill 5
";
    fs::write(dir.path().join("memory.events"), mem_content).unwrap();

    let monitor = CgroupV2PressureMonitor::new(dir.path());
    let events = monitor
        .read_memory_events()
        .expect("Failed to parse memory.events");

    assert_eq!(events.oom, 42);
    assert_eq!(events.oom_kill, 5);
}

#[test]
fn test_parse_cgroup_events() {
    let dir = tempdir().unwrap();

    let cgroup_content = "\
populated 1
frozen 0
";
    fs::write(dir.path().join("cgroup.events"), cgroup_content).unwrap();

    let monitor = CgroupV2PressureMonitor::new(dir.path());
    let events = monitor
        .read_cgroup_events()
        .expect("Failed to parse cgroup.events");

    assert_eq!(events.populated, true);
    assert_eq!(events.frozen, false);
}

#[test]
fn test_health_evaluation_throttle() {
    let dir = tempdir().unwrap();

    let cpu_content = "some avg10=25.0 avg60=20.0 avg300=15.0 total=1000\nfull avg10=1.0 avg60=0.5 avg300=0.1 total=100";
    fs::write(dir.path().join("cpu.pressure"), cpu_content).unwrap();

    fs::write(dir.path().join("memory.events"), "oom 0\noom_kill 0").unwrap();

    let mut monitor = CgroupV2PressureMonitor::new(dir.path());
    let thresholds = PressureThresholds::default(); // some_throttle = 20.0

    let (health, alert) = monitor.evaluate_health(&thresholds).unwrap();
    assert_eq!(health, 75.0); // 100.0 - 25.0
    assert_eq!(alert, AlertType::Throttle);
}

#[test]
fn test_health_evaluation_evict_due_to_oom_kill() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("memory.events"), "oom 0\noom_kill 1").unwrap();

    let mut monitor = CgroupV2PressureMonitor::new(dir.path());
    let thresholds = PressureThresholds::default();

    let (health, alert) = monitor.evaluate_health(&thresholds).unwrap();
    assert_eq!(health, 0.0);
    assert_eq!(alert, AlertType::Evict);
}

#[test]
fn test_health_evaluation_evict_due_to_oom_inc() {
    let dir = tempdir().unwrap();

    // First evaluation
    fs::write(dir.path().join("memory.events"), "oom 0\noom_kill 0").unwrap();
    let mut monitor = CgroupV2PressureMonitor::new(dir.path());
    let thresholds = PressureThresholds::default();
    monitor.evaluate_health(&thresholds).unwrap();

    // Second evaluation with oom increment
    fs::write(dir.path().join("memory.events"), "oom 2\noom_kill 0").unwrap();
    let (_, alert) = monitor.evaluate_health(&thresholds).unwrap();
    assert_eq!(alert, AlertType::Evict);
}

#[test]
fn test_health_evaluation_evict_due_to_pressure() {
    let dir = tempdir().unwrap();

    let cpu_content = "some avg10=60.0 avg60=20.0 avg300=15.0 total=1000\nfull avg10=1.0 avg60=0.5 avg300=0.1 total=100";
    fs::write(dir.path().join("cpu.pressure"), cpu_content).unwrap();
    fs::write(dir.path().join("memory.events"), "oom 0\noom_kill 0").unwrap();

    let mut monitor = CgroupV2PressureMonitor::new(dir.path());
    let thresholds = PressureThresholds::default(); // some_evict = 50.0

    let (health, alert) = monitor.evaluate_health(&thresholds).unwrap();
    assert_eq!(health, 40.0); // 100.0 - 60.0
    assert_eq!(alert, AlertType::Evict);
}
