#![forbid(unsafe_code)]

use databases::slow_query_analyzer::{analyze_slow_query, parse_slow_query_logs, SlowQueryLogEntry};

#[test]
fn test_parse_slow_query_logs() {
    let log_data = "\
# Time: 2023-10-10T12:00:00.000000Z
# User@Host: root[root] @ localhost []
# Query_time: 1.500000  Lock_time: 0.000100 Rows_sent: 5  Rows_examined: 5000
use mydb;
SET timestamp=1696939200;
SELECT * FROM users WHERE status = 'active';
# Time: 2023-10-10T12:05:00.000000Z
# User@Host: root[root] @ localhost []
# Query_time: 0.100000  Lock_time: 0.000000 Rows_sent: 1  Rows_examined: 1
SELECT 1;";

    let entries = parse_slow_query_logs(log_data);

    assert_eq!(entries.len(), 2);

    assert_eq!(entries[0].query_time, 1.5);
    assert_eq!(entries[0].lock_time, 0.0001);
    assert_eq!(entries[0].rows_sent, 5);
    assert_eq!(entries[0].rows_examined, 5000);
    assert_eq!(entries[0].query, "SELECT * FROM users WHERE status = 'active';");

    assert_eq!(entries[1].query_time, 0.1);
    assert_eq!(entries[1].lock_time, 0.0);
    assert_eq!(entries[1].rows_sent, 1);
    assert_eq!(entries[1].rows_examined, 1);
    assert_eq!(entries[1].query, "SELECT 1;");
}

#[test]
fn test_analyze_slow_query_high_anomaly() {
    let entry = SlowQueryLogEntry {
        query_time: 2.0,
        lock_time: 0.0,
        rows_sent: 10,
        rows_examined: 2000,
        query: "SELECT * FROM orders WHERE user_id = 1;".to_string(),
    };

    let anomaly_score = analyze_slow_query(&entry);

    assert_eq!(anomaly_score.examined_to_sent_ratio, 200.0);
    assert!(anomaly_score.score > 0.0);
    assert!(anomaly_score.index_recommended);
}

#[test]
fn test_analyze_slow_query_low_anomaly() {
    let entry = SlowQueryLogEntry {
        query_time: 0.1,
        lock_time: 0.0,
        rows_sent: 1,
        rows_examined: 1,
        query: "SELECT 1;".to_string(),
    };

    let anomaly_score = analyze_slow_query(&entry);

    assert_eq!(anomaly_score.examined_to_sent_ratio, 1.0);
    assert!(anomaly_score.score > 0.0);
    assert!(!anomaly_score.index_recommended);
}

#[test]
fn test_analyze_slow_query_zero_rows_sent() {
    let entry = SlowQueryLogEntry {
        query_time: 1.0,
        lock_time: 0.0,
        rows_sent: 0,
        rows_examined: 1500,
        query: "UPDATE products SET price = 100 WHERE category = 'books';".to_string(),
    };

    let anomaly_score = analyze_slow_query(&entry);

    assert_eq!(anomaly_score.examined_to_sent_ratio, 1500.0);
    assert!(anomaly_score.score > 0.0);
    assert!(anomaly_score.index_recommended);
}

#[test]
fn test_analyze_slow_query_zero_rows_examined() {
    let entry = SlowQueryLogEntry {
        query_time: 0.01,
        lock_time: 0.0,
        rows_sent: 1,
        rows_examined: 0,
        query: "SELECT NOW();".to_string(),
    };

    let anomaly_score = analyze_slow_query(&entry);

    assert_eq!(anomaly_score.examined_to_sent_ratio, 0.0);
    assert_eq!(anomaly_score.score, 0.0);
    assert!(!anomaly_score.index_recommended);
}
