#![forbid(unsafe_code)]

use databases::postgres_replication::{
    Lsn, LsnParseError, ReplicationMonitor, WalArchiveValidator,
};
use std::str::FromStr;

#[test]
fn test_lsn_parsing() {
    let lsn = Lsn::from_str("16/B374D848").unwrap();
    assert_eq!(lsn.value(), 97500059720); // 0x16B374D848

    let lsn2 = Lsn::from_str("0/15000000").unwrap();
    assert_eq!(lsn2.value(), 352321536); // 0x15000000

    let invalid = Lsn::from_str("invalid");
    assert!(matches!(invalid, Err(LsnParseError::InvalidFormat)));

    let invalid_hex = Lsn::from_str("16/XX");
    assert!(matches!(invalid_hex, Err(LsnParseError::Parse(_))));
}

#[test]
fn test_lsn_diff() {
    let lsn1 = Lsn::from_str("16/B374D848").unwrap();
    let lsn2 = Lsn::from_str("16/B374D900").unwrap();

    assert_eq!(lsn1.diff(&lsn2), 184); // 0x16B374D900 - 0x16B374D848 = 0xB8 = 184
    assert_eq!(lsn2.diff(&lsn1), 184);
}

#[test]
fn test_parse_pg_stat_replication_row() {
    let row = ReplicationMonitor::parse_pg_stat_replication_row(
        "10.0.0.5",
        "streaming",
        "16/B374D900",
        "16/B374D848",
        "16/B374D848",
        "16/B374D848",
    ).unwrap();

    assert_eq!(row.client_addr, "10.0.0.5");
    assert_eq!(row.state, "streaming");
    assert_eq!(row.sent_lsn.value(), Lsn::from_str("16/B374D900").unwrap().value());
    assert_eq!(row.write_lsn.value(), Lsn::from_str("16/B374D848").unwrap().value());
    assert_eq!(row.flush_lsn.value(), Lsn::from_str("16/B374D848").unwrap().value());
    assert_eq!(row.replay_lsn.value(), Lsn::from_str("16/B374D848").unwrap().value());

    // lag is sent_lsn - replay_lsn = 0x16B374D900 - 0x16B374D848 = 184 bytes
    assert_eq!(row.byte_lag(), 184);
}

#[test]
fn test_byte_lag_alert_thresholds() {
    // Zero mocks, direct calculation checks
    let row_healthy = ReplicationMonitor::parse_pg_stat_replication_row(
        "10.0.0.5",
        "streaming",
        "16/B374D900", // sent
        "16/B374D848", // write
        "16/B374D848", // flush
        "16/B374D848", // replay
    ).unwrap();
    let lag_bytes = row_healthy.byte_lag();
    assert!(lag_bytes < 1024 * 1024 * 10); // Less than 10MB lag is healthy

    let row_unhealthy = ReplicationMonitor::parse_pg_stat_replication_row(
        "10.0.0.5",
        "streaming",
        "17/B374D900", // sent
        "16/B374D848", // write
        "16/B374D848", // flush
        "16/B374D848", // replay
    ).unwrap();
    let large_lag = row_unhealthy.byte_lag();
    assert!(large_lag > 1024 * 1024 * 100); // Greater than 100MB lag is unhealthy
}

#[test]
fn test_wal_archive_status_parsing() {
    let status = WalArchiveValidator::parse_status(
        Some("0000000100000016000000B3"),
        "0",
        None,
    ).unwrap();
    assert_eq!(status.last_archived_wal.as_deref(), Some("0000000100000016000000B3"));
    assert_eq!(status.failed_count, 0);
    assert_eq!(status.last_failed_wal, None);
    assert_eq!(status.is_failing(), false);

    let failing_status = WalArchiveValidator::parse_status(
        Some("0000000100000016000000B3"),
        "5",
        Some("0000000100000016000000B4"),
    ).unwrap();
    assert_eq!(failing_status.failed_count, 5);
    assert_eq!(failing_status.last_failed_wal.as_deref(), Some("0000000100000016000000B4"));
    assert_eq!(failing_status.is_failing(), true);

    let empty_failed = WalArchiveValidator::parse_status(None, "   ", None).unwrap();
    assert_eq!(empty_failed.failed_count, 0);
}
