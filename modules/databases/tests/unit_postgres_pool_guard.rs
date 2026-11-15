#![forbid(unsafe_code)]

use databases::postgres_pool_guard::{
    ConnectionInfo, ConnectionState, GuardSignal, PoolGuardConfig, PostgresPoolGuard,
};
use std::time::Duration;

#[test]
fn test_evaluate_connections_idle_in_transaction_timeout() {
    let config = PoolGuardConfig {
        idle_in_transaction_session_timeout: Duration::from_secs(60),
        statement_timeout: None,
    };
    let guard = PostgresPoolGuard::new(config);

    let connections = vec![
        ConnectionInfo {
            pid: 101,
            state: ConnectionState::IdleInTransaction,
            duration_in_state: Duration::from_secs(30), // Under timeout
        },
        ConnectionInfo {
            pid: 102,
            state: ConnectionState::IdleInTransaction,
            duration_in_state: Duration::from_secs(61), // Over timeout
        },
    ];

    let signals = guard.evaluate_connections(&connections);

    assert_eq!(signals.len(), 1);
    assert_eq!(signals[0], GuardSignal::Terminate(102));
}

#[test]
fn test_evaluate_connections_statement_timeout() {
    let config = PoolGuardConfig {
        idle_in_transaction_session_timeout: Duration::from_secs(60),
        statement_timeout: Some(Duration::from_secs(30)),
    };
    let guard = PostgresPoolGuard::new(config);

    let connections = vec![
        ConnectionInfo {
            pid: 201,
            state: ConnectionState::Active,
            duration_in_state: Duration::from_secs(20), // Under timeout
        },
        ConnectionInfo {
            pid: 202,
            state: ConnectionState::Active,
            duration_in_state: Duration::from_secs(35), // Over timeout
        },
    ];

    let signals = guard.evaluate_connections(&connections);

    assert_eq!(signals.len(), 1);
    assert_eq!(signals[0], GuardSignal::Cancel(202));
}

#[test]
fn test_evaluate_connections_idle_no_action() {
    let config = PoolGuardConfig {
        idle_in_transaction_session_timeout: Duration::from_secs(60),
        statement_timeout: Some(Duration::from_secs(30)),
    };
    let guard = PostgresPoolGuard::new(config);

    let connections = vec![
        ConnectionInfo {
            pid: 301,
            state: ConnectionState::Idle,
            duration_in_state: Duration::from_secs(3600), // Very long idle
        },
    ];

    let signals = guard.evaluate_connections(&connections);

    assert_eq!(signals.len(), 0);
}

#[test]
fn test_evaluate_connections_mixed() {
    let config = PoolGuardConfig {
        idle_in_transaction_session_timeout: Duration::from_secs(60),
        statement_timeout: Some(Duration::from_secs(30)),
    };
    let guard = PostgresPoolGuard::new(config);

    let connections = vec![
        ConnectionInfo {
            pid: 1,
            state: ConnectionState::Idle,
            duration_in_state: Duration::from_secs(100),
        },
        ConnectionInfo {
            pid: 2,
            state: ConnectionState::IdleInTransaction,
            duration_in_state: Duration::from_secs(70), // Over timeout -> Terminate
        },
        ConnectionInfo {
            pid: 3,
            state: ConnectionState::IdleInTransaction,
            duration_in_state: Duration::from_secs(10),
        },
        ConnectionInfo {
            pid: 4,
            state: ConnectionState::Active,
            duration_in_state: Duration::from_secs(40), // Over timeout -> Cancel
        },
        ConnectionInfo {
            pid: 5,
            state: ConnectionState::Active,
            duration_in_state: Duration::from_secs(5),
        },
    ];

    let signals = guard.evaluate_connections(&connections);

    assert_eq!(signals.len(), 2);
    assert!(signals.contains(&GuardSignal::Terminate(2)));
    assert!(signals.contains(&GuardSignal::Cancel(4)));
}
