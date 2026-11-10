#![forbid(unsafe_code)]

use databases::binlog_retention::{BinlogFile, BinlogRetentionCalculator, RetentionRules};

#[test]
fn test_minimum_kept_logs_guarantee() {
    let logs = vec![BinlogFile {
        filename: "binlog.000001".to_string(),
        size_bytes: 1000,
        timestamp_seconds: 100,
    }];

    let rules = RetentionRules {
        max_total_size_bytes: Some(10), // normally would purge
        max_age_seconds: Some(1),       // normally would purge
        minimum_retained_logs: 0,       // calculator enforces max(1, 0)
    };

    let plan = BinlogRetentionCalculator::calculate(logs, &rules, 1000);
    assert!(plan.is_none());
}

#[test]
fn test_size_based_purge() {
    let logs = vec![
        BinlogFile {
            filename: "binlog.000001".to_string(),
            size_bytes: 100,
            timestamp_seconds: 100,
        },
        BinlogFile {
            filename: "binlog.000002".to_string(),
            size_bytes: 100,
            timestamp_seconds: 200,
        },
        BinlogFile {
            filename: "binlog.000003".to_string(),
            size_bytes: 100,
            timestamp_seconds: 300,
        },
    ];

    // we keep logs starting from newest until size exceeds max_total_size_bytes
    let rules = RetentionRules {
        max_total_size_bytes: Some(150),
        max_age_seconds: None,
        minimum_retained_logs: 1,
    };

    let plan = BinlogRetentionCalculator::calculate(logs, &rules, 400);
    assert!(plan.is_some());
    let plan = plan.unwrap();
    // newest log is binlog.000003 (size 100).
    // if we try to keep binlog.000002 (size 100), accumulated size is 200 > 150.
    // so we only keep binlog.000003.
    // Cutoff should be binlog.000003 (purging 1 and 2).
    assert_eq!(plan.cutoff_filename, "binlog.000003");
    assert_eq!(plan.sql_command, "PURGE BINARY LOGS TO 'binlog.000003';");
}

#[test]
fn test_age_based_purge() {
    let logs = vec![
        BinlogFile {
            filename: "binlog.000001".to_string(),
            size_bytes: 10,
            timestamp_seconds: 100,
        },
        BinlogFile {
            filename: "binlog.000002".to_string(),
            size_bytes: 10,
            timestamp_seconds: 200,
        },
        BinlogFile {
            filename: "binlog.000003".to_string(),
            size_bytes: 10,
            timestamp_seconds: 300,
        },
    ];

    // we keep logs starting from newest.
    // current time = 400.
    // log.000003 age = 100
    // log.000002 age = 200
    // log.000001 age = 300
    let rules = RetentionRules {
        max_total_size_bytes: None,
        max_age_seconds: Some(150),
        minimum_retained_logs: 1,
    };

    let plan = BinlogRetentionCalculator::calculate(logs, &rules, 400);
    assert!(plan.is_some());
    let plan = plan.unwrap();
    // keeps log.000003 (age 100 <= 150).
    // log.000002 (age 200 > 150) -> stop keeping.
    // purge 1 and 2, cutoff is 3.
    assert_eq!(plan.cutoff_filename, "binlog.000003");
}

#[test]
fn test_minimum_kept_logs() {
    let logs = vec![
        BinlogFile {
            filename: "binlog.000001".to_string(),
            size_bytes: 10,
            timestamp_seconds: 100,
        },
        BinlogFile {
            filename: "binlog.000002".to_string(),
            size_bytes: 10,
            timestamp_seconds: 200,
        },
        BinlogFile {
            filename: "binlog.000003".to_string(),
            size_bytes: 10,
            timestamp_seconds: 300,
        },
    ];

    let rules = RetentionRules {
        max_total_size_bytes: Some(5), // normally keeps none, but min keeps kicks in
        max_age_seconds: Some(5),      // normally keeps none, but min keeps kicks in
        minimum_retained_logs: 2,
    };

    let plan = BinlogRetentionCalculator::calculate(logs, &rules, 1000);
    assert!(plan.is_some());
    let plan = plan.unwrap();
    // keeps 2 logs (000003, 000002).
    // purge 1 log (000001). cutoff is 000002.
    assert_eq!(plan.cutoff_filename, "binlog.000002");
}

#[test]
fn test_no_purge_needed() {
    let logs = vec![
        BinlogFile {
            filename: "binlog.000001".to_string(),
            size_bytes: 10,
            timestamp_seconds: 100,
        },
        BinlogFile {
            filename: "binlog.000002".to_string(),
            size_bytes: 10,
            timestamp_seconds: 200,
        },
    ];

    let rules = RetentionRules {
        max_total_size_bytes: Some(100),
        max_age_seconds: Some(1000),
        minimum_retained_logs: 1,
    };

    let plan = BinlogRetentionCalculator::calculate(logs, &rules, 200);
    assert!(plan.is_none());
}

#[test]
fn test_empty_logs() {
    let rules = RetentionRules {
        max_total_size_bytes: Some(100),
        max_age_seconds: Some(1000),
        minimum_retained_logs: 1,
    };
    let plan = BinlogRetentionCalculator::calculate(vec![], &rules, 200);
    assert!(plan.is_none());
}
