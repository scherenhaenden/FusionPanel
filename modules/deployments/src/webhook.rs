use chrono::{DateTime, Utc};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::collections::HashSet;
use std::sync::Mutex;

use crate::error::DeploymentError;
use crate::types::GitSource;

type HmacSha256 = Hmac<Sha256>;

/// Verifies HMAC-SHA256 signatures in constant time.
pub fn verify_signature(
    secret: &[u8],
    payload: &[u8],
    signature_header: &str,
) -> Result<(), DeploymentError> {
    if secret.is_empty() {
        return Err(DeploymentError::InvalidInput(
            "webhook secret cannot be empty".to_string(),
        ));
    }

    let hex_sig = signature_header
        .strip_prefix("sha256=")
        .unwrap_or(signature_header);

    let decoded_sig = hex::decode(hex_sig)
        .map_err(|e| DeploymentError::InvalidInput(format!("malformed signature hex: {e}")))?;

    let mut mac = HmacSha256::new_from_slice(secret)
        .map_err(|e| DeploymentError::InvalidInput(format!("invalid key length: {e}")))?;
    mac.update(payload);

    mac.verify_slice(&decoded_sig).map_err(|_| {
        DeploymentError::SignatureMismatch("signature verification failed".to_string())
    })
}

/// Computes the expected HMAC-SHA256 signature in GitHub format (`sha256=<hex>`).
pub fn compute_signature(secret: &[u8], payload: &[u8]) -> Result<String, DeploymentError> {
    if secret.is_empty() {
        return Err(DeploymentError::InvalidInput(
            "webhook secret cannot be empty".to_string(),
        ));
    }
    let mut mac = HmacSha256::new_from_slice(secret)
        .map_err(|e| DeploymentError::InvalidInput(format!("invalid key length: {e}")))?;
    mac.update(payload);
    let result = mac.finalize();
    Ok(format!("sha256={}", hex::encode(result.into_bytes())))
}

/// Tracks event IDs and delivery timestamps to prevent replay attacks.
#[derive(Debug)]
pub struct ReplayGuard {
    seen_ids: Mutex<HashSet<String>>,
    tolerance_seconds: i64,
}

impl ReplayGuard {
    pub fn new(tolerance_seconds: i64) -> Self {
        Self {
            seen_ids: Mutex::new(HashSet::new()),
            tolerance_seconds,
        }
    }

    pub fn check_and_record(
        &self,
        event_id: &str,
        event_timestamp: Option<DateTime<Utc>>,
        now: DateTime<Utc>,
    ) -> Result<(), DeploymentError> {
        if event_id.trim().is_empty() {
            return Err(DeploymentError::InvalidInput(
                "event id cannot be empty".to_string(),
            ));
        }

        if let Some(ts) = event_timestamp {
            let diff = (now - ts).num_seconds().abs();
            if diff > self.tolerance_seconds {
                return Err(DeploymentError::ReplayDetected(format!(
                    "event timestamp delta {diff}s exceeds allowed tolerance {}s",
                    self.tolerance_seconds
                )));
            }
        }

        let mut seen = self.seen_ids.lock().map_err(|_| {
            DeploymentError::OperationFailed("lock poisoning in replay guard".to_string())
        })?;

        if !seen.insert(event_id.to_string()) {
            return Err(DeploymentError::ReplayDetected(format!(
                "event id '{event_id}' has already been processed"
            )));
        }

        Ok(())
    }
}

/// Parsed webhook event payload
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WebhookEvent {
    Push {
        ref_name: String,
        before_sha: String,
        after_sha: String,
        git_source: GitSource,
        sender: String,
    },
    Ping {
        zen: Option<String>,
        hook_id: Option<u64>,
    },
    Unsupported(String),
}

