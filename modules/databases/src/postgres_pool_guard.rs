#![forbid(unsafe_code)]

use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    Active,
    Idle,
    IdleInTransaction,
}

#[derive(Debug, Clone)]
pub struct ConnectionInfo {
    pub pid: u32,
    pub state: ConnectionState,
    pub duration_in_state: Duration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GuardSignal {
    Cancel(u32),
    Terminate(u32),
}

#[derive(Debug, Clone)]
pub struct PoolGuardConfig {
    pub idle_in_transaction_session_timeout: Duration,
    pub statement_timeout: Option<Duration>,
}

pub struct PostgresPoolGuard {
    config: PoolGuardConfig,
}

impl PostgresPoolGuard {
    pub fn new(config: PoolGuardConfig) -> Self {
        Self { config }
    }

    pub fn evaluate_connections(&self, connections: &[ConnectionInfo]) -> Vec<GuardSignal> {
        let mut signals = Vec::new();

        for conn in connections {
            match conn.state {
                ConnectionState::IdleInTransaction => {
                    if conn.duration_in_state > self.config.idle_in_transaction_session_timeout {
                        signals.push(GuardSignal::Terminate(conn.pid));
                    }
                }
                ConnectionState::Active => {
                    if let Some(timeout) = self.config.statement_timeout {
                        if conn.duration_in_state > timeout {
                            // Usually, we want to cancel the long-running statement first,
                            // rather than terminating the whole connection immediately.
                            signals.push(GuardSignal::Cancel(conn.pid));
                        }
                    }
                }
                ConnectionState::Idle => {
                    // Do nothing for Idle connections by default,
                    // unless a specific idle_session_timeout is configured (not requested).
                }
            }
        }

        signals
    }
}
