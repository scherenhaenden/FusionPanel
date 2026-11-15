#![forbid(unsafe_code)]

use databases::postgres_replication_slots::{
    ReplicationSlot, ReplicationSlotMonitor, SlotError, SlotHealth,
};

#[test]
fn test_evaluate_health_healthy() {
    let slot = ReplicationSlot {
        slot_name: "test_slot".to_string(),
        plugin: Some("pgoutput".to_string()),
        slot_type: "logical".to_string(),
        active: true,
        wal_status: "reserved".to_string(),
        safe_wal_size: None,
    };

    assert_eq!(
        ReplicationSlotMonitor::evaluate_health(&slot),
        SlotHealth::Healthy
    );
}

#[test]
fn test_evaluate_health_inactive() {
    let slot = ReplicationSlot {
        slot_name: "test_slot".to_string(),
        plugin: Some("pgoutput".to_string()),
        slot_type: "logical".to_string(),
        active: false,
        wal_status: "reserved".to_string(),
        safe_wal_size: None,
    };

    assert_eq!(
        ReplicationSlotMonitor::evaluate_health(&slot),
        SlotHealth::Warning("Slot 'test_slot' is inactive".to_string())
    );
}

#[test]
fn test_evaluate_health_extended() {
    let slot = ReplicationSlot {
        slot_name: "test_slot".to_string(),
        plugin: Some("pgoutput".to_string()),
        slot_type: "logical".to_string(),
        active: true,
        wal_status: "extended".to_string(),
        safe_wal_size: None,
    };

    assert_eq!(
        ReplicationSlotMonitor::evaluate_health(&slot),
        SlotHealth::Warning("Slot 'test_slot' has extended WAL status".to_string())
    );
}

#[test]
fn test_evaluate_health_unreserved() {
    let slot = ReplicationSlot {
        slot_name: "test_slot".to_string(),
        plugin: Some("pgoutput".to_string()),
        slot_type: "logical".to_string(),
        active: true,
        wal_status: "unreserved".to_string(),
        safe_wal_size: None,
    };

    assert_eq!(
        ReplicationSlotMonitor::evaluate_health(&slot),
        SlotHealth::Warning("Slot 'test_slot' has unreserved WAL status".to_string())
    );
}

#[test]
fn test_evaluate_health_lost() {
    let slot = ReplicationSlot {
        slot_name: "test_slot".to_string(),
        plugin: Some("pgoutput".to_string()),
        slot_type: "logical".to_string(),
        active: true,
        wal_status: "lost".to_string(),
        safe_wal_size: None,
    };

    assert_eq!(
        ReplicationSlotMonitor::evaluate_health(&slot),
        SlotHealth::Critical("Slot 'test_slot' has lost WAL status".to_string())
    );
}

#[test]
fn test_format_drop_slot_valid() {
    let sql = ReplicationSlotMonitor::format_drop_slot("valid_slot_name_123").unwrap();
    assert_eq!(
        sql,
        "SELECT pg_drop_replication_slot('valid_slot_name_123');"
    );
}

#[test]
fn test_format_drop_slot_invalid_chars() {
    let err = ReplicationSlotMonitor::format_drop_slot("invalid-slot-name").unwrap_err();
    assert_eq!(err, SlotError::InvalidSlotName);

    let err2 =
        ReplicationSlotMonitor::format_drop_slot("injection'; DROP TABLE slots; --").unwrap_err();
    assert_eq!(err2, SlotError::InvalidSlotName);
}

#[test]
fn test_format_drop_slot_empty() {
    let err = ReplicationSlotMonitor::format_drop_slot("").unwrap_err();
    assert_eq!(err, SlotError::InvalidSlotName);
}

#[test]
fn test_format_drop_slot_too_long() {
    let long_name = "a".repeat(64);
    let err = ReplicationSlotMonitor::format_drop_slot(&long_name).unwrap_err();
    assert_eq!(err, SlotError::InvalidSlotName);
}
