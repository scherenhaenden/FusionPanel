#![forbid(unsafe_code)]

use databases::slow_query::{normalize_query, QueryRanker, SlowQueryLogEntry, SlowQueryLogParser};

#[test]
fn test_slow_query_parser_basic() {
    let log_data = r#"# Time: 2023-10-01T12:00:00.000000Z
# User@Host: root[root] @ localhost [127.0.0.1]
# Query_time: 1.200000  Lock_time: 0.000000 Rows_sent: 10  Rows_examined: 100
use test_db;
SET timestamp=1696161600;
SELECT * FROM users WHERE id = 1;
"#;

    let cursor = std::io::Cursor::new(log_data);
    let mut parser = SlowQueryLogParser::new(cursor);

    let entry = parser.next_entry().unwrap().expect("Expected an entry");
    assert_eq!(
        entry.timestamp.as_deref(),
        Some("2023-10-01T12:00:00.000000Z")
    );
    assert_eq!(
        entry.user_host.as_deref(),
        Some("root[root] @ localhost [127.0.0.1]")
    );
    assert_eq!(entry.query_time, 1.2);
    assert_eq!(entry.lock_time, 0.0);
    assert_eq!(entry.rows_sent, 10);
    assert_eq!(entry.rows_examined, 100);
    assert_eq!(entry.sql, "SELECT * FROM users WHERE id = 1;");

    assert!(parser.next_entry().unwrap().is_none());
}

#[test]
fn test_slow_query_parser_multiple() {
    let log_data = r#"# Time: 2023-10-01T12:00:00.000000Z
# User@Host: root[root] @ localhost [127.0.0.1]
# Query_time: 1.500000  Lock_time: 0.100000 Rows_sent: 5  Rows_examined: 50
SELECT * FROM users WHERE id = 1;
# Time: 2023-10-01T12:01:00.000000Z
# User@Host: app[app] @  [192.168.1.10]
# Query_time: 2.000000  Lock_time: 0.200000 Rows_sent: 1  Rows_examined: 1000
SELECT COUNT(*) FROM logs WHERE created_at < '2023-01-01';
"#;

    let cursor = std::io::Cursor::new(log_data);
    let mut parser = SlowQueryLogParser::new(cursor);

    let entry1 = parser.next_entry().unwrap().expect("Expected first entry");
    assert_eq!(entry1.query_time, 1.5);
    assert_eq!(entry1.sql, "SELECT * FROM users WHERE id = 1;");

    let entry2 = parser.next_entry().unwrap().expect("Expected second entry");
    assert_eq!(entry2.query_time, 2.0);
    assert_eq!(
        entry2.sql,
        "SELECT COUNT(*) FROM logs WHERE created_at < '2023-01-01';"
    );

    assert!(parser.next_entry().unwrap().is_none());
}

#[test]
fn test_slow_query_parser_multiline_sql() {
    let log_data = r#"# Time: 2023-10-01T12:00:00.000000Z
# User@Host: root[root] @ localhost [127.0.0.1]
# Query_time: 1.000000  Lock_time: 0.000000 Rows_sent: 1  Rows_examined: 1
SELECT id, name
FROM users
WHERE id = 42;
"#;

    let cursor = std::io::Cursor::new(log_data);
    let mut parser = SlowQueryLogParser::new(cursor);

    let entry = parser.next_entry().unwrap().expect("Expected an entry");
    assert_eq!(entry.sql, "SELECT id, name\nFROM users\nWHERE id = 42;");
}

#[test]
fn test_normalize_query() {
    assert_eq!(
        normalize_query("SELECT * FROM users WHERE id = 1"),
        "select * from users where id = ?"
    );
    assert_eq!(
        normalize_query("SELECT * FROM users WHERE id = 42 AND status = 'active'"),
        "select * from users where id = ? and status = ?"
    );
    assert_eq!(
        normalize_query("SELECT * FROM items WHERE price > 10.50"),
        "select * from items where price > ?"
    );
    assert_eq!(
        normalize_query("INSERT INTO users (name, age) VALUES ('Alice', 30)"),
        "insert into users (name, age) values (?, ?)"
    );
    // Note: The simplistic normalization logic may not be perfect for all SQL dialects,
    // but should handle literals (numbers, strings) as requirements generally expect.
    // Testing the current implementation:
}

#[test]
fn test_query_ranker() {
    let mut ranker = QueryRanker::new();

    ranker.add_entry(&SlowQueryLogEntry {
        query_time: 1.5,
        lock_time: 0.1,
        rows_sent: 5,
        rows_examined: 50,
        sql: "SELECT * FROM users WHERE id = 1;".to_string(),
        ..Default::default()
    });

    ranker.add_entry(&SlowQueryLogEntry {
        query_time: 2.0,
        lock_time: 0.2,
        rows_sent: 1,
        rows_examined: 100,
        sql: "SELECT * FROM users WHERE id = 2;".to_string(),
        ..Default::default()
    });

    ranker.add_entry(&SlowQueryLogEntry {
        query_time: 5.0,
        lock_time: 0.5,
        rows_sent: 0,
        rows_examined: 1000,
        sql: "DELETE FROM logs WHERE created_at < '2023-01-01';".to_string(),
        ..Default::default()
    });

    let top = ranker.top_queries_by_time(2);

    assert_eq!(top.len(), 2);
    // The top one should be the DELETE query
    assert!(top[0].normalized_query.contains("delete"));
    assert_eq!(top[0].total_time, 5.0);
    assert_eq!(top[0].execution_count, 1);

    // The second one should be the SELECT query with 3.5 total time (1.5 + 2.0)
    assert!(top[1].normalized_query.contains("select"));
    assert_eq!(top[1].total_time, 3.5);
    assert_eq!(top[1].execution_count, 2);
    assert_eq!(top[1].average_latency(), 1.75);
}
