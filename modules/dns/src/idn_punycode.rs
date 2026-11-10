#![forbid(unsafe_code)]

use thiserror::Error;
use idna::domain_to_ascii;
use idna::domain_to_unicode;

#[derive(Error, Debug, PartialEq)]
pub enum IdnaError {
    #[error("Label exceeds maximum length of 63 octets")]
    LabelTooLong,
    #[error("Domain exceeds maximum length of 253 octets")]
    DomainTooLong,
    #[error("Invalid hyphen placement in label")]
    InvalidHyphenPlacement,
    #[error("Homoglyph/mixed-script detected in domain")]
    HomoglyphDetected,
    #[error("IDNA conversion failed: {0}")]
    ConversionError(String),
}

pub struct IdnaConverter;

impl IdnaConverter {
    pub fn to_ascii(domain: &str) -> Result<String, IdnaError> {
        // Homoglyph / Mixed-script guard
        // A simple heuristic for mixed script:
        // We'll check if it mixes characters from different scripts.
        // For simplicity, we check if it mixes Cyrillic and Latin in the same domain.
        // (A more comprehensive check would use Unicode Script properties, but we can do a basic check).
        let has_latin = domain.chars().any(|c| matches!(c, 'a'..='z' | 'A'..='Z'));
        // Cyrillic block is U+0400 to U+04FF and U+0500 to U+052F
        let has_cyrillic = domain.chars().any(|c| (c >= '\u{0400}' && c <= '\u{04FF}') || (c >= '\u{0500}' && c <= '\u{052F}'));

        let has_greek = domain.chars().any(|c| c >= '\u{0370}' && c <= '\u{03FF}');

        if has_latin && (has_cyrillic || has_greek) {
            return Err(IdnaError::HomoglyphDetected);
        }

        // Convert to ASCII using idna
        let ascii_domain = domain_to_ascii(domain)
            .map_err(|e| IdnaError::ConversionError(format!("{:?}", e)))?;

        // Total domain length check (RFC 5890)
        // Root domain trailing dot is not always included in string length, but typically
        // total length must be <= 253 octets.
        let length = if ascii_domain.ends_with('.') {
            ascii_domain.len() - 1
        } else {
            ascii_domain.len()
        };

        if length > 253 {
            return Err(IdnaError::DomainTooLong);
        }

        // Label-level validation
        let labels = ascii_domain.split('.');
        for label in labels {
            if label.is_empty() {
                continue; // Root or trailing dot
            }
            if label.len() > 63 {
                return Err(IdnaError::LabelTooLong);
            }

            // Hyphen rules
            if label.starts_with('-') || label.ends_with('-') {
                return Err(IdnaError::InvalidHyphenPlacement);
            }

            // Punycode prefixes (xn--) are allowed to have hyphens at 3 and 4, but otherwise
            // RFC 5890 restricts them depending on IDNA. For general labels, hyphens at 3 and 4
            // are disallowed unless it's xn--.
            if label.len() >= 4 && &label[2..4] == "--" && !label.starts_with("xn--") && !label.starts_with("XN--") {
                return Err(IdnaError::InvalidHyphenPlacement);
            }
        }

        Ok(ascii_domain)
    }

    pub fn to_unicode(domain: &str) -> String {
        let (unicode, _errs) = domain_to_unicode(domain);
        unicode
    }
}
