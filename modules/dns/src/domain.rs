use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;
use uuid::Uuid;

macro_rules! define_uuid_id {
    ($name:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(Uuid);

        impl $name {
            pub fn new() -> Self {
                Self(Uuid::new_v4())
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(f)
            }
        }
    };
}

define_uuid_id!(NodeId);
define_uuid_id!(SiteId);
define_uuid_id!(TenantId);
define_uuid_id!(JobId);

#[derive(Debug, Error)]
pub enum DomainError {
    #[error("Invalid DNS label: {0}")]
    InvalidLabel(String),
    #[error("Label too long (max 63 bytes)")]
    LabelTooLong,
    #[error("Domain name too long (max 253 bytes)")]
    DomainTooLong,
    #[error("IP literal unsupported as domain name")]
    IpLiteralUnsupported,
    #[error("IDNA/Unicode domains are unsupported")]
    IdnaUnsupported,
    #[error("Invalid TTL value")]
    InvalidTtl,
    #[error("Invalid Record data: {0}")]
    InvalidRecordData(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DomainName(String);

impl DomainName {
    pub fn parse(name: &str) -> Result<Self, DomainError> {
        if name.is_empty() || name == "." {
            return Err(DomainError::InvalidLabel("Empty domain".to_string()));
        }

        if name.len() > 253 {
            return Err(DomainError::DomainTooLong);
        }

        if !name.is_ascii() {
            return Err(DomainError::IdnaUnsupported);
        }

        if name.parse::<std::net::IpAddr>().is_ok() {
            return Err(DomainError::IpLiteralUnsupported);
        }

        let trimmed_name = name.trim_end_matches('.');

        for label in trimmed_name.split('.') {
            if label.is_empty() {
                return Err(DomainError::InvalidLabel("Empty label".to_string()));
            }
            if label.len() > 63 {
                return Err(DomainError::LabelTooLong);
            }
            if !label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
                return Err(DomainError::InvalidLabel(label.to_string()));
            }
            if label.starts_with('-') || label.ends_with('-') {
                return Err(DomainError::InvalidLabel("Label cannot start or end with hyphen".to_string()));
            }
        }

        Ok(Self(trimmed_name.to_lowercase()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for DomainName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum RecordType {
    A,
    AAAA,
    CNAME,
    MX,
    TXT,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Record {
    pub name: DomainName,
    pub record_type: RecordType,
    pub ttl: u32,
    pub data: String,
}

impl Record {
    pub fn new(name: DomainName, record_type: RecordType, ttl: u32, data: String) -> Result<Self, DomainError> {
        if ttl == 0 || ttl > 2147483647 {
            return Err(DomainError::InvalidTtl);
        }

        // Basic record data validation
        match record_type {
            RecordType::A => {
                data.parse::<std::net::Ipv4Addr>().map_err(|_| DomainError::InvalidRecordData("Invalid IPv4 address".to_string()))?;
            }
            RecordType::AAAA => {
                data.parse::<std::net::Ipv6Addr>().map_err(|_| DomainError::InvalidRecordData("Invalid IPv6 address".to_string()))?;
            }
            RecordType::CNAME => {
                DomainName::parse(&data)?;
            }
            RecordType::MX => {
                let parts: Vec<&str> = data.split_whitespace().collect();
                if parts.len() != 2 {
                    return Err(DomainError::InvalidRecordData("MX record must have priority and target".to_string()));
                }
                parts[0].parse::<u16>().map_err(|_| DomainError::InvalidRecordData("Invalid MX priority".to_string()))?;
                DomainName::parse(parts[1])?;
            }
            RecordType::TXT => {
                if data.is_empty() || data.len() > 255 {
                    return Err(DomainError::InvalidRecordData("TXT record data must be 1-255 characters".to_string()));
                }
            }
        }

        Ok(Self {
            name,
            record_type,
            ttl,
            data,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Zone {
    pub id: Uuid,
    pub name: DomainName,
    pub tenant_id: TenantId,
    pub records: Vec<Record>,
}

impl Zone {
    pub fn new(name: DomainName, tenant_id: TenantId) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            tenant_id,
            records: Vec::new(),
        }
    }

    pub fn add_record(&mut self, record: Record) {
        self.records.push(record);
    }
}
