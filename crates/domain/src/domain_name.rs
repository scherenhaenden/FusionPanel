use std::fmt;
use std::str::FromStr;
use serde::{Deserialize, Serialize};
use crate::error::DomainError;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct DomainName(String);

impl DomainName {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    fn is_alternate_ipv4(name: &str) -> bool {
        let labels: Vec<&str> = name.split('.').collect();
        if labels.is_empty() || labels.len() > 4 {
            return false;
        }
        labels.iter().all(|label| {
            if label.is_empty() {
                return false;
            }
            if let Some(hex) = label.strip_prefix("0x").or_else(|| label.strip_prefix("0X")) {
                !hex.is_empty() && hex.chars().all(|c| c.is_ascii_hexdigit())
            } else {
                label.chars().all(|c| c.is_ascii_digit())
            }
        })
    }

    fn validate(name: &str) -> Result<String, DomainError> {
        if name.is_empty() {
            return Err(DomainError::InvalidDomainName("Domain name cannot be empty".to_string()));
        }

        if name.len() > 253 {
            return Err(DomainError::InvalidDomainName("Domain name too long (max 253 bytes)".to_string()));
        }

        if !name.is_ascii() {
            return Err(DomainError::InvalidDomainName("Domain name must be ASCII (IDNA not supported)".to_string()));
        }

        let name = name.to_ascii_lowercase();

        // Ensure it doesn't parse as an IP address (standard IPv4/IPv6)
        if std::net::IpAddr::from_str(&name).is_ok() {
            return Err(DomainError::InvalidDomainName("Domain name cannot be an IP literal".to_string()));
        }

        // Reject alternate numeric IPv4 (e.g. 0x7f000001, 2130706433, 0177.0.0.1)
        if Self::is_alternate_ipv4(&name) {
            return Err(DomainError::InvalidDomainName("Domain name cannot be an alternate numeric IPv4 literal".to_string()));
        }

        let labels: Vec<&str> = name.split('.').collect();

        for label in &labels {
            if label.is_empty() {
                return Err(DomainError::InvalidDomainName("Empty label or consecutive separators".to_string()));
            }

            if label.len() > 63 {
                return Err(DomainError::InvalidDomainName("Label too long (max 63 bytes)".to_string()));
            }

            if !label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
                return Err(DomainError::InvalidDomainName("Invalid characters in label".to_string()));
            }

            if label.starts_with('-') || label.ends_with('-') {
                return Err(DomainError::InvalidDomainName("Label cannot start or end with a hyphen".to_string()));
            }
        }

        Ok(name)
    }
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
        assert_eq!(DomainName::try_from("example.com").unwrap().as_str(), "example.com");
        assert_eq!(DomainName::try_from("sub.example.com").unwrap().as_str(), "sub.example.com");
        assert_eq!(DomainName::try_from("EXAMple.cOm").unwrap().as_str(), "example.com"); // canonicalization
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

        // Alternate numeric IPv4
        assert!(DomainName::try_from("2130706433").is_err());
        assert!(DomainName::try_from("0x7f000001").is_err());
        assert!(DomainName::try_from("0177.0.0.1").is_err());
        assert!(DomainName::try_from("127.1").is_err());

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
