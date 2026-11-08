#![forbid(unsafe_code)]

use crate::domain::DomainName;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum CaaError {
    #[error("Invalid flag: {0}. Must be 0 or 128.")]
    InvalidFlag(u8),
    #[error("Unsupported tag: {0}. Must be 'issue', 'issuewild', or 'iodef'.")]
    UnsupportedTag(String),
    #[error("Invalid CA domain: {0}")]
    InvalidCaDomain(String),
    #[error("Invalid URI for iodef: {0}")]
    InvalidUri(String),
}

pub struct CaaRecordFormatter;

impl CaaRecordFormatter {
    pub fn format(
        domain: &str,
        flags: u8,
        tag: &str,
        value: &str,
    ) -> Result<String, CaaError> {
        if flags != 0 && flags != 128 {
            return Err(CaaError::InvalidFlag(flags));
        }

        match tag {
            "issue" | "issuewild" => {
                if let Err(_) = DomainName::parse(value) {
                    return Err(CaaError::InvalidCaDomain(value.to_string()));
                }
            }
            "iodef" => {
                if !value.starts_with("mailto:") && !value.starts_with("https:") && !value.starts_with("http:") {
                    return Err(CaaError::InvalidUri(value.to_string()));
                }
            }
            _ => {
                return Err(CaaError::UnsupportedTag(tag.to_string()));
            }
        }

        // BIND9 format: <domain>. IN CAA <flags> <tag> "<value>"
        let fqdn = if domain.ends_with('.') {
            domain.to_string()
        } else {
            format!("{}.", domain)
        };

        Ok(format!("{} IN CAA {} {} \"{}\"", fqdn, flags, tag, value))
    }
}
