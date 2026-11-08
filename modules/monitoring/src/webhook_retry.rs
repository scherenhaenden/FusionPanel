#![forbid(unsafe_code)]

use std::time::Duration;
use hmac::{Hmac, Mac, KeyInit};
use sha2::Sha256;
use uuid::Uuid;

type HmacSha256 = Hmac<Sha256>;

pub struct WebhookDispatcher {
    secret: String,
}

impl WebhookDispatcher {
    pub fn new(secret: String) -> Self {
        Self { secret }
    }

    pub fn sign_payload(&self, payload: &str) -> Result<String, String> {
        let mut mac = HmacSha256::new_from_slice(self.secret.as_bytes())
            .map_err(|e| format!("Failed to initialize HMAC: {}", e))?;
        mac.update(payload.as_bytes());
        let result = mac.finalize();
        let bytes = result.into_bytes();

        let mut hex = String::with_capacity(bytes.len() * 2);
        for b in bytes {
            use std::fmt::Write;
            write!(hex, "{:02x}", b).unwrap();
        }

        Ok(format!("sha256={}", hex))
    }
}

pub struct RetrySchedule {
    pub initial_delay: Duration,
    pub factor: f64,
    pub max_delay: Duration,
    pub max_attempts: u32,
}

impl RetrySchedule {
    pub fn new(
        initial_delay: Duration,
        factor: f64,
        max_delay: Duration,
        max_attempts: u32,
    ) -> Self {
        Self {
            initial_delay,
            factor,
            max_delay,
            max_attempts,
        }
    }

    pub fn get_delay(&self, attempt: u32) -> Option<Duration> {
        if attempt >= self.max_attempts {
            return None;
        }

        let delay = if attempt == 0 {
            self.initial_delay.as_secs_f64()
        } else {
            self.initial_delay.as_secs_f64() * self.factor.powi(attempt as i32)
        };

        // Pseudo-random jitter using UUID bytes
        let uuid = Uuid::new_v4();
        let bytes = uuid.as_bytes();
        let r_u32 = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        let r_f64 = (r_u32 as f64) / (u32::MAX as f64);
        let jitter = 0.8 + (r_f64 * 0.4);

        let final_delay = Duration::from_secs_f64(delay * jitter).min(self.max_delay);
        Some(final_delay)
    }
}
