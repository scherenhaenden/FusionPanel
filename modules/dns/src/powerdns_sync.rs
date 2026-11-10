#![forbid(unsafe_code)]
use serde::{Deserialize, Serialize};

/// Represents a single DNS record inside an RRset.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PdnsRecord {
    pub content: String,
    pub disabled: bool,
}

impl PdnsRecord {
    pub fn new(content: &str, disabled: bool) -> Self {
        Self {
            content: content.to_string(),
            disabled,
        }
    }
}

/// Represents the type of change to apply to an RRset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum PdnsChangeType {
    Replace,
    Delete,
}

/// Represents an RRset modification in the PowerDNS API.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PdnsRrset {
    pub name: String,
    #[serde(rename = "type")]
    pub record_type: String,
    pub ttl: u32,
    pub changetype: PdnsChangeType,
    pub records: Vec<PdnsRecord>,
}

impl PdnsRrset {
    pub fn replace(name: &str, record_type: &str, ttl: u32, records: Vec<PdnsRecord>) -> Self {
        Self {
            name: canonicalize_domain(name),
            record_type: record_type.to_string(),
            ttl,
            changetype: PdnsChangeType::Replace,
            records,
        }
    }

    pub fn delete(name: &str, record_type: &str) -> Self {
        Self {
            name: canonicalize_domain(name),
            record_type: record_type.to_string(),
            ttl: 0,
            changetype: PdnsChangeType::Delete,
            records: vec![],
        }
    }
}

/// Payload for PATCH /api/v1/servers/localhost/zones/<zone_id>
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PdnsZonePatch {
    pub rrsets: Vec<PdnsRrset>,
}

impl PdnsZonePatch {
    pub fn new(rrsets: Vec<PdnsRrset>) -> Self {
        Self { rrsets }
    }
}

/// Helper function to ensure canonical domain format (trailing dot).
pub fn canonicalize_domain(domain: &str) -> String {
    let trimmed = domain.trim();
    if trimmed.ends_with('.') {
        trimmed.to_string()
    } else {
        format!("{}.", trimmed)
    }
}
