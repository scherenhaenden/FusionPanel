#![forbid(unsafe_code)]

use std::fmt;

#[derive(Debug, PartialEq, Eq)]
pub enum LimitError {
    InvalidUsername,
    InvalidLimit,
}

impl fmt::Display for LimitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LimitError::InvalidUsername => write!(f, "Invalid username format"),
            LimitError::InvalidLimit => write!(f, "Invalid limit value"),
        }
    }
}

impl std::error::Error for LimitError {}

pub struct PostgresLimitManager;

impl PostgresLimitManager {
    fn validate_username(username: &str) -> Result<(), LimitError> {
        if username.is_empty() || username.len() > 63 {
            return Err(LimitError::InvalidUsername);
        }
        for c in username.chars() {
            if !c.is_ascii_alphanumeric() && c != '_' {
                return Err(LimitError::InvalidUsername);
            }
        }
        Ok(())
    }

    pub fn format_connection_limit(username: &str, limit: i32) -> Result<String, LimitError> {
        Self::validate_username(username)?;
        if limit < -1 {
            return Err(LimitError::InvalidLimit);
        }
        Ok(format!(
            "ALTER ROLE \"{}\" CONNECTION LIMIT {};",
            username, limit
        ))
    }

    pub fn format_statement_timeout(username: &str, timeout_ms: u64) -> Result<String, LimitError> {
        Self::validate_username(username)?;
        Ok(format!(
            "ALTER ROLE \"{}\" SET statement_timeout = '{}ms';",
            username, timeout_ms
        ))
    }

    pub fn format_reset_statement_timeout(username: &str) -> Result<String, LimitError> {
        Self::validate_username(username)?;
        Ok(format!(
            "ALTER ROLE \"{}\" RESET statement_timeout;",
            username
        ))
    }
}
