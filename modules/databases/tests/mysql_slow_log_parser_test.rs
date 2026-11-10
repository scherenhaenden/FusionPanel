#![forbid(unsafe_code)]

use databases::mysql_slow_log_parser::SlowQueryLogParser;

#[test]
fn test_parse_multi_line_slow_query() {
    let log_data = r#"
# Time: 2023-10-10T12:00:00Z
# User@Host: root[root] @ localhost []  Id:     1
# Query_time: 1.200  Lock_time: 0.050 Rows_sent: 5  Rows_examined: 1000
SET timestamp=1696939200;
SELECT * FROM users
WHERE id = 5;
"#;

    let parser = SlowQueryLogParser::new();
    let entries = parser.parse(log_data).unwrap();

    assert_eq!(entries.len(), 1);
    let entry = &entries[0];
    assert_eq!(entry.timestamp.as_deref(), Some("1696939200"));
    assert_eq!(entry.user_host, "root[root] @ localhost []  Id:     1");
    assert_eq!(entry.query_time, 1.2);
    assert_eq!(entry.lock_time, 0.05);
    assert_eq!(entry.rows_sent, 5);
    assert_eq!(entry.rows_examined, 1000);
    assert_eq!(entry.query, "SELECT * FROM users\nWHERE id = 5;");
}

#[test]
fn test_query_fingerprinting() {
    // Numeric literals
    assert_eq!(SlowQueryLogParser::fingerprint("SELECT * FROM users WHERE id = 123"), "SELECT * FROM users WHERE id = ?");
    assert_eq!(SlowQueryLogParser::fingerprint("SELECT * FROM users WHERE id = -123"), "SELECT * FROM users WHERE id = ?");
    assert_eq!(SlowQueryLogParser::fingerprint("SELECT * FROM users WHERE id = 123.45"), "SELECT * FROM users WHERE id = ?");

    // String literals
    assert_eq!(SlowQueryLogParser::fingerprint("SELECT * FROM users WHERE name = 'John'"), "SELECT * FROM users WHERE name = ?");
    assert_eq!(SlowQueryLogParser::fingerprint("SELECT * FROM users WHERE name = \"John\""), "SELECT * FROM users WHERE name = ?");
    assert_eq!(SlowQueryLogParser::fingerprint("SELECT * FROM users WHERE name = 'John\\'s'"), "SELECT * FROM users WHERE name = ?");

    // IN clauses
    assert_eq!(SlowQueryLogParser::fingerprint("SELECT * FROM users WHERE id IN (1, 2, 3)"), "SELECT * FROM users WHERE id IN (?)");
    assert_eq!(SlowQueryLogParser::fingerprint("SELECT * FROM users WHERE id in (1, 2, 3)"), "SELECT * FROM users WHERE id IN (?)");
    assert_eq!(SlowQueryLogParser::fingerprint("SELECT * FROM users WHERE id IN( 'a', 'b', 'c' )"), "SELECT * FROM users WHERE id IN (?)");

    // Normalization of whitespace
    assert_eq!(SlowQueryLogParser::fingerprint("SELECT    *   FROM   users\nWHERE id = 1"), "SELECT * FROM users WHERE id = ?");
}

#[test]
fn test_aggregation_and_top_n() {
    let log_data = r#"
# Time: 2023-10-10T12:00:00Z
# User@Host: root[root] @ localhost []
# Query_time: 2.0  Lock_time: 0.0 Rows_sent: 0  Rows_examined: 0
SELECT * FROM users WHERE id = 1;
# Time: 2023-10-10T12:01:00Z
# User@Host: root[root] @ localhost []
# Query_time: 3.0  Lock_time: 0.0 Rows_sent: 0  Rows_examined: 0
SELECT * FROM users WHERE id = 2;
# Time: 2023-10-10T12:02:00Z
# User@Host: root[root] @ localhost []
# Query_time: 0.5  Lock_time: 0.0 Rows_sent: 0  Rows_examined: 0
SELECT * FROM posts WHERE user_id = 1;
"#;

    let parser = SlowQueryLogParser::new();
    let entries = parser.parse(log_data).unwrap();
    let stats = parser.aggregate(&entries);

    assert_eq!(stats.len(), 2);

    // Ordered by total time descending
    assert_eq!(stats[0].fingerprint, "SELECT * FROM users WHERE id = ?;");
    assert_eq!(stats[0].total_calls, 2);
    assert_eq!(stats[0].total_time, 5.0);

    assert_eq!(stats[1].fingerprint, "SELECT * FROM posts WHERE user_id = ?;");
    assert_eq!(stats[1].total_calls, 1);
    assert_eq!(stats[1].total_time, 0.5);

    // P95 latency
    assert_eq!(stats[0].p95_latency(), 3.0);
}

#[test]
fn test_parse_multiple_entries() {
    let log_data = r#"
# Time: 2023-01-01T00:00:00.000000Z
# User@Host: db_user[db_user] @  [10.0.0.1]  Id: 100
# Query_time: 5.123456  Lock_time: 0.000000 Rows_sent: 1  Rows_examined: 100000
use my_db;
SET timestamp=1672531200;
SELECT COUNT(*) FROM very_large_table WHERE status = 'active';

# Time: 2023-01-01T00:05:00.000000Z
# User@Host: another_user[another_user] @ localhost []  Id: 101
# Query_time: 0.500000  Lock_time: 0.010000 Rows_sent: 10  Rows_examined: 50
UPDATE smaller_table SET updated_at = '2023-01-01 00:05:00' WHERE id IN (1, 2, 3);
"#;

    let parser = SlowQueryLogParser::new();
    let entries = parser.parse(log_data).unwrap();

    assert_eq!(entries.len(), 2);

    let e1 = &entries[0];
    assert_eq!(e1.timestamp.as_deref(), Some("1672531200"));
    assert_eq!(e1.user_host, "db_user[db_user] @  [10.0.0.1]  Id: 100");
    assert_eq!(e1.query_time, 5.123456);
    assert_eq!(e1.rows_examined, 100000);
    assert_eq!(e1.query, "SELECT COUNT(*) FROM very_large_table WHERE status = 'active';");

    let e2 = &entries[1];
    assert_eq!(e2.timestamp.as_deref(), Some("2023-01-01T00:05:00.000000Z")); // No SET timestamp, fallback to # Time
    assert_eq!(e2.user_host, "another_user[another_user] @ localhost []  Id: 101");
    assert_eq!(e2.query_time, 0.5);
    assert_eq!(e2.query, "UPDATE smaller_table SET updated_at = '2023-01-01 00:05:00' WHERE id IN (1, 2, 3);");

    let stats = parser.aggregate(&entries);
    assert_eq!(stats.len(), 2);
    assert_eq!(stats[0].fingerprint, "SELECT COUNT(*) FROM very_large_table WHERE status = ?;");
    assert_eq!(stats[1].fingerprint, "UPDATE smaller_table SET updated_at = ? WHERE id IN (?);");
}

#[test]
fn test_query_stats_p95_latency() {
    let mut stats = databases::mysql_slow_log_parser::QueryStats::new("SELECT * FROM tbl".to_string());
    // Add 100 latencies from 1.0 to 100.0
    for i in 1..=100 {
        stats.add_call(i as f64);
    }

    let p95 = stats.p95_latency();
    assert_eq!(p95, 95.0);
}
