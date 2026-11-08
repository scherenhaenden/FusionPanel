use crate::domain::error::{DomainError, Result};
use std::convert::TryFrom;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct DomainName(String);

impl DomainName {
    pub fn new(name: impl Into<String>) -> Result<Self> {
        let name = name.into();

        if name.is_empty() {
            return Err(DomainError::InvalidInput(
                "Domain name cannot be empty".into(),
            ));
        }

        if name.len() > 253 {
            return Err(DomainError::InvalidInput("Domain name too long".into()));
        }

        if name.chars().any(|c| c > '\x7F') {
            return Err(DomainError::Unsupported(
                "IDNA/Unicode domains are not supported".into(),
            ));
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
                return Err(DomainError::InvalidInput(
                    "Label too long (max 63 chars)".into(),
                ));
            }
            if label.starts_with('-') || label.ends_with('-') {
                return Err(DomainError::InvalidInput(
                    "Label cannot start or end with hyphen".into(),
                ));
            }
            if !label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
                return Err(DomainError::InvalidInput(
                    "Invalid characters in label".into(),
                ));
            }
        }

        // Disallow bare IPs
        if name.parse::<std::net::IpAddr>().is_ok() {
            return Err(DomainError::InvalidInput(
                "IP literals not allowed for domain name".into(),
            ));
        }

        Ok(Self(name.to_lowercase()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for DomainName {
    type Error = DomainError;

    fn try_from(value: String) -> Result<Self> {
        Self::new(value)
    }
}

impl From<DomainName> for String {
    fn from(value: DomainName) -> Self {
        value.0
    }
}

pub fn validate_document_root(path_str: &str) -> Result<()> {
    if !path_str.starts_with("/srv/fusionpanel/sites/") {
        return Err(DomainError::InvalidInput(
            "Path must be under /srv/fusionpanel/sites/".into(),
        ));
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

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct DocumentRoot(String);

impl TryFrom<String> for DocumentRoot {
    type Error = DomainError;

    fn try_from(s: String) -> Result<Self> {
        validate_document_root(&s)?;
        Ok(Self(s))
    }
}

impl From<DocumentRoot> for String {
    fn from(val: DocumentRoot) -> Self {
        val.0
    }
}

impl DocumentRoot {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct PoolName(String);

impl TryFrom<String> for PoolName {
    type Error = DomainError;

    fn try_from(s: String) -> Result<Self> {
        if s.is_empty() || s.len() > 64 {
            return Err(DomainError::InvalidInput("Invalid pool name length".into()));
        }
        if !s
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        {
            return Err(DomainError::InvalidInput(
                "Invalid characters in pool name".into(),
            ));
        }
        Ok(Self(s))
    }
}

impl From<PoolName> for String {
    fn from(val: PoolName) -> Self {
        val.0
    }
}

impl PoolName {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct PhpVersion(String);

impl TryFrom<String> for PhpVersion {
    type Error = DomainError;

    fn try_from(s: String) -> Result<Self> {
        if s.is_empty() || s.len() > 10 {
            return Err(DomainError::InvalidInput(
                "Invalid PHP version length".into(),
            ));
        }
        if !s.chars().all(|c| c.is_ascii_digit() || c == '.') {
            return Err(DomainError::InvalidInput(
                "Invalid characters in PHP version".into(),
            ));
        }
        Ok(Self(s))
    }
}

impl From<PhpVersion> for String {
    fn from(val: PhpVersion) -> Self {
        val.0
    }
}

impl PhpVersion {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct MemoryLimit(String);

impl TryFrom<String> for MemoryLimit {
    type Error = DomainError;

    fn try_from(s: String) -> Result<Self> {
        if s.is_empty() || s.len() > 10 {
            return Err(DomainError::InvalidInput(
                "Invalid memory limit length".into(),
            ));
        }
        if !s
            .chars()
            .all(|c| c.is_ascii_digit() || c == 'M' || c == 'G' || c == 'K')
        {
            return Err(DomainError::InvalidInput(
                "Invalid characters in memory limit".into(),
            ));
        }
        Ok(Self(s))
    }
}

impl From<MemoryLimit> for String {
    fn from(val: MemoryLimit) -> Self {
        val.0
    }
}

impl MemoryLimit {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "u16", into = "u16")]
pub struct Port(u16);

impl TryFrom<u16> for Port {
    type Error = DomainError;

    fn try_from(p: u16) -> Result<Self> {
        if p < 1024 {
            return Err(DomainError::InvalidInput(
                "Port must be >= 1024 to avoid privileged ports".into(),
            ));
        }
        Ok(Self(p))
    }
}

impl From<Port> for u16 {
    fn from(val: Port) -> Self {
        val.0
    }
}

impl Port {
    pub fn as_u16(&self) -> u16 {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct UpstreamUrl(String);

impl TryFrom<String> for UpstreamUrl {
    type Error = DomainError;

    fn try_from(s: String) -> Result<Self> {
        if !s.starts_with("http://") && !s.starts_with("https://") {
            return Err(DomainError::InvalidInput(
                "Upstream URL must start with http:// or https://".into(),
            ));
        }
        if s.contains(|c: char| c.is_whitespace() || c == '\0' || c == '\n' || c == '\r') {
            return Err(DomainError::InvalidInput(
                "Upstream URL contains invalid characters".into(),
            ));
        }
        Ok(Self(s))
    }
}

impl From<UpstreamUrl> for String {
    fn from(val: UpstreamUrl) -> Self {
        val.0
    }
}

impl UpstreamUrl {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct OsUsername(String);

impl TryFrom<String> for OsUsername {
    type Error = DomainError;

    fn try_from(s: String) -> Result<Self> {
        if s.is_empty() {
            return Err(DomainError::InvalidInput(
                "OS username cannot be empty".into(),
            ));
        }
        if s.len() > 32 {
            return Err(DomainError::InvalidInput(
                "OS username too long (max 32 chars)".into(),
            ));
        }
        if s.starts_with('-') {
            return Err(DomainError::InvalidInput(
                "OS username cannot start with a hyphen".into(),
            ));
        }
        if !s
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            return Err(DomainError::InvalidInput(
                "OS username contains invalid characters".into(),
            ));
        }
        let reserved = [
            "root",
            "daemon",
            "bin",
            "sys",
            "sync",
            "games",
            "man",
            "lp",
            "mail",
            "news",
            "uucp",
            "proxy",
            "www-data",
            "backup",
            "list",
            "irc",
            "gnats",
            "nobody",
            "systemd-network",
            "systemd-resolve",
            "systemd-timesync",
            "messagebus",
            "sshd",
        ];
        if reserved.contains(&s.as_str()) {
            return Err(DomainError::InvalidInput("Protected system account".into()));
        }
        Ok(Self(s))
    }
}

impl From<OsUsername> for String {
    fn from(val: OsUsername) -> Self {
        val.0
    }
}

impl OsUsername {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn normalized(&self) -> String {
        self.0.replace("-", "_")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum BindingReadWritePolicy {
    ReadOnly,
    ReadWrite,
}
