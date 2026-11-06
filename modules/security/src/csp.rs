#![forbid(unsafe_code)]

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
    #[error("CSP policy must contain at least one directive or a report URI")]
    EmptyPolicy,
    #[error("CSP report URI is empty or contains invalid characters")]
    InvalidReportUri,
    #[error("Value '{0}' in directive '{1}' is unsafe without a nonce or hash (e.g. 'unsafe-inline' without nonce)")]
    UnsafeInline(String, String),
    #[error("Value '{0}' in directive '{1}' contains invalid characters")]
    InvalidCharacters(String, String),
}

impl CspPolicy {
    pub fn validate(&self) -> Result<(), CspError> {
        if self.directives.is_empty() && self.report_uri.is_none() {
            return Err(CspError::EmptyPolicy);
        }

        for directive in &self.directives {
            if !is_supported_directive_name(&directive.name) {
                return Err(CspError::InvalidDirectiveName(directive.name.clone()));
            }

            let mut has_unsafe_inline = false;
            let mut has_nonce_or_hash = false;

            for value in &directive.values {
                if value.contains('\n')
                    || value.contains('\r')
                    || value.contains(';')
                    || value.contains(' ')
                    || value.contains('\t')
                {
                    if value.contains("'unsafe-inline'") {
                        return Err(CspError::UnsafeInline(
                            "'unsafe-inline'".into(),
                            directive.name.clone(),
                        ));
                    }
                    return Err(CspError::InvalidCharacters(
                        value.clone(),
                        directive.name.clone(),
                    ));
                }

                let token = value;
                if token == "'unsafe-inline'" {
                    has_unsafe_inline = true;
                } else if token == "'unsafe-eval'"
                    || token == "'strict-dynamic'"
                    || token == "'unsafe-hashes'"
                    || token == "'report-sample'"
                    || token == "'unsafe-allow-redirects'"
                    || token == "'wasm-unsafe-eval'"
                    || token == "'self'"
                    || token == "'none'"
                    || token.starts_with("http://")
                    || token.starts_with("https://")
                    || token.starts_with("ws://")
                    || token.starts_with("wss://")
                    || token.starts_with("data:")
                    || token.starts_with("blob:")
                    || token.starts_with("mediastream:")
                    || token.starts_with("filesystem:")
                    || !token.starts_with('\'')
                    || token == "*"
                {
                    // Do nothing, valid token
                } else if let Some(rest) = token.strip_prefix("'nonce-") {
                    if let Some(b64) = rest.strip_suffix('\'') {
                        let is_valid_base64 = !b64.is_empty()
                            && b64.chars().all(|c| {
                                c.is_ascii_alphanumeric()
                                    || c == '+'
                                    || c == '/'
                                    || c == '-'
                                    || c == '_'
                                    || c == '='
                            });
                        if !is_valid_base64 {
                            return Err(CspError::InvalidCharacters(
                                value.clone(),
                                directive.name.clone(),
                            ));
                        }
                        has_nonce_or_hash = true;
                    } else {
                        return Err(CspError::InvalidCharacters(
                            value.clone(),
                            directive.name.clone(),
                        ));
                    }
                } else if let Some(rest) = token
                    .strip_prefix("'sha256-")
                    .or_else(|| token.strip_prefix("'sha384-"))
                    .or_else(|| token.strip_prefix("'sha512-"))
                {
                    if let Some(b64) = rest.strip_suffix('\'') {
                        let is_valid_base64 = !b64.is_empty()
                            && b64.chars().all(|c| {
                                c.is_ascii_alphanumeric()
                                    || c == '+'
                                    || c == '/'
                                    || c == '-'
                                    || c == '_'
                                    || c == '='
                            });
                        if !is_valid_base64 {
                            return Err(CspError::InvalidCharacters(
                                value.clone(),
                                directive.name.clone(),
                            ));
                        }
                        has_nonce_or_hash = true;
                    } else {
                        return Err(CspError::InvalidCharacters(
                            value.clone(),
                            directive.name.clone(),
                        ));
                    }
                } else {
                    return Err(CspError::InvalidCharacters(
                        value.clone(),
                        directive.name.clone(),
                    ));
                }
            }

            if has_unsafe_inline && !has_nonce_or_hash {
                return Err(CspError::UnsafeInline(
                    "'unsafe-inline'".into(),
                    directive.name.clone(),
                ));
            }
        }

        if self.report_uri.as_deref().is_some_and(|uri| {
            uri.is_empty()
                || uri
                    .bytes()
                    .any(|byte| byte <= b' ' || byte == b';' || byte == 0x7f)
        }) {
            return Err(CspError::InvalidReportUri);
        }

        Ok(())
    }

