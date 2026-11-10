#![forbid(unsafe_code)]

use databases::postgres_replication_lag::{
    generate_alter_role_connection_limit, is_valid_identifier, LagSeverity, Lsn, PgStatReplication,
    ReplicationState, ReplicationThresholds,
};
use std::time::Duration;

#[test]
fn test_lsn_parsing_and_diff() {
    let lsn1 = Lsn::from_str("16/B374D848").unwrap();
    let lsn2 = Lsn::from_str("16/B374D800").unwrap();

    assert_eq!(lsn1.0, 97500059720);
    assert_eq!(lsn2.0, 97500059648);
    assert_eq!(lsn1.diff(&lsn2), 72);
}

#[test]
fn test_lsn_invalid() {
    assert!(Lsn::from_str("invalid").is_none());
    assert!(Lsn::from_str("16/INVALID").is_none());
    assert!(Lsn::from_str("16/B374D848/EXTRA").is_none());
}

#[test]
fn test_byte_lag() {
    let stat = PgStatReplication {
        application_name: "test".to_string(),
        client_addr: "127.0.0.1".to_string(),
        state: ReplicationState::Streaming,
        sent_lsn: Lsn(1000),
        write_lsn: Lsn(900),
        flush_lsn: Lsn(800),
        replay_lsn: Lsn(500),
        write_lag: None,
        flush_lag: None,
        replay_lag: None,
    };

    assert_eq!(stat.byte_lag(), 500);
}

#[test]
fn test_evaluate_lag() {
    let thresholds = ReplicationThresholds {
        warning_bytes: 1000,
        critical_bytes: 5000,
        warning_time: Duration::from_secs(10),
        critical_time: Duration::from_secs(60),
    };

    let mut stat = PgStatReplication {
        application_name: "test".to_string(),
        client_addr: "127.0.0.1".to_string(),
        state: ReplicationState::Streaming,
        sent_lsn: Lsn(0),
        write_lsn: Lsn(0),
        flush_lsn: Lsn(0),
        replay_lsn: Lsn(0),
        write_lag: None,
        flush_lag: None,
        replay_lag: None,
    };

    // Ok state
    assert_eq!(stat.evaluate_lag(&thresholds), LagSeverity::Ok);

    // Warning bytes
    stat.sent_lsn = Lsn(1500);
    assert_eq!(stat.evaluate_lag(&thresholds), LagSeverity::Warning);

    // Critical bytes
    stat.sent_lsn = Lsn(6000);
    assert_eq!(stat.evaluate_lag(&thresholds), LagSeverity::Critical);

    // Warning time
    stat.sent_lsn = Lsn(0);
    stat.replay_lag = Some(Duration::from_secs(15));
    assert_eq!(stat.evaluate_lag(&thresholds), LagSeverity::Warning);

    // Critical time
    stat.replay_lag = Some(Duration::from_secs(70));
    assert_eq!(stat.evaluate_lag(&thresholds), LagSeverity::Critical);
}

#[test]
fn test_valid_identifier() {
    assert!(is_valid_identifier("valid_role"));
    assert!(is_valid_identifier("user123"));
    assert!(is_valid_identifier("_hidden"));

    assert!(!is_valid_identifier("")); // Empty
    assert!(!is_valid_identifier("invalid role")); // Space
    assert!(!is_valid_identifier("invalid-role")); // Hyphen
    assert!(!is_valid_identifier("invalid;role")); // Semicolon
    assert!(!is_valid_identifier("invalid'role")); // Quote
    assert!(!is_valid_identifier(&"a".repeat(64))); // Too long
}

#[test]
fn test_generate_alter_role_connection_limit() {
    assert_eq!(
        generate_alter_role_connection_limit("app_user", 100).unwrap(),
        "ALTER ROLE app_user CONNECTION LIMIT 100;"
    );

    assert_eq!(
        generate_alter_role_connection_limit("admin", -1).unwrap(),
        "ALTER ROLE admin CONNECTION LIMIT -1;"
    );

    assert!(generate_alter_role_connection_limit("bad; user", 100).is_err());
}
