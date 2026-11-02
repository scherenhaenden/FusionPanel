#![forbid(unsafe_code)]

use crate::heartbeat::{Backoff, HeartbeatPayload};
use futures_util::{SinkExt, StreamExt};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::time::Duration;
use tokio::time::sleep;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::Message;

#[derive(Debug)]
pub enum DaemonError {
    TransportFailed,
}

#[async_trait::async_trait]
pub trait HeartbeatTransport {
    async fn send_heartbeat(&self, payload: &HeartbeatPayload) -> Result<(), DaemonError>;
}

pub struct HmacWsTransport {
    url: String,
    secret: Vec<u8>,
}

impl HmacWsTransport {
    pub fn new(url: String, secret: Vec<u8>) -> Self {
        Self { url, secret }
    }

    fn sign_payload(&self, payload: &[u8]) -> Result<String, DaemonError> {
        type HmacSha256 = Hmac<Sha256>;
        let mut mac =
            HmacSha256::new_from_slice(&self.secret).map_err(|_| DaemonError::TransportFailed)?;
        mac.update(payload);
        let result = mac.finalize();
        Ok(hex::encode(result.into_bytes()))
    }
}

#[async_trait::async_trait]
impl HeartbeatTransport for HmacWsTransport {
    async fn send_heartbeat(&self, payload: &HeartbeatPayload) -> Result<(), DaemonError> {
        let payload_str =
            serde_json::to_string(payload).map_err(|_| DaemonError::TransportFailed)?;
        let signature = self.sign_payload(payload_str.as_bytes())?;

        let mut request = self
            .url
            .clone()
            .into_client_request()
            .map_err(|_| DaemonError::TransportFailed)?;
        request.headers_mut().insert(
            "x-hmac-signature",
            signature.parse().map_err(|_| DaemonError::TransportFailed)?,
        );

        let (mut ws_stream, _) = connect_async(request)
            .await
            .map_err(|_| DaemonError::TransportFailed)?;

        ws_stream
            .send(Message::Text(payload_str))
            .await
            .map_err(|_| DaemonError::TransportFailed)?;

        if let Some(msg) = ws_stream.next().await {
            let _ = msg.map_err(|_| DaemonError::TransportFailed)?;
        } else {
            return Err(DaemonError::TransportFailed);
        }

        Ok(())
    }
}

pub struct AgentDaemon<T: HeartbeatTransport> {
    transport: T,
    agent_version: String,
    initial_delay: Duration,
    max_delay: Duration,
}

impl<T: HeartbeatTransport> AgentDaemon<T> {
    pub fn new(transport: T, agent_version: String) -> Self {
        Self {
            transport,
            agent_version,
            initial_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(60),
        }
    }

    pub fn with_delays(mut self, initial: Duration, max: Duration) -> Self {
        self.initial_delay = initial;
        self.max_delay = max;
        self
    }

    pub async fn run(&self) {
        let mut backoff = Backoff::new(self.initial_delay, self.max_delay);
        let start_time = std::time::Instant::now();

        loop {
            let uptime = start_time.elapsed().as_secs();
            let payload = HeartbeatPayload::new(self.agent_version.clone(), uptime);

            match self.transport.send_heartbeat(&payload).await {
                Ok(_) => {
                    backoff.reset(self.initial_delay);
                    sleep(Duration::from_secs(30)).await;
                }
                Err(_) => {
                    let delay = backoff.next_delay();
                    sleep(delay).await;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    struct MockTransport {
        calls: Arc<Mutex<usize>>,
        fail_count: usize,
    }

    #[async_trait::async_trait]
    impl HeartbeatTransport for MockTransport {
        async fn send_heartbeat(&self, _payload: &HeartbeatPayload) -> Result<(), DaemonError> {
            let mut calls = self.calls.lock().unwrap();
            *calls += 1;

            if *calls <= self.fail_count {
                Err(DaemonError::TransportFailed)
            } else {
                Ok(())
            }
        }
    }

    #[tokio::test]
    async fn test_daemon_loop_backoff() {
        let calls = Arc::new(Mutex::new(0));
        let transport = MockTransport {
            calls: calls.clone(),
            fail_count: 2,
        };

        let daemon = AgentDaemon::new(transport, "1.0.0".to_string())
            .with_delays(Duration::from_millis(10), Duration::from_millis(50));

        let handle = tokio::spawn(async move {
            daemon.run().await;
        });

        sleep(Duration::from_millis(150)).await;
        handle.abort();

        let call_count = *calls.lock().unwrap();
        assert!(call_count >= 3);
    }
}