/// Parses a GitHub webhook payload (JSON) given event header `X-GitHub-Event` and body bytes.
pub fn parse_github_webhook(
    event_type: &str,
    payload: &[u8],
) -> Result<WebhookEvent, DeploymentError> {
    match event_type {
        "ping" => {
            let v: serde_json::Value = serde_json::from_slice(payload)
                .map_err(|e| DeploymentError::InvalidInput(format!("malformed ping json: {e}")))?;
            let zen = v.get("zen").and_then(|z| z.as_str()).map(|s| s.to_string());
            let hook_id = v.get("hook_id").and_then(|h| h.as_u64());
            Ok(WebhookEvent::Ping { zen, hook_id })
        }
        "push" => {
            let v: serde_json::Value = serde_json::from_slice(payload)
                .map_err(|e| DeploymentError::InvalidInput(format!("malformed push json: {e}")))?;

            let ref_name = v
                .get("ref")
                .and_then(|r| r.as_str())
                .ok_or_else(|| {
                    DeploymentError::InvalidInput("missing 'ref' in push event".to_string())
                })?
                .to_string();

            let before_sha = v
                .get("before")
                .and_then(|r| r.as_str())
                .unwrap_or("")
                .to_string();

            let after_sha = v
                .get("after")
                .and_then(|r| r.as_str())
                .ok_or_else(|| {
                    DeploymentError::InvalidInput(
                        "missing 'after' commit SHA in push event".to_string(),
                    )
                })?
                .to_string();

            let repo_url = v
                .get("repository")
                .and_then(|r| r.get("clone_url").or_else(|| r.get("html_url")))
                .and_then(|u| u.as_str())
                .ok_or_else(|| {
                    DeploymentError::InvalidInput(
                        "missing repository url in push event".to_string(),
                    )
                })?
                .to_string();

            let sender = v
                .get("sender")
                .and_then(|s| s.get("login"))
                .and_then(|l| l.as_str())
                .unwrap_or("unknown")
                .to_string();

            let branch = ref_name.strip_prefix("refs/heads/").map(|b| b.to_string());

            Ok(WebhookEvent::Push {
                ref_name: ref_name.clone(),
                before_sha,
                after_sha: after_sha.clone(),
                git_source: GitSource {
                    repo_url,
                    commit_sha: after_sha,
                    branch,
                    ref_name: Some(ref_name),
                },
                sender,
            })
        }
        other => Ok(WebhookEvent::Unsupported(other.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[test]
    fn test_valid_signature_verification() {
        let secret = b"my_super_secret_webhook_key";
        let payload = b"{\"action\":\"test\"}";
        let sig = compute_signature(secret, payload).unwrap();
        assert!(verify_signature(secret, payload, &sig).is_ok());
    }

    #[test]
    fn test_invalid_signature_rejection() {
        let secret = b"my_super_secret_webhook_key";
        let payload = b"{\"action\":\"test\"}";
        let bad_sig = "sha256=0000000000000000000000000000000000000000000000000000000000000000";
        let res = verify_signature(secret, payload, bad_sig);
        assert!(matches!(res, Err(DeploymentError::SignatureMismatch(_))));
    }

    #[test]
    fn test_empty_secret_rejection() {
        let secret = b"";
        let payload = b"{}";
        let res = verify_signature(secret, payload, "sha256=123");
        assert!(matches!(res, Err(DeploymentError::InvalidInput(_))));
    }

    #[test]
    fn test_replay_guard_prevents_duplicate_delivery() {
        let guard = ReplayGuard::new(300);
        let now = Utc::now();
        assert!(guard.check_and_record("evt-1", Some(now), now).is_ok());
        // Duplicate delivery must fail
        let res = guard.check_and_record("evt-1", Some(now), now);
        assert!(matches!(res, Err(DeploymentError::ReplayDetected(_))));
    }

    #[test]
    fn test_replay_guard_rejects_expired_timestamp() {
        let guard = ReplayGuard::new(300);
        let now = Utc::now();
        let old = now - Duration::seconds(400);
        let res = guard.check_and_record("evt-old", Some(old), now);
        assert!(matches!(res, Err(DeploymentError::ReplayDetected(_))));
    }

    #[test]
    fn test_parse_push_webhook() {
        let payload = serde_json::json!({
            "ref": "refs/heads/master",
            "before": "0000000000000000000000000000000000000000",
            "after": "3a9a482fb69312597af52ea8162e03876e12ecc8",
            "repository": {
                "clone_url": "https://github.com/scherenhaenden/FusionPanel.git"
            },
            "sender": {
                "login": "octocat"
            }
        });
        let bytes = serde_json::to_vec(&payload).unwrap();
        let event = parse_github_webhook("push", &bytes).unwrap();
        match event {
            WebhookEvent::Push {
                ref_name,
                after_sha,
                git_source,
                sender,
                ..
            } => {
                assert_eq!(ref_name, "refs/heads/master");
                assert_eq!(after_sha, "3a9a482fb69312597af52ea8162e03876e12ecc8");
                assert_eq!(git_source.branch.as_deref(), Some("master"));
                assert_eq!(sender, "octocat");
            }
            _ => panic!("expected push event"),
        }
    }
}
