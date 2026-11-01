use crate::domain::error::{DomainError, Result};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DomainName(String);

impl DomainName {
    pub fn new(name: impl Into<String>) -> Result<Self> {
        let name = name.into();

        if name.is_empty() {
            return Err(DomainError::InvalidInput("Domain name cannot be empty".into()));
        }

        if name.len() > 253 {
            return Err(DomainError::InvalidInput("Domain name too long".into()));
        }

        if name.chars().any(|c| c > '\x7F') {
            return Err(DomainError::Unsupported("IDNA/Unicode domains are not supported".into()));
        }

        // Basic label check
        let labels: Vec<&str> = name.split('.').collect();
        if labels.is_empty() {
            return Err(DomainError::InvalidInput("Invalid domain structure".into()));
        }

        for label in labels {
            if label.is_empty() {
                return Err(DomainError::InvalidInput("Empty label not allowed".into()));
            }
            if label.len() > 63 {
                return Err(DomainError::InvalidInput("Label too long (max 63 chars)".into()));
            }
            if label.starts_with('-') || label.ends_with('-') {
                return Err(DomainError::InvalidInput("Label cannot start or end with hyphen".into()));
            }
            if !label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
                return Err(DomainError::InvalidInput("Invalid characters in label".into()));
            }
        }

        // Disallow bare IPs
        if name.parse::<std::net::IpAddr>().is_ok() {
            return Err(DomainError::InvalidInput("IP literals not allowed for domain name".into()));
        }

        Ok(Self(name.to_lowercase()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

pub fn validate_document_root(path_str: &str) -> Result<()> {
    if !path_str.starts_with("/srv/fusionpanel/sites/") {
        return Err(DomainError::InvalidInput("Path must be under /srv/fusionpanel/sites/".into()));
    }

    if path_str.contains('\0') {
        return Err(DomainError::InvalidInput("Path contains NUL byte".into()));
    }

    if path_str.contains("..") {
        return Err(DomainError::InvalidInput("Path traversal detected".into()));
    }

    let path = Path::new(path_str);
    if !path.is_absolute() {
        return Err(DomainError::InvalidInput("Path must be absolute".into()));
    }

    Ok(())
}
