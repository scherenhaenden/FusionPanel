use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CspDirective {
    pub name: String,
    pub values: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CspPolicy {
    pub directives: Vec<CspDirective>,
    pub report_only: bool,
    pub report_uri: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum CspError {
    #[error("Directive name '{0}' is invalid")]
    InvalidDirectiveName(String),
    #[error("Value '{0}' in directive '{1}' is unsafe without a nonce or hash (e.g. 'unsafe-inline' without nonce)")]
    UnsafeInline(String, String),
    #[error("Value '{0}' in directive '{1}' contains invalid characters")]
    InvalidCharacters(String, String),
}

impl CspPolicy {
    pub fn validate(&self) -> Result<(), CspError> {
        for directive in &self.directives {
            if directive.name.is_empty() || !directive.name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
                return Err(CspError::InvalidDirectiveName(directive.name.clone()));
            }

            let mut has_unsafe_inline = false;
            let mut has_nonce_or_hash = false;

            for value in &directive.values {
                if value.contains('\n') || value.contains('\r') || value.contains(';') {
                    return Err(CspError::InvalidCharacters(value.clone(), directive.name.clone()));
                }

                if value == "'unsafe-inline'" {
                    has_unsafe_inline = true;
                }

                if value.starts_with("'nonce-") || value.starts_with("'sha256-") || value.starts_with("'sha384-") || value.starts_with("'sha512-") {
                    has_nonce_or_hash = true;
                }
            }

            if has_unsafe_inline && !has_nonce_or_hash {
                return Err(CspError::UnsafeInline("'unsafe-inline'".into(), directive.name.clone()));
            }
        }

        Ok(())
    }

    pub fn generate_header_value(&self) -> Result<String, CspError> {
        self.validate()?;

        let mut parts = Vec::new();
        for directive in &self.directives {
            let mut part = directive.name.clone();
            if !directive.values.is_empty() {
                part.push(' ');
                part.push_str(&directive.values.join(" "));
            }
            parts.push(part);
        }

        if let Some(ref uri) = self.report_uri {
            parts.push(format!("report-uri {}", uri));
        }

        Ok(parts.join("; "))
    }

    pub fn header_name(&self) -> &'static str {
        if self.report_only {
            "Content-Security-Policy-Report-Only"
        } else {
            "Content-Security-Policy"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_csp() {
        let policy = CspPolicy {
            directives: vec![
                CspDirective {
                    name: "default-src".into(),
                    values: vec!["'self'".into()],
                }
            ],
            report_only: false,
            report_uri: None,
        };
        assert!(policy.validate().is_ok());
        assert_eq!(policy.generate_header_value().unwrap(), "default-src 'self'");
    }

    #[test]
    fn test_unsafe_inline() {
        let policy = CspPolicy {
            directives: vec![
                CspDirective {
                    name: "script-src".into(),
                    values: vec!["'unsafe-inline'".into()],
                }
            ],
            report_only: false,
            report_uri: None,
        };
        assert!(matches!(policy.validate(), Err(CspError::UnsafeInline(_, _))));
    }

    #[test]
    fn test_unsafe_inline_mitigated() {
        let policy = CspPolicy {
            directives: vec![
                CspDirective {
                    name: "script-src".into(),
                    values: vec!["'unsafe-inline'".into(), "'nonce-123'".into()],
                }
            ],
            report_only: false,
            report_uri: None,
        };
        assert!(policy.validate().is_ok()); // Should pass because nonce is present
    }

    #[test]
    fn test_invalid_chars() {
        let policy = CspPolicy {
            directives: vec![
                CspDirective {
                    name: "default-src".into(),
                    values: vec!["'self'; attack".into()],
                }
            ],
            report_only: false,
            report_uri: None,
        };
        assert!(matches!(policy.validate(), Err(CspError::InvalidCharacters(_, _))));
    }
}
