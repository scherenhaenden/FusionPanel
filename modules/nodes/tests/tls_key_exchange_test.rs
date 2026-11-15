#![forbid(unsafe_code)]

use nodes::tls_key_exchange::{MeshRekeyScheduler, RekeyError, RekeyEventCallback, RekeyPolicy};
use std::cell::RefCell;
use std::collections::HashMap;

struct TestCallback {
    rekeys: RefCell<HashMap<String, Vec<u64>>>,
    timeouts: RefCell<HashMap<String, Vec<u64>>>,
}

impl TestCallback {
    fn new() -> Self {
        Self {
            rekeys: RefCell::new(HashMap::new()),
            timeouts: RefCell::new(HashMap::new()),
        }
    }
}

impl RekeyEventCallback for TestCallback {
    fn on_rekey_completed(&self, node_id: &str, new_epoch: u64) {
        self.rekeys
            .borrow_mut()
            .entry(node_id.to_string())
            .or_default()
            .push(new_epoch);
    }

    fn on_epoch_timeout(&self, node_id: &str, expired_epoch: u64) {
        self.timeouts
            .borrow_mut()
            .entry(node_id.to_string())
            .or_default()
            .push(expired_epoch);
    }
}

#[test]
fn test_rekey_on_bytes_threshold() {
    let policy = RekeyPolicy {
        max_bytes: 1000,
        ..RekeyPolicy::default()
    };
    let cb = TestCallback::new();
    let mut scheduler = MeshRekeyScheduler::new(policy, cb);

    let current_time = 1000;
    scheduler.register_node("node1".to_string(), 1, current_time);

    // Transfer 600 bytes
    let rekeyed = scheduler
        .record_transfer("node1", 600, current_time)
        .unwrap();
    assert!(!rekeyed);
    assert_eq!(scheduler.sessions.get("node1").unwrap().active_epoch, 1);

    // Transfer 400 bytes, total 1000
    let rekeyed = scheduler
        .record_transfer("node1", 400, current_time)
        .unwrap();
    assert!(rekeyed);
    assert_eq!(scheduler.sessions.get("node1").unwrap().active_epoch, 2);

    assert_eq!(
        scheduler.callback.rekeys.borrow().get("node1"),
        Some(&vec![2])
    );
}

#[test]
fn test_rekey_on_packets_threshold() {
    let policy = RekeyPolicy {
        max_packets: 3,
        ..RekeyPolicy::default()
    };
    let cb = TestCallback::new();
    let mut scheduler = MeshRekeyScheduler::new(policy, cb);

    let current_time = 1000;
    scheduler.register_node("node1".to_string(), 1, current_time);

    scheduler
        .record_transfer("node1", 10, current_time)
        .unwrap(); // pkt 1
    scheduler
        .record_transfer("node1", 10, current_time)
        .unwrap(); // pkt 2
    assert_eq!(scheduler.sessions.get("node1").unwrap().active_epoch, 1);

    scheduler
        .record_transfer("node1", 10, current_time)
        .unwrap(); // pkt 3 (threshold reached)
    assert_eq!(scheduler.sessions.get("node1").unwrap().active_epoch, 2);
}

#[test]
fn test_rekey_on_time_limit() {
    let policy = RekeyPolicy {
        ttl_secs: 3600,
        ..RekeyPolicy::default()
    };
    let cb = TestCallback::new();
    let mut scheduler = MeshRekeyScheduler::new(policy, cb);

    scheduler.register_node("node1".to_string(), 1, 1000);

    // 3599s later, not quite expired
    let rekeyed = scheduler.check_ttl("node1", 4599).unwrap();
    assert!(!rekeyed);
    assert_eq!(scheduler.sessions.get("node1").unwrap().active_epoch, 1);

    // 3600s later, expired
    let rekeyed = scheduler.check_ttl("node1", 4600).unwrap();
    assert!(rekeyed);
    assert_eq!(scheduler.sessions.get("node1").unwrap().active_epoch, 2);
}

#[test]
fn test_dual_epoch_resolution_and_eviction() {
    let policy = RekeyPolicy {
        transition_window_secs: 60,
        ..RekeyPolicy::default()
    };
    let cb = TestCallback::new();
    let mut scheduler = MeshRekeyScheduler::new(policy, cb);

    let mut current_time = 1000;
    scheduler.register_node("node1".to_string(), 1, current_time);

    // Force rekey
    scheduler.trigger_rekey("node1", current_time).unwrap();
    assert_eq!(scheduler.sessions.get("node1").unwrap().active_epoch, 2);

    // Both epochs should be valid during transition window
    assert!(scheduler.resolve_epoch("node1", 2, current_time).unwrap()); // Active
    assert!(scheduler.resolve_epoch("node1", 1, current_time).unwrap()); // Previous (valid)
    assert_eq!(
        scheduler
            .resolve_epoch("node1", 3, current_time)
            .unwrap_err(),
        RekeyError::InvalidEpoch(3)
    );

    current_time += 30; // Within window
    assert!(scheduler.resolve_epoch("node1", 1, current_time).unwrap());

    scheduler.evict_expired_epochs(current_time);
    assert!(scheduler
        .sessions
        .get("node1")
        .unwrap()
        .previous_epoch
        .is_some());
    assert!(scheduler.callback.timeouts.borrow().get("node1").is_none());

    current_time += 30; // Exactly at the end of the window
    assert_eq!(
        scheduler
            .resolve_epoch("node1", 1, current_time)
            .unwrap_err(),
        RekeyError::InvalidEpoch(1)
    );

    scheduler.evict_expired_epochs(current_time);
    assert!(scheduler
        .sessions
        .get("node1")
        .unwrap()
        .previous_epoch
        .is_none());
    assert_eq!(
        scheduler.callback.timeouts.borrow().get("node1"),
        Some(&vec![1])
    );
}
