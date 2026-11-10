#![forbid(unsafe_code)]

use std::str::FromStr;
use std::time::Duration;
use databases::gtid_monitor::{Gtid, GtidSet, DetectorState, MasterFailureDetector};

#[test]
fn test_gtid_parse_valid() {
    let gtid = Gtid::from_str("0-1-12345").unwrap();
    assert_eq!(gtid.domain_id, 0);
    assert_eq!(gtid.server_id, 1);
    assert_eq!(gtid.sequence_no, 12345);
}

#[test]
fn test_gtid_parse_invalid() {
    assert!(Gtid::from_str("0-1").is_err());
    assert!(Gtid::from_str("0-1-abc").is_err());
    assert!(Gtid::from_str("a-1-12345").is_err());
    assert!(Gtid::from_str("0-b-12345").is_err());
    assert!(Gtid::from_str("0-1-12345-6").is_err());
}

#[test]
fn test_gtid_display() {
    let gtid = Gtid { domain_id: 1, server_id: 2, sequence_no: 3 };
    assert_eq!(gtid.to_string(), "1-2-3");
}

#[test]
fn test_gtid_set_parse_valid() {
    let set = GtidSet::from_str("0-1-12345, 1-2-500").unwrap();
    assert_eq!(set.gtids.len(), 2);
    assert_eq!(set.gtids.get(&(0, 1)), Some(&12345));
    assert_eq!(set.gtids.get(&(1, 2)), Some(&500));
}

#[test]
fn test_gtid_set_parse_empty() {
    let set = GtidSet::from_str("").unwrap();
    assert!(set.gtids.is_empty());
}

#[test]
fn test_gtid_set_parse_overlap() {
    // Should take the maximum sequence_no
    let set = GtidSet::from_str("0-1-100, 0-1-50, 0-1-200").unwrap();
    assert_eq!(set.gtids.len(), 1);
    assert_eq!(set.gtids.get(&(0, 1)), Some(&200));
}

#[test]
fn test_gtid_set_display() {
    let mut set = GtidSet::new();
    set.add(Gtid::from_str("0-1-100").unwrap());
    set.add(Gtid::from_str("1-2-200").unwrap());
    assert_eq!(set.to_string(), "0-1-100,1-2-200");
}

#[test]
fn test_gtid_set_is_subset() {
    let master = GtidSet::from_str("0-1-100, 1-2-200").unwrap();
    let replica1 = GtidSet::from_str("0-1-100, 1-2-200").unwrap();
    let replica2 = GtidSet::from_str("0-1-50, 1-2-200").unwrap();
    let replica3 = GtidSet::from_str("0-1-100").unwrap();

    assert!(replica1.is_subset_of(&master));
    assert!(replica2.is_subset_of(&master));
    assert!(replica3.is_subset_of(&master));

    let ahead_replica = GtidSet::from_str("0-1-150").unwrap();
    assert!(!ahead_replica.is_subset_of(&master));

    let unknown_server = GtidSet::from_str("2-3-10").unwrap();
    assert!(!unknown_server.is_subset_of(&master));
}

#[test]
fn test_gtid_set_calculate_gap() {
    let master = GtidSet::from_str("0-1-100, 1-2-200").unwrap();
    let replica = GtidSet::from_str("0-1-80").unwrap(); // missing 1-2 entirely

    let gap = master.calculate_gap(&replica);
    assert_eq!(gap.len(), 2);
    assert_eq!(gap.get(&(0, 1)), Some(&20));
    assert_eq!(gap.get(&(1, 2)), Some(&200));

    let total_lag = master.calculate_total_lag(&replica);
    assert_eq!(total_lag, 220);
}

#[test]
fn test_master_failure_detector_state_escalation() {
    // Note: since we can't easily mock Instant::now() without external crates or unsafe code,
    // we use a small actual delay, or test logical flow.
    // Given the constraints of a unit test, we can use std::thread::sleep for small durations
    // to verify the logic.
    let mut detector = MasterFailureDetector::new(
        Duration::from_millis(10),
        Duration::from_millis(20),
        Duration::from_millis(30),
    );

    assert_eq!(detector.state(), DetectorState::Healthy);

    std::thread::sleep(Duration::from_millis(15));
    assert_eq!(detector.update_state(), DetectorState::Degraded);

    std::thread::sleep(Duration::from_millis(10)); // Total 25ms
    assert_eq!(detector.update_state(), DetectorState::Unreachable);

    std::thread::sleep(Duration::from_millis(10)); // Total 35ms
    assert_eq!(detector.update_state(), DetectorState::Quarantined);

    // Heartbeat should recover state
    detector.record_heartbeat();
    assert_eq!(detector.update_state(), DetectorState::Healthy);
}

#[test]
fn test_master_desynchronization() {
    let master = GtidSet::from_str("0-1-100").unwrap();
    let replica = GtidSet::from_str("0-1-90").unwrap();

    // Gap is 10.
    assert!(MasterFailureDetector::check_desynchronization(&master, &replica, 5));
    assert!(!MasterFailureDetector::check_desynchronization(&master, &replica, 15));
}
