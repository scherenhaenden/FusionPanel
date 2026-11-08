#![forbid(unsafe_code)]

use chrono::{Duration, Utc};
use uuid::Uuid;
use administration::incident_manager::{
    IncidentManager, Alert, AlertFingerprint, Severity, IncidentState, EventType, IncidentError
};

#[test]
fn test_state_machine_transitions() {
    let mut manager = IncidentManager::new(Duration::minutes(5), Duration::minutes(15));
    let now = Utc::now();

    let alert = Alert {
        fingerprint: AlertFingerprint {
            source: "node-1".into(),
            resource: "cpu".into(),
            severity: Severity::Medium,
        },
        message: "High CPU usage".into(),
    };

    let id = manager.receive_alert(alert, now);
    let incident = manager.get_incident(id).unwrap();
    assert_eq!(incident.state, IncidentState::Open);

    let ack_time = now + Duration::minutes(2);
    assert!(manager.acknowledge(id, ack_time).is_ok());
    let incident = manager.get_incident(id).unwrap();
    assert_eq!(incident.state, IncidentState::Acknowledged);
    assert_eq!(incident.acknowledged_at, Some(ack_time));

    let invalid_res = manager.acknowledge(id, ack_time);
    assert!(matches!(invalid_res, Err(IncidentError::InvalidTransition)));

    let res_time = ack_time + Duration::minutes(10);
    assert!(manager.resolve(id, res_time).is_ok());
    let incident = manager.get_incident(id).unwrap();
    assert_eq!(incident.state, IncidentState::Resolved);
    assert_eq!(incident.resolved_at, Some(res_time));

    let close_time = res_time + Duration::minutes(5);
    assert!(manager.close(id, close_time).is_ok());
    let incident = manager.get_incident(id).unwrap();
    assert_eq!(incident.state, IncidentState::Closed);
    assert_eq!(incident.closed_at, Some(close_time));
}

#[test]
fn test_invalid_state_transitions() {
    let mut manager = IncidentManager::new(Duration::minutes(5), Duration::minutes(15));
    let now = Utc::now();

    let alert = Alert {
        fingerprint: AlertFingerprint {
            source: "node-2".into(),
            resource: "memory".into(),
            severity: Severity::Low,
        },
        message: "OOM".into(),
    };

    let id = manager.receive_alert(alert, now);

    assert!(matches!(manager.resolve(id, now), Err(IncidentError::InvalidTransition)));
    assert!(matches!(manager.close(id, now), Err(IncidentError::InvalidTransition)));
}

#[test]
fn test_deduplication_window() {
    let dedup_window = Duration::minutes(5);
    let mut manager = IncidentManager::new(dedup_window, Duration::minutes(15));
    let start_time = Utc::now();

    let alert1 = Alert {
        fingerprint: AlertFingerprint {
            source: "node-3".into(),
            resource: "disk".into(),
            severity: Severity::High,
        },
        message: "Disk 90% full".into(),
    };

    let id = manager.receive_alert(alert1.clone(), start_time);

    let time2 = start_time + Duration::minutes(2);
    let alert2 = Alert {
        fingerprint: alert1.fingerprint.clone(),
        message: "Disk 91% full".into(),
    };
    let id2 = manager.receive_alert(alert2, time2);
    assert_eq!(id, id2);

    let incident = manager.get_incident(id).unwrap();
    assert_eq!(incident.duplicate_count, 1);
    assert_eq!(incident.timeline.len(), 1); 
    assert_eq!(incident.latest_alert_message, "Disk 91% full");

    let time3 = start_time + Duration::minutes(10);
    let alert3 = Alert {
        fingerprint: alert1.fingerprint.clone(),
        message: "Disk 95% full".into(),
    };
    let id3 = manager.receive_alert(alert3.clone(), time3);
    assert_eq!(id, id3);

    let incident = manager.get_incident(id).unwrap();
    assert_eq!(incident.duplicate_count, 1); 
    assert_eq!(incident.timeline.len(), 2); 
    assert!(matches!(incident.timeline[1].event_type, EventType::AlertRepeated(_)));
    assert_eq!(incident.latest_alert_message, "Disk 95% full");
}

