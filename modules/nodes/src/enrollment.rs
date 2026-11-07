#![forbid(unsafe_code)]

use chrono::{DateTime, Duration, Utc};
use std::collections::HashMap;
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum EnrollmentError {
    #[error("Token not found or already consumed")]
    TokenNotFound,
    #[error("Token expired")]
    TokenExpired,
}

#[derive(Debug, Clone)]
pub struct EnrollmentToken {
    pub token_string: String,
    pub expires_at: DateTime<Utc>,
    pub consumed: bool,
}

#[derive(Debug, Clone)]
pub struct EnrollmentResult {
    pub node_id: Uuid,
    pub cert_bundle: String,
}

pub struct MasterEnrollment {
    tokens: HashMap<String, EnrollmentToken>,
}

impl MasterEnrollment {
    pub fn new() -> Self {
        Self {
            tokens: HashMap::new(),
        }
    }

    pub fn generate_token(&mut self) -> String {
        self.generate_token_with_ttl(Duration::minutes(15))
    }

    pub fn generate_token_with_ttl(&mut self, ttl: Duration) -> String {
        let token_string = Uuid::new_v4().to_string();
        let token = EnrollmentToken {
            token_string: token_string.clone(),
            expires_at: Utc::now() + ttl,
            consumed: false,
        };
        self.tokens.insert(token_string.clone(), token);
        token_string
    }

    pub fn join(
        &mut self,
        token_string: &str,
        public_key: &str,
    ) -> Result<EnrollmentResult, EnrollmentError> {
        let token = self
            .tokens
            .get_mut(token_string)
            .ok_or(EnrollmentError::TokenNotFound)?;

        if token.consumed {
            return Err(EnrollmentError::TokenNotFound);
        }

        if Utc::now() > token.expires_at {
            return Err(EnrollmentError::TokenExpired);
        }

        token.consumed = true;

        Ok(EnrollmentResult {
            node_id: Uuid::new_v4(),
            cert_bundle: format!("mtls_cert_bundle_for_{}", public_key),
        })
    }
}

impl Default for MasterEnrollment {
    fn default() -> Self {
        Self::new()
    }
}
