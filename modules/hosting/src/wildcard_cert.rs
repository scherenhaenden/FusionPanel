#![forbid(unsafe_code)]

use std::collections::{HashMap, HashSet};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum WildcardCertError {
    #[error("Invalid SAN FQDN syntax: {0}")]
    InvalidSyntax(String),
    #[error("Duplicate SAN detected: {0}")]
    DuplicateSan(String),
    #[error("Redundant SAN covered by wildcard: {0}")]
    RedundantSan(String),
}

/// A parsed Subject Alternative Name (SAN).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum San {
    Wildcard(String),
    Exact(String),
}

impl San {
    /// Parses and validates a SAN string.
    pub fn parse(san: &str) -> Result<Self, WildcardCertError> {
        if san.is_empty() {
            return Err(WildcardCertError::InvalidSyntax("empty".to_string()));
        }

        let is_wildcard = san.starts_with("*.");
        let fqdn = if is_wildcard { &san[2..] } else { san };

        if fqdn.is_empty() || fqdn.len() > 253 {
            return Err(WildcardCertError::InvalidSyntax("invalid length".to_string()));
        }

        let labels: Vec<&str> = fqdn.split('.').collect();
        if labels.is_empty() || labels.iter().any(|l| l.is_empty() || l.len() > 63) {
            return Err(WildcardCertError::InvalidSyntax("invalid label".to_string()));
        }

        for label in labels {
            if label.starts_with('-') || label.ends_with('-') {
                return Err(WildcardCertError::InvalidSyntax(
                    "label cannot start or end with a hyphen".to_string(),
                ));
            }
            if !label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
                return Err(WildcardCertError::InvalidSyntax(
                    "invalid characters in label".to_string(),
                ));
            }
        }

        if is_wildcard {
            Ok(San::Wildcard(fqdn.to_string()))
        } else {
            Ok(San::Exact(fqdn.to_string()))
        }
    }

    pub fn to_string(&self) -> String {
        match self {
            San::Wildcard(base) => format!("*.{}", base),
            San::Exact(fqdn) => fqdn.clone(),
        }
    }

    /// The base domain without wildcard
    pub fn base_domain(&self) -> &str {
        match self {
            San::Wildcard(base) => base.as_str(),
            San::Exact(fqdn) => fqdn.as_str(),
        }
    }

    /// The DNS TXT record name required for ACME DNS-01 challenge.
    pub fn challenge_domain(&self) -> String {
        format!("_acme-challenge.{}", self.base_domain())
    }
}

/// Validates a list of SANs, checking for redundancies, and groups them by required DNS challenge domains.
pub fn validate_and_group_sans(
    sans: &[&str],
) -> Result<HashMap<String, Vec<San>>, WildcardCertError> {
    let mut parsed_sans = Vec::new();
    let mut seen_sans = HashSet::new();

    for san_str in sans {
        let san = San::parse(san_str)?;

        if !seen_sans.insert(san.clone()) {
            return Err(WildcardCertError::DuplicateSan(san_str.to_string()));
        }

        parsed_sans.push(san);
    }

    let wildcards: HashSet<&str> = parsed_sans
        .iter()
        .filter_map(|s| match s {
            San::Wildcard(base) => Some(base.as_str()),
            _ => None,
        })
        .collect();

    for san in &parsed_sans {
        if let San::Exact(fqdn) = san {
            if let Some((_first_label, rest)) = fqdn.split_once('.') {
                if wildcards.contains(rest) {
                    return Err(WildcardCertError::RedundantSan(san.to_string()));
                }
            }
        }
    }

    let mut grouped = HashMap::new();
    for san in parsed_sans {
        let challenge = san.challenge_domain();
        grouped.entry(challenge).or_insert_with(Vec::new).push(san);
    }

    Ok(grouped)
}
