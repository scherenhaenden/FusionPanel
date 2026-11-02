use crate::domain::{MailAddress, MailDomainName};
use crate::error::MailError;
use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

/// Secure wrapper for sensitive strings (e.g. passwords, relay secrets)
/// preventing unintentional leakage into Debug formats or serialization.
#[derive(Clone, PartialEq, Eq)]
pub struct RedactedSecret(String);

impl RedactedSecret {
    pub fn new(secret: impl Into<String>) -> Result<Self, MailError> {
        let s = secret.into();
        if s.is_empty() {
            return Err(MailError::InvalidInput(
                "Secret cannot be empty".to_string(),
            ));
        }
        // Disallow null bytes or line breaks to prevent config injection
        if s.chars().any(|c| c == '\0' || c == '\n' || c == '\r') {
            return Err(MailError::InvalidInput(
                "Secret cannot contain line breaks or null bytes".to_string(),
            ));
        }
        Ok(Self(s))
    }

    /// Expose secret safely only where strictly required (e.g. generating hashed auth records or authenticating)
    pub fn expose_secret(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for RedactedSecret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[REDACTED]")
    }
}

impl fmt::Display for RedactedSecret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[REDACTED]")
    }
}

impl Serialize for RedactedSecret {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str("[REDACTED]")
    }
}

/// Mailbox credentials custody holding password hash or password reference.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MailboxCredentials {
    pub mailbox_id: Uuid,
    /// Securely redacted secret / password reference
    pub password_hash: RedactedSecret,
    pub auth_mode: AuthMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuthMode {
    PasswdFile,
    DovecotSha512Crypt,
    Argon2id,
}

/// Mail domain configuration and service ownership.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MailDomain {
    pub id: Uuid,
    pub name: MailDomainName,
    pub tenant_id: Uuid,
    pub is_active: bool,
    pub dkim_enabled: bool,
    pub dkim_selector: Option<String>,
    pub spf_record: Option<String>,
}

impl MailDomain {
    pub fn new(name: MailDomainName, tenant_id: Uuid) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            tenant_id,
            is_active: true,
            dkim_enabled: false,
            dkim_selector: None,
            spf_record: None,
        }
    }

    pub fn with_dkim(mut self, selector: &str) -> Result<Self, MailError> {
        if selector.is_empty()
            || !selector
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        {
            return Err(MailError::InvalidInput(
                "DKIM selector must consist only of ASCII letters, digits, '_' and '-'".to_string(),
            ));
        }
        self.dkim_enabled = true;
        self.dkim_selector = Some(selector.to_string());
        Ok(self)
    }

    pub fn with_spf(mut self, spf: &str) -> Result<Self, MailError> {
        if spf.is_empty() || spf.contains('\n') || spf.contains('\r') {
            return Err(MailError::InvalidInput(
                "SPF record cannot contain line breaks".to_string(),
            ));
        }
        if !spf.starts_with("v=spf1") {
            return Err(MailError::InvalidInput(
                "SPF record must start with 'v=spf1'".to_string(),
            ));
        }
        self.spf_record = Some(spf.to_string());
        Ok(self)
    }
}

/// Hosted mailbox definition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mailbox {
    pub id: Uuid,
    pub domain_id: Uuid,
    pub address: MailAddress,
    pub quota_bytes: u64,
    pub is_active: bool,
    /// Sub-path inside mail spool root: e.g. "example.com/user/"
    pub maildir_subpath: String,
}

impl Mailbox {
    pub fn new(domain_id: Uuid, address: MailAddress, quota_bytes: u64) -> Result<Self, MailError> {
        // Enforce safe relative subpath (e.g. "example.com/username/")
        let local_part = address.local_part();
        let domain_str = address.domain().as_str();

        if local_part.contains('/') || local_part.contains('\\') || local_part.contains("..") {
            return Err(MailError::InvalidInput(
                "Local-part contains directory traversal characters".to_string(),
            ));
        }

        let maildir_subpath = format!("{}/{}/", domain_str, local_part);

        Ok(Self {
            id: Uuid::new_v4(),
            domain_id,
            address,
            quota_bytes,
            is_active: true,
            maildir_subpath,
        })
    }
}

/// Mail alias / forwarding definition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MailAlias {
    pub id: Uuid,
    pub source: MailAddress,
    pub destinations: Vec<MailAddress>,
    pub is_active: bool,
}

impl MailAlias {
    pub fn new(source: MailAddress, destinations: Vec<MailAddress>) -> Result<Self, MailError> {
        if destinations.is_empty() {
            return Err(MailError::InvalidInput(
                "Mail alias must have at least one destination".to_string(),
            ));
        }
        Ok(Self {
            id: Uuid::new_v4(),
            source,
            destinations,
            is_active: true,
        })
    }
}

/// Outgoing SMTP relay configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SmtpRelayConfig {
    pub id: Uuid,
    pub relay_host: String,
    pub relay_port: u16,
    pub username: Option<String>,
    pub auth_secret: Option<RedactedSecret>,
    pub tls_enabled: bool,
}

impl SmtpRelayConfig {
    pub fn new(relay_host: &str, relay_port: u16) -> Result<Self, MailError> {
        if relay_host.is_empty()
            || relay_host.contains('\n')
            || relay_host.contains('\r')
            || relay_host.contains(' ')
            || relay_host.contains(':')
        {
            return Err(MailError::InvalidInput("Invalid relay host".to_string()));
        }
        if relay_port == 0 {
            return Err(MailError::InvalidInput(
                "Relay port must be greater than 0".to_string(),
            ));
        }

        Ok(Self {
            id: Uuid::new_v4(),
            relay_host: relay_host.to_string(),
            relay_port,
            username: None,
            auth_secret: None,
            tls_enabled: true,
        })
    }

    pub fn with_credentials(
        mut self,
        username: &str,
        secret: RedactedSecret,
    ) -> Result<Self, MailError> {
        if username.is_empty()
            || username.contains(':')
            || username.contains('\n')
            || username.contains('\r')
        {
            return Err(MailError::InvalidInput(
                "Invalid relay username".to_string(),
            ));
        }
        self.username = Some(username.to_string());
        self.auth_secret = Some(secret);
        Ok(self)
    }
}
