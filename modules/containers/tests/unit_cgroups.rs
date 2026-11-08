#![forbid(unsafe_code)]

use containers::cgroups::{CgroupMemoryController, MemoryLimit, MemoryEvents};
use std::fs;
use tempfile::tempdir;

#[test]
fn test_set_memory_max() {
    let dir = tempdir().unwrap();
    let controller = CgroupMemoryController::new(dir.path());

    // Set max
    controller.set_memory_max(MemoryLimit::Max).unwrap();
    let content = fs::read_to_string(dir.path().join("memory.max")).unwrap();
    assert_eq!(content, "max");

    // Set bytes
    controller.set_memory_max(MemoryLimit::Bytes(1024 * 1024)).unwrap();
    let content = fs::read_to_string(dir.path().join("memory.max")).unwrap();
    assert_eq!(content, "1048576");
}

#[test]
fn test_set_memory_high() {
    let dir = tempdir().unwrap();
    let controller = CgroupMemoryController::new(dir.path());

    controller.set_memory_high(MemoryLimit::Bytes(500)).unwrap();
    let content = fs::read_to_string(dir.path().join("memory.high")).unwrap();
    assert_eq!(content, "500");
}

#[test]
fn test_set_memory_low() {
    let dir = tempdir().unwrap();
    let controller = CgroupMemoryController::new(dir.path());

    controller.set_memory_low(MemoryLimit::Bytes(100)).unwrap();
    let content = fs::read_to_string(dir.path().join("memory.low")).unwrap();
    assert_eq!(content, "100");
}

#[test]
fn test_set_memory_swap_max() {
    let dir = tempdir().unwrap();
    let controller = CgroupMemoryController::new(dir.path());

    controller.set_memory_swap_max(MemoryLimit::Max).unwrap();
    let content = fs::read_to_string(dir.path().join("memory.swap.max")).unwrap();
    assert_eq!(content, "max");

    controller.set_memory_swap_max(MemoryLimit::Bytes(2048)).unwrap();
    let content = fs::read_to_string(dir.path().join("memory.swap.max")).unwrap();
    assert_eq!(content, "2048");
}

#[test]
fn test_read_memory_events() {
    let dir = tempdir().unwrap();
    let controller = CgroupMemoryController::new(dir.path());

    let events_content = "\
low 1
high 2
max 3
oom 4
oom_kill 5
oom_group_kill 6
";
    fs::write(dir.path().join("memory.events"), events_content).unwrap();

    let events = controller.read_memory_events().unwrap();
    assert_eq!(
        events,
        MemoryEvents {
            low: 1,
            high: 2,
            max: 3,
            oom: 4,
            oom_kill: 5,
            oom_group_kill: 6,
        }
    );
}

#[test]
fn test_read_memory_events_missing_fields() {
    let dir = tempdir().unwrap();
    let controller = CgroupMemoryController::new(dir.path());

    // Only partial events present
    let events_content = "\
oom 10
oom_kill 15
";
    fs::write(dir.path().join("memory.events"), events_content).unwrap();

    let events = controller.read_memory_events().unwrap();
    assert_eq!(
        events,
        MemoryEvents {
            low: 0,
            high: 0,
            max: 0,
            oom: 10,
            oom_kill: 15,
            oom_group_kill: 0,
        }
    );
}

#[test]
fn test_read_memory_events_unknown_fields() {
    let dir = tempdir().unwrap();
    let controller = CgroupMemoryController::new(dir.path());

    // Partial with unknown fields (which should be ignored)
    let events_content = "\
low 1
some_new_feature 99
oom 2
";
    fs::write(dir.path().join("memory.events"), events_content).unwrap();

    let events = controller.read_memory_events().unwrap();
    assert_eq!(
        events,
        MemoryEvents {
            low: 1,
            high: 0,
            max: 0,
            oom: 2,
            oom_kill: 0,
            oom_group_kill: 0,
        }
    );
}
