#![forbid(unsafe_code)]

use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, PartialEq, Eq)]
pub enum WebhookVerificationError {
    InvalidFormat,
    ExpiredTimestamp,
    FutureTimestamp,
    InvalidSignature,
}

pub struct WebhookSigner {
    secret: Vec<u8>,
    max_age_seconds: u64,
}

impl WebhookSigner {
    pub fn new(secret: impl Into<Vec<u8>>, max_age_seconds: u64) -> Self {
        Self {
            secret: secret.into(),
            max_age_seconds,
        }
    }

    pub fn sign(&self, payload: &str, timestamp: u64) -> String {
        let mut mac = HmacSha256::new_from_slice(&self.secret).expect("HMAC can take key of any size");
        let to_sign = format!("{}.{}", timestamp, payload);
        mac.update(to_sign.as_bytes());
        let result = mac.finalize();
        let hex_sig = hex::encode(result.into_bytes());
        format!("t={},v1={}", timestamp, hex_sig)
    }

    pub fn verify(&self, payload: &str, signature_header: &str, current_timestamp: u64) -> Result<(), WebhookVerificationError> {
        let parts: Vec<&str> = signature_header.split(',').collect();
        let mut timestamp_str = None;
        let mut signature_hex = None;

        for part in parts {
            if let Some(t) = part.strip_prefix("t=") {
                timestamp_str = Some(t);
            } else if let Some(v) = part.strip_prefix("v1=") {
                signature_hex = Some(v);
            }
        }

        let timestamp_str = timestamp_str.ok_or(WebhookVerificationError::InvalidFormat)?;
        let signature_hex = signature_hex.ok_or(WebhookVerificationError::InvalidFormat)?;

        let timestamp: u64 = timestamp_str.parse().map_err(|_| WebhookVerificationError::InvalidFormat)?;

        if timestamp > current_timestamp {
            return Err(WebhookVerificationError::FutureTimestamp);
        }

        if current_timestamp - timestamp > self.max_age_seconds {
            return Err(WebhookVerificationError::ExpiredTimestamp);
        }

        let to_sign = format!("{}.{}", timestamp, payload);
        let actual_bytes = hex::decode(signature_hex).map_err(|_| WebhookVerificationError::InvalidFormat)?;

        // Constant time comparison is recommended for MACs, `hmac::Mac::verify_slice` does this.
        let mut verify_mac = HmacSha256::new_from_slice(&self.secret).expect("HMAC can take key of any size");
        verify_mac.update(to_sign.as_bytes());
        if verify_mac.verify_slice(&actual_bytes).is_err() {
            return Err(WebhookVerificationError::InvalidSignature);
        }

        Ok(())
    }
}
