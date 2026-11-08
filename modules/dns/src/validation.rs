#![forbid(unsafe_code)]

use crate::domain::DomainName;
use std::net::{Ipv4Addr, Ipv6Addr};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationError {
    InvalidIpv4,
    InvalidIpv6,
    InvalidCname,
    InvalidMx(String),
    InvalidTxt(String),
    InvalidCaa(String),
    InvalidSrv(String),
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidIpv4 => write!(f, "Invalid IPv4 address"),
            Self::InvalidIpv6 => write!(f, "Invalid IPv6 address"),
            Self::InvalidCname => write!(f, "Invalid CNAME target domain"),
            Self::InvalidMx(msg) => write!(f, "Invalid MX record: {}", msg),
            Self::InvalidTxt(msg) => write!(f, "Invalid TXT record: {}", msg),
            Self::InvalidCaa(msg) => write!(f, "Invalid CAA record: {}", msg),
            Self::InvalidSrv(msg) => write!(f, "Invalid SRV record: {}", msg),
        }
    }
}

impl std::error::Error for ValidationError {}

pub fn validate_a(data: &str) -> Result<(), ValidationError> {
    data.parse::<Ipv4Addr>()
        .map_err(|_| ValidationError::InvalidIpv4)?;
    Ok(())
}

pub fn validate_aaaa(data: &str) -> Result<(), ValidationError> {
    data.parse::<Ipv6Addr>()
        .map_err(|_| ValidationError::InvalidIpv6)?;
    Ok(())
}

pub fn validate_cname(data: &str) -> Result<(), ValidationError> {
    if data.is_empty() {
        return Err(ValidationError::InvalidCname);
    }

    // Delegate to existing DomainName parser for RFC compliance
    DomainName::parse(data).map_err(|_| ValidationError::InvalidCname)?;
    Ok(())
}

pub fn validate_mx(data: &str) -> Result<(), ValidationError> {
    let parts: Vec<&str> = data.split_whitespace().collect();
    if parts.len() != 2 {
        return Err(ValidationError::InvalidMx(
            "must have priority and target".to_string(),
        ));
    }

    let priority = parts[0]
        .parse::<u16>()
        .map_err(|_| ValidationError::InvalidMx("invalid priority".to_string()))?;

    // Null MX check
    if parts[1] == "." {
        if priority != 0 {
            return Err(ValidationError::InvalidMx(
                "Null MX must have priority 0".to_string(),
            ));
        }
    } else {
        DomainName::parse(parts[1])
            .map_err(|_| ValidationError::InvalidMx("invalid target domain".to_string()))?;
    }

    Ok(())
}

pub fn validate_txt(data: &str) -> Result<(), ValidationError> {
    if data.is_empty() {
        return Err(ValidationError::InvalidTxt("cannot be empty".to_string()));
    }

    // An unquoted string over 255 chars is invalid
    if !data.contains('"') {
        if data.len() > 255 {
            return Err(ValidationError::InvalidTxt(
                "chunk exceeds 255 characters".to_string(),
            ));
        }
        return Ok(());
    }

    let mut current_len = 0;
    let mut in_quotes = false;
    let mut chars = data.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '"' {
            if in_quotes {
                if current_len > 255 {
                    return Err(ValidationError::InvalidTxt(
                        "chunk exceeds 255 characters".to_string(),
                    ));
                }
                current_len = 0;
            }
            in_quotes = !in_quotes;
        } else if in_quotes {
            if c == '\\' {
                if chars.next().is_none() {
                    return Err(ValidationError::InvalidTxt(
                        "invalid escape sequence".to_string(),
                    ));
                }
            }
            current_len += 1;
            if current_len > 255 {
                return Err(ValidationError::InvalidTxt(
                    "chunk exceeds 255 characters".to_string(),
                ));
            }
        } else {
            if !c.is_whitespace() {
                return Err(ValidationError::InvalidTxt(
                    "unexpected character outside quotes".to_string(),
                ));
            }
        }
    }

    if in_quotes {
        return Err(ValidationError::InvalidTxt("unmatched quotes".to_string()));
    }

    Ok(())
}

pub fn validate_caa(data: &str) -> Result<(), ValidationError> {
    // RFC 6844: <flags> <tag> <value>
    let mut parts = data.split_whitespace();

    let flags_str = parts
        .next()
        .ok_or_else(|| ValidationError::InvalidCaa("missing flags".to_string()))?;
    let _flags: u8 = flags_str
        .parse()
        .map_err(|_| ValidationError::InvalidCaa("invalid flags".to_string()))?;

    let tag = parts
        .next()
        .ok_or_else(|| ValidationError::InvalidCaa("missing tag".to_string()))?;
    if !tag.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Err(ValidationError::InvalidCaa(
            "invalid tag characters".to_string(),
        ));
    }

    let remainder: Vec<&str> = parts.collect();
    if remainder.is_empty() {
        return Err(ValidationError::InvalidCaa("missing value".to_string()));
    }

    let value = remainder.join(" ");
    if !value.starts_with('"') || !value.ends_with('"') {
        return Err(ValidationError::InvalidCaa(
            "value must be quoted".to_string(),
        ));
    }

    Ok(())
}

pub fn validate_srv(data: &str) -> Result<(), ValidationError> {
    let parts: Vec<&str> = data.split_whitespace().collect();
    if parts.len() != 4 {
        return Err(ValidationError::InvalidSrv(
            "must have priority, weight, port, and target".to_string(),
        ));
    }

    parts[0]
        .parse::<u16>()
        .map_err(|_| ValidationError::InvalidSrv("invalid priority".to_string()))?;
    parts[1]
        .parse::<u16>()
        .map_err(|_| ValidationError::InvalidSrv("invalid weight".to_string()))?;
    parts[2]
        .parse::<u16>()
        .map_err(|_| ValidationError::InvalidSrv("invalid port".to_string()))?;

    DomainName::parse(parts[3])
        .map_err(|_| ValidationError::InvalidSrv("invalid target domain".to_string()))?;

    Ok(())
}
