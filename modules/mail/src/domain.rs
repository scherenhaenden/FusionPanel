#![forbid(unsafe_code)]

use regex::Regex;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use uuid::Uuid;

use crate::error::MailError;

/// A validated canonical ASCII DNS domain name.
/// Rejects IDNA/Unicode U-labels, empty labels, leading/trailing hyphens,
/// labels > 63 chars, domain > 253 chars, and IP address literals.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MailDomainName(String);

impl MailDomainName {
    pub fn parse(input: &str) -> Result<Self, MailError> {
        let trimmed = input.trim_end_matches('.');
        if trimmed.is_empty() {
            return Err(MailError::InvalidInput(
                "Domain name cannot be empty".to_string(),
            ));
        }
        if trimmed.len() > 253 {
            return Err(MailError::InvalidInput(
                "Domain name exceeds 253 characters".to_string(),
            ));
        }

        // Domain must be pure ASCII
        if !trimmed.is_ascii() {
            return Err(MailError::InvalidInput(
                "Domain name must be canonical ASCII; IDNA/Unicode domains are not permitted"
                    .to_string(),
            ));
        }

        // Reject IP addresses (IPv4 or IPv6)
        if trimmed.parse::<std::net::IpAddr>().is_ok() {
            return Err(MailError::InvalidInput(
                "Domain name cannot be an IP address literal".to_string(),
            ));
        }

        let labels: Vec<&str> = trimmed.split('.').collect();
        if labels.is_empty() {
            return Err(MailError::InvalidInput(
                "Domain name must contain at least one label".to_string(),
            ));
        }

        for label in &labels {
            if label.is_empty() {
                return Err(MailError::InvalidInput(
                    "Domain name contains empty label".to_string(),
                ));
            }
            if label.len() > 63 {
                return Err(MailError::InvalidInput(format!(
                    "Domain label '{}' exceeds 63 characters",
                    label
                )));
            }
            if label.starts_with('-') || label.ends_with('-') {
                return Err(MailError::InvalidInput(format!(
                    "Domain label '{}' cannot start or end with a hyphen",
                    label
                )));
            }
            if !label
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
            {
                return Err(MailError::InvalidInput(format!(
                    "Domain label '{}' contains invalid character",
                    label
                )));
            }
        }

        let canonical = trimmed.to_ascii_lowercase();
        Ok(Self(canonical))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for MailDomainName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Serialize for MailDomainName {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for MailDomainName {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Self::parse(&s).map_err(serde::de::Error::custom)
    }
}

/// A validated mailbox address: `<local-part>@<domain>`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MailAddress {
    local_part: String,
    domain: MailDomainName,
}

impl MailAddress {
    pub fn parse(input: &str) -> Result<Self, MailError> {
        let parts: Vec<&str> = input.split('@').collect();
        if parts.len() != 2 {
            return Err(MailError::InvalidInput(
                "Mail address must contain exactly one '@' character".to_string(),
            ));
        }

        let local_part = parts[0];
        let domain_part = parts[1];

        if local_part.is_empty() {
            return Err(MailError::InvalidInput(
                "Local-part cannot be empty".to_string(),
            ));
        }
        if local_part.len() > 64 {
            return Err(MailError::InvalidInput(
                "Local-part exceeds 64 characters".to_string(),
            ));
        }
        if !local_part.is_ascii() {
            return Err(MailError::InvalidInput(
                "Local-part must be ASCII".to_string(),
            ));
        }
        if local_part.starts_with('.') || local_part.ends_with('.') {
            return Err(MailError::InvalidInput(
                "Local-part cannot start or end with a dot".to_string(),
            ));
        }
        if local_part.contains("..") {
            return Err(MailError::InvalidInput(
                "Local-part cannot contain consecutive dots".to_string(),
            ));
        }

        // Check for disallowed characters or directive injection (newlines, cr, tabs, etc.)
        for ch in local_part.chars() {
            if ch.is_ascii_control()
                || ch.is_whitespace()
                || ch == ':'
                || ch == ';'
                || ch == '/'
                || ch == '\\'
            {
                return Err(MailError::InvalidInput(format!(
                    "Local-part contains invalid character: {:?}",
                    ch
                )));
            }
            if !ch.is_ascii_alphanumeric()
                && !matches!(
                    ch,
                    '.' | '-'
                        | '_'
                        | '+'
                        | '='
                        | '!'
                        | '#'
                        | '$'
                        | '%'
                        | '&'
                        | '\''
                        | '*'
                        | '/'
                        | '?'
                        | '^'
                        | '`'
                        | '{'
                        | '|'
                        | '}'
                        | '~'
                )
            {
                return Err(MailError::InvalidInput(format!(
                    "Local-part contains invalid character: {:?}",
                    ch
                )));
            }
        }

        let domain = MailDomainName::parse(domain_part)?;
        Ok(Self {
            local_part: local_part.to_string(),
            domain,
        })
    }

