#![forbid(unsafe_code)]

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Debug, PartialEq, Eq)]
pub enum GuardError {
    TooManyConnections,
    RateLimitExceeded,
}

struct ConnectionState {
    active_connections: usize,
    connection_attempts: VecDeque<Instant>,
}

impl ConnectionState {
    fn new() -> Self {
        Self {
            active_connections: 0,
            connection_attempts: VecDeque::new(),
        }
    }
}

struct GuardInner {
    ip_states: Mutex<HashMap<String, ConnectionState>>,
    user_states: Mutex<HashMap<String, ConnectionState>>,

    max_concurrent_per_ip: usize,
    max_concurrent_per_user: usize,

    max_burst_per_ip: usize,
    max_burst_per_user: usize,

    burst_window: Duration,
}

#[derive(Clone, Debug)]
pub struct MailConnectionGuard {
    inner: Arc<GuardInner>,
}

impl std::fmt::Debug for GuardInner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GuardInner")
            .field("max_concurrent_per_ip", &self.max_concurrent_per_ip)
            .field("max_concurrent_per_user", &self.max_concurrent_per_user)
            .field("max_burst_per_ip", &self.max_burst_per_ip)
            .field("max_burst_per_user", &self.max_burst_per_user)
            .field("burst_window", &self.burst_window)
            .finish()
    }
}

#[derive(Debug)]
pub struct IpToken {
    guard: MailConnectionGuard,
    ip: String,
}

impl Drop for IpToken {
    fn drop(&mut self) {
        self.guard.release_ip(&self.ip);
    }
}

#[derive(Debug)]
pub struct UserToken {
    guard: MailConnectionGuard,
    user: String,
}

impl Drop for UserToken {
    fn drop(&mut self) {
        self.guard.release_user(&self.user);
    }
}

impl MailConnectionGuard {
    pub fn new(
        max_concurrent_per_ip: usize,
        max_concurrent_per_user: usize,
        max_burst_per_ip: usize,
        max_burst_per_user: usize,
        burst_window: Duration,
    ) -> Self {
        Self {
            inner: Arc::new(GuardInner {
                ip_states: Mutex::new(HashMap::new()),
                user_states: Mutex::new(HashMap::new()),
                max_concurrent_per_ip,
                max_concurrent_per_user,
                max_burst_per_ip,
                max_burst_per_user,
                burst_window,
            }),
        }
    }

    pub fn acquire_ip(&self, ip: &str, now: Instant) -> Result<IpToken, GuardError> {
        let mut states = self.inner.ip_states.lock().unwrap();
        let state = states.entry(ip.to_string()).or_insert_with(ConnectionState::new);

        // Clean up old connection attempts
        while let Some(&oldest) = state.connection_attempts.front() {
            if now.saturating_duration_since(oldest) >= self.inner.burst_window {
                state.connection_attempts.pop_front();
            } else {
                break;
            }
        }

        if state.connection_attempts.len() >= self.inner.max_burst_per_ip {
            return Err(GuardError::RateLimitExceeded);
        }

        if state.active_connections >= self.inner.max_concurrent_per_ip {
            return Err(GuardError::TooManyConnections);
        }

        state.active_connections += 1;
        state.connection_attempts.push_back(now);

        Ok(IpToken {
            guard: self.clone(),
            ip: ip.to_string(),
        })
    }

    pub fn acquire_user(&self, user: &str, now: Instant) -> Result<UserToken, GuardError> {
        let mut states = self.inner.user_states.lock().unwrap();
        let state = states.entry(user.to_string()).or_insert_with(ConnectionState::new);

        // Clean up old connection attempts
        while let Some(&oldest) = state.connection_attempts.front() {
            if now.saturating_duration_since(oldest) >= self.inner.burst_window {
                state.connection_attempts.pop_front();
            } else {
                break;
            }
        }

        if state.connection_attempts.len() >= self.inner.max_burst_per_user {
            return Err(GuardError::RateLimitExceeded);
        }

        if state.active_connections >= self.inner.max_concurrent_per_user {
            return Err(GuardError::TooManyConnections);
        }

        state.active_connections += 1;
        state.connection_attempts.push_back(now);

        Ok(UserToken {
            guard: self.clone(),
            user: user.to_string(),
        })
    }

    fn release_ip(&self, ip: &str) {
        let now = Instant::now();
        let mut states = self.inner.ip_states.lock().unwrap();
        if let Some(state) = states.get_mut(ip) {
            state.active_connections = state.active_connections.saturating_sub(1);

            while let Some(&oldest) = state.connection_attempts.front() {
                if now.saturating_duration_since(oldest) >= self.inner.burst_window {
                    state.connection_attempts.pop_front();
                } else {
                    break;
                }
            }

            if state.active_connections == 0 && state.connection_attempts.is_empty() {
                states.remove(ip);
            }
        }
    }

    fn release_user(&self, user: &str) {
        let now = Instant::now();
        let mut states = self.inner.user_states.lock().unwrap();
        if let Some(state) = states.get_mut(user) {
            state.active_connections = state.active_connections.saturating_sub(1);

            while let Some(&oldest) = state.connection_attempts.front() {
                if now.saturating_duration_since(oldest) >= self.inner.burst_window {
                    state.connection_attempts.pop_front();
                } else {
                    break;
                }
            }

            if state.active_connections == 0 && state.connection_attempts.is_empty() {
                states.remove(user);
            }
        }
    }
}