#[test]
fn test_severity_escalation() {
    let mut manager = IncidentManager::new(Duration::minutes(5), Duration::minutes(15));
    let start_time = Utc::now();

    let alert = Alert {
        fingerprint: AlertFingerprint {
            source: "db-1".into(),
            resource: "connection".into(),
            severity: Severity::Medium,
        },
        message: "DB connection timeout".into(),
    };

    let id = manager.receive_alert(alert, start_time);
    
    manager.process_escalations(start_time + Duration::minutes(10));
    let incident = manager.get_incident(id).unwrap();
    assert_eq!(incident.current_severity, Severity::Medium);

    manager.process_escalations(start_time + Duration::minutes(20));
    let incident = manager.get_incident(id).unwrap();
    assert_eq!(incident.current_severity, Severity::High);

    manager.process_escalations(start_time + Duration::minutes(40));
    let incident = manager.get_incident(id).unwrap();
    assert_eq!(incident.current_severity, Severity::Critical);

    manager.process_escalations(start_time + Duration::minutes(60));
    let incident = manager.get_incident(id).unwrap();
    assert_eq!(incident.current_severity, Severity::Critical); 

    let alert2 = Alert {
        fingerprint: AlertFingerprint {
            source: "db-1".into(),
            resource: "connection".into(),
            severity: Severity::Critical,
        },
        message: "DB dead".into(),
    };
    let id2 = manager.receive_alert(alert2, start_time + Duration::minutes(65));
    assert_eq!(id, id2);
}

#[test]
fn test_mtta_and_mttr() {
    let mut manager = IncidentManager::new(Duration::minutes(5), Duration::minutes(15));
    let start_time = Utc::now();

    let id1 = manager.receive_alert(Alert {
        fingerprint: AlertFingerprint {
            source: "s1".into(),
            resource: "r1".into(),
            severity: Severity::Low,
        },
        message: "m1".into(),
    }, start_time);

    let id2 = manager.receive_alert(Alert {
        fingerprint: AlertFingerprint {
            source: "s2".into(),
            resource: "r2".into(),
            severity: Severity::Medium,
        },
        message: "m2".into(),
    }, start_time + Duration::minutes(5));

    manager.acknowledge(id1, start_time + Duration::minutes(10)).unwrap();
    manager.acknowledge(id2, start_time + Duration::minutes(25)).unwrap(); 

    assert_eq!(manager.mtta().unwrap().num_minutes(), 15);

    manager.resolve(id1, start_time + Duration::minutes(30)).unwrap(); 
    manager.resolve(id2, start_time + Duration::minutes(45)).unwrap(); 

    assert_eq!(manager.mttr().unwrap().num_minutes(), 35);
}

#[test]
fn test_note_addition() {
    let mut manager = IncidentManager::new(Duration::minutes(5), Duration::minutes(15));
    let now = Utc::now();
    let id = manager.receive_alert(Alert {
        fingerprint: AlertFingerprint {
            source: "s".into(),
            resource: "r".into(),
            severity: Severity::Low,
        },
        message: "m".into(),
    }, now);

    assert!(manager.add_note(id, "Investigating".into(), now + Duration::minutes(1)).is_ok());
    
    let incident = manager.get_incident(id).unwrap();
    assert_eq!(incident.timeline.len(), 2);
    assert!(matches!(&incident.timeline[1].event_type, EventType::NoteAdded(n) if n == "Investigating"));
}

#[test]
fn test_non_existent_incident() {
    let mut manager = IncidentManager::new(Duration::minutes(5), Duration::minutes(15));
    let id = Uuid::new_v4();
    let now = Utc::now();
    assert!(matches!(manager.acknowledge(id, now), Err(IncidentError::NotFound)));
    assert!(matches!(manager.resolve(id, now), Err(IncidentError::NotFound)));
    assert!(matches!(manager.close(id, now), Err(IncidentError::NotFound)));
    assert!(matches!(manager.add_note(id, "Note".into(), now), Err(IncidentError::NotFound)));
}