    pub fn local_part(&self) -> &str {
        &self.local_part
    }

    pub fn domain(&self) -> &MailDomainName {
        &self.domain
    }

    pub fn full_address(&self) -> String {
        format!("{}@{}", self.local_part, self.domain)
    }
}

impl fmt::Display for MailAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}@{}", self.local_part, self.domain)
    }
}

impl Serialize for MailAddress {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.full_address())
    }
}

impl<'de> Deserialize<'de> for MailAddress {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Self::parse(&s).map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MailDomainError {
    InvalidCharacters,
    TooLong,
    Empty,
}

impl fmt::Display for MailDomainError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidCharacters => write!(f, "Domain contains invalid characters or IDNA"),
            Self::TooLong => write!(f, "Domain is too long"),
            Self::Empty => write!(f, "Domain cannot be empty"),
        }
    }
}
impl std::error::Error for MailDomainError {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MailDomain(String);

impl MailDomain {
    pub fn new(domain: &str) -> Result<Self, MailDomainError> {
        if domain.is_empty() {
            return Err(MailDomainError::Empty);
        }
        if domain.len() > 253 {
            return Err(MailDomainError::TooLong);
        }

        // Canonical ASCII DNS name check, reject IDNA/Unicode
        if !domain.is_ascii() {
            return Err(MailDomainError::InvalidCharacters);
        }

        // Simple regex for valid hostname ASCII rules
        let re =
            Regex::new(r"^(?i)[a-z0-9]([a-z0-9-]*[a-z0-9])?(\.[a-z0-9]([a-z0-9-]*[a-z0-9])?)*$")
                .unwrap();
        if !re.is_match(domain) {
            return Err(MailDomainError::InvalidCharacters);
        }

        // Prevent domain traversal/paths
        if domain.contains('/') || domain.contains('\\') || domain.contains("..") {
            return Err(MailDomainError::InvalidCharacters);
        }

        Ok(Self(domain.to_lowercase()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EmailAddressError {
    InvalidFormat,
    DomainError(MailDomainError),
}

impl fmt::Display for EmailAddressError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidFormat => write!(f, "Invalid email address format"),
            Self::DomainError(e) => write!(f, "Invalid domain in email address: {}", e),
        }
    }
}
impl std::error::Error for EmailAddressError {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmailAddress {
    local_part: String,
    domain: MailDomain,
}

impl EmailAddress {
    pub fn new(email: &str) -> Result<Self, EmailAddressError> {
        let parts: Vec<&str> = email.split('@').collect();
        if parts.len() != 2 {
            return Err(EmailAddressError::InvalidFormat);
        }

        let local_part = parts[0];
        let domain_part = parts[1];

        if local_part.is_empty() || local_part.len() > 64 {
            return Err(EmailAddressError::InvalidFormat);
        }

        // Basic RFC 5322 local part validation
        let re = Regex::new(r"^[a-zA-Z0-9.!#$%&'*+/=?^_`{|}~-]+$").unwrap();
        if !re.is_match(local_part) {
            return Err(EmailAddressError::InvalidFormat);
        }

        // Validate domain
        let domain = MailDomain::new(domain_part).map_err(EmailAddressError::DomainError)?;

        Ok(Self {
            local_part: local_part.to_string(),
            domain,
        })
    }

    pub fn as_string(&self) -> String {
        format!("{}@{}", self.local_part, self.domain.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MailQuotaError {
    ZeroOrNegative,
}

impl fmt::Display for MailQuotaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroOrNegative => write!(f, "Mail quota must be greater than zero"),
        }
    }
}
impl std::error::Error for MailQuotaError {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MailQuota(u64); // bytes

impl MailQuota {
    pub fn new(bytes: u64) -> Result<Self, MailQuotaError> {
        if bytes == 0 {
            return Err(MailQuotaError::ZeroOrNegative);
        }
        Ok(Self(bytes))
    }

    pub fn bytes(&self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PasswordHashError {
    InvalidFormat,
}

impl fmt::Display for PasswordHashError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidFormat => write!(f, "Password hash must be in argon2id format"),
        }
    }
}
impl std::error::Error for PasswordHashError {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PasswordHash(String);

impl PasswordHash {
    pub fn new(hash: &str) -> Result<Self, PasswordHashError> {
        if !hash.starts_with("$argon2id$") {
            return Err(PasswordHashError::InvalidFormat);
        }
        Ok(Self(hash.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AccountState {
    Active,
    Suspended,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MailboxAccount {
    pub id: Uuid,
    pub email: EmailAddress,
    pub password_hash: PasswordHash,
    pub quota: MailQuota,
    pub state: AccountState,
}

impl MailboxAccount {
    pub fn new(
        id: Uuid,
        email: EmailAddress,
        password_hash: PasswordHash,
        quota: MailQuota,
    ) -> Self {
        Self {
            id,
            email,
            password_hash,
            quota,
            state: AccountState::Active,
        }
    }
}
