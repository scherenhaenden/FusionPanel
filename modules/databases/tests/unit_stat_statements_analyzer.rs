#![forbid(unsafe_code)]

use databases::stat_statements_analyzer::*;

#[test]
fn test_parse_pg_stat_statements() {
    let csv = "queryid,total_exec_time,mean_exec_time,calls,shared_blks_hit,shared_blks_read\n\
               12345,1000.5,10.005,100,500,50\n\
               67890,2000.0,20.0,100,600,60";

    let records = parse_pg_stat_statements(csv).unwrap();
    assert_eq!(records.len(), 2);

    assert_eq!(records[0].queryid, 12345);
    assert_eq!(records[0].total_exec_time, 1000.5);
    assert_eq!(records[0].mean_exec_time, 10.005);
    assert_eq!(records[0].calls, 100);
    assert_eq!(records[0].shared_blks_hit, 500);
    assert_eq!(records[0].shared_blks_read, 50);
}

#[test]
fn test_analyze_pg_stat_statements_regression() {
    let baseline = vec![
        PgStatRecord {
            queryid: 1,
            calls: 100,
            total_exec_time: 1000.0,
            mean_exec_time: 10.0,
            shared_blks_hit: 0,
            shared_blks_read: 0,
        },
        PgStatRecord {
            queryid: 2,
            calls: 100,
            total_exec_time: 1000.0,
            mean_exec_time: 10.0,
            shared_blks_hit: 0,
            shared_blks_read: 0,
        },
    ];

    let current = vec![
        PgStatRecord {
            queryid: 1,
            calls: 100,
            total_exec_time: 3100.0,
            mean_exec_time: 31.0, // > 3x regression
            shared_blks_hit: 0,
            shared_blks_read: 0,
        },
        PgStatRecord {
            queryid: 2,
            calls: 100,
            total_exec_time: 2000.0,
            mean_exec_time: 20.0, // <= 3x
            shared_blks_hit: 0,
            shared_blks_read: 0,
        },
    ];

    let alerts = analyze_pg_stat_statements(&baseline, &current);
    assert_eq!(alerts.len(), 1);

    assert_eq!(alerts[0].queryid, 1);
    assert_eq!(alerts[0].baseline_mean, 10.0);
    assert_eq!(alerts[0].current_mean, 31.0);
    assert_eq!(alerts[0].degradation_factor, 3.1);
}

#[test]
fn test_calculate_execution_jitter() {
    let records = vec![
        PgStatRecord {
            queryid: 1,
            calls: 100,
            total_exec_time: 1000.0,
            mean_exec_time: 10.0,
            shared_blks_hit: 0,
            shared_blks_read: 0,
        },
        PgStatRecord {
            queryid: 2,
            calls: 100,
            total_exec_time: 1000.0,
            mean_exec_time: 9.0, // intentional mismatch to produce jitter
            shared_blks_hit: 0,
            shared_blks_read: 0,
        },
    ];

    let jitters = calculate_execution_jitter(&records);
    assert_eq!(jitters.len(), 2);

    assert_eq!(jitters[0].queryid, 1);
    assert!(jitters[0].jitter < 0.0001);

    assert_eq!(jitters[1].queryid, 2);
    assert!((jitters[1].jitter - 1.0).abs() < 0.0001); // 10.0 - 9.0 = 1.0
}
