#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeartbeatPayload {
    pub version: u32,
    pub agent_version: String,
    pub uptime_seconds: u64,
}

impl HeartbeatPayload {
    pub fn new(agent_version: String, uptime_seconds: u64) -> Self {
        Self {
            version: 1,
            agent_version,
            uptime_seconds,
        }
    }
}

pub struct Backoff {
    current: Duration,
    max: Duration,
}

impl Backoff {
    pub fn new(initial: Duration, max: Duration) -> Self {
        Self {
            current: initial,
            max,
        }
    }

    pub fn next_delay(&mut self) -> Duration {
        let delay = self.current;
        self.current = std::cmp::min(self.current * 2, self.max);
        delay
    }

    pub fn reset(&mut self, initial: Duration) {
        self.current = initial;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_heartbeat_payload() {
        let payload = HeartbeatPayload::new("1.0.0".to_string(), 12345);
        assert_eq!(payload.version, 1);
        assert_eq!(payload.agent_version, "1.0.0");
        assert_eq!(payload.uptime_seconds, 12345);
    }

    #[test]
    fn test_backoff() {
        let initial = Duration::from_secs(1);
        let max = Duration::from_secs(10);
        let mut backoff = Backoff::new(initial, max);

        assert_eq!(backoff.next_delay(), Duration::from_secs(1));
        assert_eq!(backoff.next_delay(), Duration::from_secs(2));
        assert_eq!(backoff.next_delay(), Duration::from_secs(4));
        assert_eq!(backoff.next_delay(), Duration::from_secs(8));
        assert_eq!(backoff.next_delay(), Duration::from_secs(10));
        assert_eq!(backoff.next_delay(), Duration::from_secs(10));

        backoff.reset(initial);
        assert_eq!(backoff.next_delay(), Duration::from_secs(1));
    }
}
