use crate::error::DomainError;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct DomainName(String);

impl DomainName {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    fn validate(name: &str) -> Result<String, DomainError> {
        if name.is_empty() {
            return Err(DomainError::InvalidDomainName(
                "Domain name cannot be empty".to_string(),
            ));
        }

        if name.len() > 253 {
            return Err(DomainError::InvalidDomainName(
                "Domain name too long (max 253 bytes)".to_string(),
            ));
        }

        if !name.is_ascii() {
            return Err(DomainError::InvalidDomainName(
                "Domain name must be ASCII (IDNA not supported)".to_string(),
            ));
        }

        let name = name.to_ascii_lowercase();

        // Some URL parsers accept non-canonical IPv4 forms such as `127.1`
        // and interpret them as loopback. Reject those forms before they can
        // be stored as domain names.
        if std::net::IpAddr::from_str(&name).is_ok() || is_ipv4_address_or_legacy_form(&name) {
            return Err(DomainError::InvalidDomainName(
                "Domain name cannot be an IP literal or alternate IPv4 form".to_string(),
            ));
        }

        let labels: Vec<&str> = name.split('.').collect();

        for label in &labels {
            if label.is_empty() {
                return Err(DomainError::InvalidDomainName(
                    "Empty label or consecutive separators".to_string(),
                ));
            }

            if label.len() > 63 {
                return Err(DomainError::InvalidDomainName(
                    "Label too long (max 63 bytes)".to_string(),
                ));
            }

            if !label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
                return Err(DomainError::InvalidDomainName(
                    "Invalid characters in label".to_string(),
                ));
            }

            if label.starts_with('-') || label.ends_with('-') {
                return Err(DomainError::InvalidDomainName(
                    "Label cannot start or end with a hyphen".to_string(),
                ));
            }
        }

        Ok(name)
    }
}

/// Returns true when `host` is a valid WHATWG-style IPv4 address, including
/// shortened, octal, and hexadecimal component forms.
fn is_ipv4_address_or_legacy_form(host: &str) -> bool {
    let parts: Vec<&str> = host.split('.').collect();
    if parts.is_empty() || parts.len() > 4 {
        return false;
    }

    let numbers: Option<Vec<u64>> = parts.iter().map(|part| parse_ipv4_number(part)).collect();
    let Some(numbers) = numbers else {
        return false;
    };

    if numbers[..numbers.len() - 1].iter().any(|part| *part > 255) {
        return false;
    }

    let remaining_bytes = 5 - numbers.len();
    let max_last_part = (1_u64 << (remaining_bytes * 8)) - 1;
    numbers[numbers.len() - 1] <= max_last_part
}

/// Parses one IPv4 component using decimal, octal, or hexadecimal notation.
fn parse_ipv4_number(part: &str) -> Option<u64> {
    if part.is_empty() {
        return None;
    }

    if let Some(hex) = part.strip_prefix("0x") {
        return if hex.is_empty() {
            Some(0)
        } else {
            u64::from_str_radix(hex, 16).ok()
        };
    }

    if part.len() > 1 && part.starts_with('0') {
        return u64::from_str_radix(&part[1..], 8).ok();
    }

    part.parse().ok()
}

impl TryFrom<String> for DomainName {
    type Error = DomainError;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        let validated = Self::validate(&s)?;
        Ok(Self(validated))
    }
}

impl TryFrom<&str> for DomainName {
    type Error = DomainError;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        let validated = Self::validate(s)?;
        Ok(Self(validated))
    }
}

impl FromStr for DomainName {
    type Err = DomainError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::try_from(s)
    }
}

impl From<DomainName> for String {
    fn from(d: DomainName) -> Self {
        d.0
    }
}

impl fmt::Display for DomainName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_domain_names() {
        assert_eq!(
            DomainName::try_from("example.com").unwrap().as_str(),
            "example.com"
        );
        assert_eq!(
            DomainName::try_from("sub.example.com").unwrap().as_str(),
            "sub.example.com"
        );
        assert_eq!(
            DomainName::try_from("EXAMple.cOm").unwrap().as_str(),
            "example.com"
        ); // canonicalization
        assert_eq!(DomainName::try_from("a-b.com").unwrap().as_str(), "a-b.com");
        assert_eq!(DomainName::try_from("123.com").unwrap().as_str(), "123.com");
    }

    #[test]
    fn test_invalid_domain_names() {
        assert!(DomainName::try_from("").is_err());
        assert!(DomainName::try_from("example..com").is_err()); // consecutive separators
        assert!(DomainName::try_from(".example.com").is_err()); // empty label at start
        assert!(DomainName::try_from("example.com.").is_err()); // empty label at end

        // IDNA unsupported
        assert!(DomainName::try_from("münchen.de").is_err());
        assert!(DomainName::try_from("рф.ru").is_err());

        // IP Literals
        assert!(DomainName::try_from("192.168.1.1").is_err());
        assert!(DomainName::try_from("127.0.0.1").is_err());

        // Alternate IPv4 spellings accepted by common URL parsers must not
        // pass as DNS names.
        for alternate_ip in ["127.1", "127.0.1", "2130706433", "0x7f.1", "0177.1"] {
            assert!(
                DomainName::try_from(alternate_ip).is_err(),
                "accepted alternate IPv4 form: {alternate_ip}"
            );
        }

        // invalid chars
        assert!(DomainName::try_from("example!.com").is_err());
        assert!(DomainName::try_from("ex ample.com").is_err());
        assert!(DomainName::try_from("example_com").is_err());

        // hyphens
        assert!(DomainName::try_from("-example.com").is_err());
        assert!(DomainName::try_from("example-.com").is_err());

        // length
        let long_label = "a".repeat(64) + ".com";
        assert!(DomainName::try_from(long_label).is_err());

        let mut long_domain = "a".repeat(63) + ".";
        long_domain.push_str(&"b".repeat(63));
        long_domain.push_str(".");
        long_domain.push_str(&"c".repeat(63));
        long_domain.push_str(".");
        long_domain.push_str(&"d".repeat(63));
        assert!(DomainName::try_from(long_domain).is_err()); // > 253 total
    }

    #[test]
    fn test_serde() {
        let domain: DomainName = "example.com".parse().unwrap();
        let json = serde_json::to_string(&domain).unwrap();
        assert_eq!(json, "\"example.com\"");

        let deserialized: DomainName = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, domain);

        // invalid serde
        let bad_json = "\"example!.com\"";
        assert!(serde_json::from_str::<DomainName>(bad_json).is_err());
    }
}
