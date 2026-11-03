use subtle::ConstantTimeEq;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum WebhookError {
    #[error("Missing signature header")]
    MissingSignature,
    #[error("Signature mismatch")]
    SignatureMismatch,
    #[error("Invalid payload format")]
    InvalidPayload,
}

pub enum GitlabEvent {
    Push { r#ref: String, commit_sha: String },
    Tag { r#ref: String },
    Unknown(String),
}

pub fn verify_token(secret: &str, token_header: &str) -> Result<(), WebhookError> {
    if secret.as_bytes().ct_eq(token_header.as_bytes()).into() {
        Ok(())
    } else {
        Err(WebhookError::SignatureMismatch)
    }
}

pub fn parse_event(event_header: &str, payload: &[u8]) -> Result<GitlabEvent, WebhookError> {
    let json: serde_json::Value =
        serde_json::from_slice(payload).map_err(|_| WebhookError::InvalidPayload)?;

    match event_header {
        "Push Hook" => {
            let r#ref = json["ref"].as_str().ok_or(WebhookError::InvalidPayload)?;
            let commit_sha = json["after"].as_str().ok_or(WebhookError::InvalidPayload)?;
            Ok(GitlabEvent::Push {
                r#ref: r#ref.to_string(),
                commit_sha: commit_sha.to_string(),
            })
        }
        "Tag Push Hook" => {
            let r#ref = json["ref"].as_str().ok_or(WebhookError::InvalidPayload)?;
            Ok(GitlabEvent::Tag {
                r#ref: r#ref.to_string(),
            })
        }
        other => Ok(GitlabEvent::Unknown(other.to_string())),
    }
}