    pub fn generate_header_value(&self) -> Result<String, CspError> {
        self.validate()?;

        let mut parts = Vec::new();
        for directive in &self.directives {
            // CSP directive names are ASCII case-insensitive. Canonicalizing
            // them makes the emitted header stable and easier to audit.
            let mut part = directive.name.to_ascii_lowercase();
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

// CSP directive names are case-insensitive, but a typo is silently ignored
// by user agents. Accept the core CSP Level 3 names plus established CSP
// extensions and legacy names, and reject all other tokens before rendering.
fn is_supported_directive_name(name: &str) -> bool {
    const SUPPORTED_DIRECTIVES: &[&str] = &[
        "base-uri",
        "block-all-mixed-content",
        "child-src",
        "connect-src",
        "default-src",
        "font-src",
        "form-action",
        "frame-ancestors",
        "frame-src",
        "img-src",
        "manifest-src",
        "media-src",
        "navigate-to",
        "object-src",
        "plugin-types",
        "prefetch-src",
        "report-to",
        "report-uri",
        "require-sri-for",
        "require-trusted-types-for",
        "sandbox",
        "script-src",
        "script-src-attr",
        "script-src-elem",
        "style-src",
        "style-src-attr",
        "style-src-elem",
        "trusted-types",
        "upgrade-insecure-requests",
        "webrtc",
        "worker-src",
    ];

    name.is_ascii()
        && SUPPORTED_DIRECTIVES
            .iter()
            .any(|supported| name.eq_ignore_ascii_case(supported))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_csp() {
        let policy = CspPolicy {
            directives: vec![CspDirective {
                name: "default-src".into(),
                values: vec!["'self'".into()],
            }],
            report_only: false,
            report_uri: None,
        };
        assert!(policy.validate().is_ok());
        assert_eq!(
            policy.generate_header_value().unwrap(),
            "default-src 'self'"
        );
    }

    #[test]
    fn accepts_supported_directives_case_insensitively_and_canonicalizes_output() {
        let policy = CspPolicy {
            directives: vec![CspDirective {
                name: "ScRiPt-SrC".into(),
                values: vec!["'self'".into()],
            }],
            report_only: false,
            report_uri: None,
        };

        assert!(policy.validate().is_ok());
        assert_eq!(policy.generate_header_value().unwrap(), "script-src 'self'");
    }

    #[test]
    fn rejects_unknown_or_misspelled_directives() {
        for name in ["default-srcx", "x-custom-policy", "not a directive"] {
            let policy = CspPolicy {
                directives: vec![CspDirective {
                    name: name.into(),
                    values: vec!["'self'".into()],
                }],
                report_only: false,
                report_uri: None,
            };

            assert!(matches!(
                policy.validate(),
                Err(CspError::InvalidDirectiveName(_))
            ));
            assert!(matches!(
                policy.generate_header_value(),
                Err(CspError::InvalidDirectiveName(_))
            ));
        }
    }

    #[test]
    fn rejects_empty_csp_policy() {
        let policy = CspPolicy {
            directives: vec![],
            report_only: false,
            report_uri: None,
        };

        assert!(matches!(policy.validate(), Err(CspError::EmptyPolicy)));
        assert!(matches!(
            policy.generate_header_value(),
            Err(CspError::EmptyPolicy)
        ));
    }

    #[test]
    fn accepts_report_uri_only_policy_and_rejects_empty_or_injected_report_uri() {
        let valid = CspPolicy {
            directives: vec![],
            report_only: true,
            report_uri: Some("/csp-violations".into()),
        };
        assert_eq!(
            valid.generate_header_value().unwrap(),
            "report-uri /csp-violations"
        );
        assert_eq!(valid.header_name(), "Content-Security-Policy-Report-Only");

        for uri in ["", "/report; default-src *", "/report\r\nX-Foo: bar"] {
            let policy = CspPolicy {
                directives: vec![],
                report_only: false,
                report_uri: Some(uri.into()),
            };
            assert!(matches!(policy.validate(), Err(CspError::InvalidReportUri)));
        }
    }

    #[test]
    fn test_unsafe_inline() {
        let policy = CspPolicy {
            directives: vec![CspDirective {
                name: "script-src".into(),
                values: vec!["'unsafe-inline'".into()],
            }],
            report_only: false,
            report_uri: None,
        };
        assert!(matches!(
            policy.validate(),
            Err(CspError::UnsafeInline(_, _))
        ));
    }

    #[test]
    fn test_unsafe_inline_mitigated() {
        let policy = CspPolicy {
            directives: vec![CspDirective {
                name: "script-src".into(),
                values: vec!["'unsafe-inline'".into(), "'nonce-123'".into()],
            }],
            report_only: false,
            report_uri: None,
        };
        assert!(policy.validate().is_ok()); // Should pass because nonce is present
    }

    #[test]
    fn test_invalid_chars() {
        let policy = CspPolicy {
            directives: vec![CspDirective {
                name: "default-src".into(),
                values: vec!["'self'; attack".into()],
            }],
            report_only: false,
            report_uri: None,
        };
        assert!(matches!(
            policy.validate(),
            Err(CspError::InvalidCharacters(_, _))
        ));
    }

    #[test]
    fn test_unsafe_inline_with_trailing_whitespace_is_caught() {
        let policy = CspPolicy {
            directives: vec![CspDirective {
                name: "script-src".into(),
                values: vec!["'unsafe-inline'   ".into()],
            }],
            report_only: false,
            report_uri: None,
        };
        assert!(matches!(
            policy.validate(),
            Err(CspError::UnsafeInline(_, _))
        ));
    }

    #[test]
    fn test_invalid_nonce_syntax_rejected() {
        let policy = CspPolicy {
            directives: vec![CspDirective {
                name: "script-src".into(),
                values: vec!["'nonce-123".into()], // Missing closing quote
            }],
            report_only: false,
            report_uri: None,
        };
        assert!(matches!(
            policy.validate(),
            Err(CspError::InvalidCharacters(_, _))
        ));
    }

    #[test]
    fn test_invalid_hash_syntax_rejected() {
        let policy = CspPolicy {
            directives: vec![CspDirective {
                name: "script-src".into(),
                values: vec!["'sha256-abc".into()], // Missing closing quote
            }],
            report_only: false,
            report_uri: None,
        };
        assert!(matches!(
            policy.validate(),
            Err(CspError::InvalidCharacters(_, _))
        ));
    }
}
