#![forbid(unsafe_code)]

use databases::postgres_replication_slot::{
    ReplicationSlotMonitor, ReplicationSlot, LagSeverity, RecommendedAction, MonitorError
};

#[test]
fn test_parse_lsn_valid() {
    // 0/16B3748
    let lsn = ReplicationSlotMonitor::parse_lsn("0/16B3748").unwrap();
    assert_eq!(lsn, 0x16B3748);

    // 1/16B38F0
    let lsn2 = ReplicationSlotMonitor::parse_lsn("1/16B38F0").unwrap();
    assert_eq!(lsn2, 0x100000000 + 0x16B38F0);
}

#[test]
fn test_parse_lsn_invalid() {
    assert_eq!(ReplicationSlotMonitor::parse_lsn("invalid").unwrap_err(), MonitorError::InvalidLsnFormat);
    assert_eq!(ReplicationSlotMonitor::parse_lsn("0/16B3748/Extra").unwrap_err(), MonitorError::InvalidLsnFormat);

    match ReplicationSlotMonitor::parse_lsn("ZZZ/16B3748").unwrap_err() {
        MonitorError::LsnParseError(_) => (),
        _ => panic!("Expected LsnParseError"),
    }
}

#[test]
fn test_calculate_lag() {
    // Diff between 0/16B38F0 and 0/16B3748
    let lag = ReplicationSlotMonitor::calculate_lag("0/16B38F0", "0/16B3748").unwrap();
    assert_eq!(lag, 0x16B38F0 - 0x16B3748);

    // No lag if current is equal to slot
    let lag = ReplicationSlotMonitor::calculate_lag("0/16B3748", "0/16B3748").unwrap();
    assert_eq!(lag, 0);

    // current is behind slot (should return 0)
    let lag = ReplicationSlotMonitor::calculate_lag("0/16B3748", "0/16B38F0").unwrap();
    assert_eq!(lag, 0);
}

#[test]
fn test_evaluate_slot_physical_normal() {
    let monitor = ReplicationSlotMonitor::new(1000, 5000);
    let slot = ReplicationSlot {
        slot_name: "phys_slot".to_string(),
        plugin: None,
        slot_type: "physical".to_string(),
        active: true,
        restart_lsn: Some("0/16B3748".to_string()),
        confirmed_flush_lsn: None,
    };

    // current = 0/16B3748 + 500 (less than 1000)
    let current_lsn = format!("0/{:X}", 0x16B3748 + 500);
    let eval = monitor.evaluate_slot(&current_lsn, &slot).unwrap();

    assert_eq!(eval.lag_bytes, Some(500));
    assert_eq!(eval.severity, LagSeverity::Normal);
    assert_eq!(eval.action, RecommendedAction::None);
}

#[test]
fn test_evaluate_slot_logical_warning() {
    let monitor = ReplicationSlotMonitor::new(1000, 5000);
    let slot = ReplicationSlot {
        slot_name: "log_slot".to_string(),
        plugin: Some("pgoutput".to_string()),
        slot_type: "logical".to_string(),
        active: true,
        restart_lsn: Some("0/16B3748".to_string()),
        confirmed_flush_lsn: Some("0/16B3748".to_string()),
    };

    // current = 0/16B3748 + 2000 (between 1000 and 5000)
    let current_lsn = format!("0/{:X}", 0x16B3748 + 2000);
    let eval = monitor.evaluate_slot(&current_lsn, &slot).unwrap();

    assert_eq!(eval.lag_bytes, Some(2000));
    assert_eq!(eval.severity, LagSeverity::Warning);
    assert_eq!(eval.action, RecommendedAction::None); // active so no drop
}

