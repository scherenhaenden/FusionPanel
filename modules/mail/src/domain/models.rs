use regex::Regex;
use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

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
