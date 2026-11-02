use hmac::{Hmac, Mac};
use sha2::Sha256;
use subtle::ConstantTimeEq;
use thiserror::Error;

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Error)]
pub enum WebhookError {
    #[error("Missing signature header")]
    MissingSignature,
    #[error("Invalid signature format")]
    InvalidSignatureFormat,
    #[error("Signature mismatch")]
    SignatureMismatch,
    #[error("Invalid payload format")]
    InvalidPayload,
}

pub enum GithubEvent {
    Push { r#ref: String, commit_sha: String },
    Tag { r#ref: String },
    Unknown(String),
}

pub fn verify_signature(
    secret: &str,
    signature_header: &str,
    payload: &[u8],
) -> Result<(), WebhookError> {
    if !signature_header.starts_with("sha256=") {
        return Err(WebhookError::InvalidSignatureFormat);
    }

    let hex_sig = &signature_header[7..];
    let decoded_sig = hex::decode(hex_sig).map_err(|_| WebhookError::InvalidSignatureFormat)?;

    let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
        .map_err(|_| WebhookError::SignatureMismatch)?; // Should not happen with SHA256

    mac.update(payload);
    let expected_sig = mac.finalize().into_bytes();

    if expected_sig.ct_eq(&decoded_sig).into() {
        Ok(())
    } else {
        Err(WebhookError::SignatureMismatch)
    }
}

pub fn parse_event(event_header: &str, payload: &[u8]) -> Result<GithubEvent, WebhookError> {
    let json: serde_json::Value =
        serde_json::from_slice(payload).map_err(|_| WebhookError::InvalidPayload)?;

    match event_header {
        "push" => {
            let r#ref = json["ref"].as_str().ok_or(WebhookError::InvalidPayload)?;

            if r#ref.starts_with("refs/tags/") {
                Ok(GithubEvent::Tag {
                    r#ref: r#ref.to_string(),
                })
            } else {
                let commit_sha = json["after"].as_str().ok_or(WebhookError::InvalidPayload)?;
                Ok(GithubEvent::Push {
                    r#ref: r#ref.to_string(),
                    commit_sha: commit_sha.to_string(),
                })
            }
        }
        other => Ok(GithubEvent::Unknown(other.to_string())),
    }
}