#[test]
fn test_evaluate_slot_logical_critical_active() {
    let monitor = ReplicationSlotMonitor::new(1000, 5000);
    let slot = ReplicationSlot {
        slot_name: "log_slot".to_string(),
        plugin: Some("pgoutput".to_string()),
        slot_type: "logical".to_string(),
        active: true, // active
        restart_lsn: Some("0/16B3748".to_string()),
        confirmed_flush_lsn: Some("0/16B3748".to_string()),
    };

    // current = 0/16B3748 + 6000 (> 5000)
    let current_lsn = format!("0/{:X}", 0x16B3748 + 6000);
    let eval = monitor.evaluate_slot(&current_lsn, &slot).unwrap();

    assert_eq!(eval.lag_bytes, Some(6000));
    assert_eq!(eval.severity, LagSeverity::Critical);
    assert_eq!(eval.action, RecommendedAction::Pause); // recommend pause since it's active and critical
}

#[test]
fn test_evaluate_slot_physical_critical_inactive() {
    let monitor = ReplicationSlotMonitor::new(1000, 5000);
    let slot = ReplicationSlot {
        slot_name: "phys_slot".to_string(),
        plugin: None,
        slot_type: "physical".to_string(),
        active: false, // inactive
        restart_lsn: Some("0/16B3748".to_string()),
        confirmed_flush_lsn: None,
    };

    // current = 0/16B3748 + 6000 (> 5000)
    let current_lsn = format!("0/{:X}", 0x16B3748 + 6000);
    let eval = monitor.evaluate_slot(&current_lsn, &slot).unwrap();

    assert_eq!(eval.lag_bytes, Some(6000));
    assert_eq!(eval.severity, LagSeverity::Critical);
    assert_eq!(eval.action, RecommendedAction::ActiveReplicaReconnect); // recommend reconnect for inactive physical
}

#[test]
fn test_evaluate_slot_logical_critical_inactive() {
    let monitor = ReplicationSlotMonitor::new(1000, 5000);
    let slot = ReplicationSlot {
        slot_name: "log_slot".to_string(),
        plugin: Some("pgoutput".to_string()),
        slot_type: "logical".to_string(),
        active: false, // inactive
        restart_lsn: Some("0/16B3748".to_string()),
        confirmed_flush_lsn: Some("0/16B3748".to_string()),
    };

    // current = 0/16B3748 + 6000 (> 5000)
    let current_lsn = format!("0/{:X}", 0x16B3748 + 6000);
    let eval = monitor.evaluate_slot(&current_lsn, &slot).unwrap();

    assert_eq!(eval.lag_bytes, Some(6000));
    assert_eq!(eval.severity, LagSeverity::Critical);
    assert_eq!(eval.action, RecommendedAction::SafeDrop); // recommend safe drop for inactive logical
}

#[test]
fn test_evaluate_slot_inactive_normal() {
    let monitor = ReplicationSlotMonitor::new(1000, 5000);
    let slot = ReplicationSlot {
        slot_name: "inactive_slot".to_string(),
        plugin: None,
        slot_type: "physical".to_string(),
        active: false, // inactive
        restart_lsn: Some("0/16B3748".to_string()),
        confirmed_flush_lsn: None,
    };

    // current = 0/16B3748 + 500 (< 1000)
    let current_lsn = format!("0/{:X}", 0x16B3748 + 500);
    let eval = monitor.evaluate_slot(&current_lsn, &slot).unwrap();

    assert_eq!(eval.lag_bytes, Some(500));
    assert_eq!(eval.severity, LagSeverity::Normal);
    assert_eq!(eval.action, RecommendedAction::SafeDrop); // recommend safe drop for inactive with normal severity
}

#[test]
fn test_evaluate_slot_no_lsn() {
    let monitor = ReplicationSlotMonitor::new(1000, 5000);
    let slot = ReplicationSlot {
        slot_name: "no_lsn_slot".to_string(),
        plugin: None,
        slot_type: "physical".to_string(),
        active: false, // inactive
        restart_lsn: None,
        confirmed_flush_lsn: None,
    };

    let eval = monitor.evaluate_slot("0/16B3748", &slot).unwrap();

    assert_eq!(eval.lag_bytes, None);
    assert_eq!(eval.severity, LagSeverity::Normal);
    assert_eq!(eval.action, RecommendedAction::SafeDrop);
}
