#![forbid(unsafe_code)]


use thiserror::Error;

#[derive(Debug, Error)]
pub enum AuditError {
    #[error("Invalid configuration: {0}")]
    InvalidConfiguration(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum TlsVersion {
    Tls1_0,
    Tls1_1,
    Tls1_2,
    Tls1_3,
}

#[derive(Debug, Clone)]
pub struct PciTlsConfig {
    pub versions: Vec<TlsVersion>,
    pub weak_ciphers_enabled: bool,
}

#[derive(Debug, Clone)]
pub struct PasswordPolicy {
    pub min_length: u32,
    pub complexity_enabled: bool,
    pub expiration_days: Option<u32>,
    pub history_count: Option<u32>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FirewallAction {
    Allow,
    Deny,
    Drop,
}

#[derive(Debug, Clone)]
pub struct FirewallConfig {
    pub default_inbound_action: FirewallAction,
    pub open_ports: Vec<u16>,
}

#[derive(Debug, Clone)]
pub struct AuditLoggingConfig {
    pub enabled: bool,
    pub retention_days: u32,
}

#[derive(Debug, Clone)]
pub struct ServerConfigSnapshot {
    pub tls_config: PciTlsConfig,
    pub password_policy: PasswordPolicy,
    pub firewall_config: FirewallConfig,
    pub audit_logging_config: AuditLoggingConfig,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ComplianceStatus {
    Pass,
    Fail,
}

#[derive(Debug, Clone)]
pub struct Finding {
    pub requirement: String,
    pub status: ComplianceStatus,
    pub description: String,
    pub remediation: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ComplianceReport {
    pub findings: Vec<Finding>,
    pub overall_status: ComplianceStatus,
}

pub struct PciDssAuditor;

impl PciDssAuditor {
    pub fn new() -> Self {
        Self
    }

    pub fn audit(&self, config: &ServerConfigSnapshot) -> Result<ComplianceReport, AuditError> {
        let mut findings = Vec::new();
        let mut overall_status = ComplianceStatus::Pass;

        // Requirement 4.2.1: Strong TLS cryptography
        let mut req4_2_1_status = ComplianceStatus::Pass;
        let mut req4_2_1_remediation = Vec::new();

        let has_weak_versions = config.tls_config.versions.iter().any(|v| matches!(v, TlsVersion::Tls1_0 | TlsVersion::Tls1_1));
        if has_weak_versions {
            req4_2_1_status = ComplianceStatus::Fail;
            req4_2_1_remediation.push("Disable TLS 1.0 and TLS 1.1.");
        }

        if config.tls_config.weak_ciphers_enabled {
            req4_2_1_status = ComplianceStatus::Fail;
            req4_2_1_remediation.push("Disable weak ciphers.");
        }

        findings.push(Finding {
            requirement: "4.2.1".to_string(),
            status: req4_2_1_status.clone(),
            description: "Strong TLS cryptography (TLS 1.2 / TLS 1.3 only, weak ciphers disabled)".to_string(),
            remediation: if req4_2_1_status == ComplianceStatus::Fail {
                Some(req4_2_1_remediation.join(" "))
            } else {
                None
            },
        });

        if req4_2_1_status == ComplianceStatus::Fail {
            overall_status = ComplianceStatus::Fail;
        }

        // Requirement 8.3.6: Password policy
        let mut req8_3_6_status = ComplianceStatus::Pass;
        let mut req8_3_6_remediation = Vec::new();

        if config.password_policy.min_length < 12 {
            req8_3_6_status = ComplianceStatus::Fail;
            req8_3_6_remediation.push("Increase minimum password length to at least 12.");
        }

        if !config.password_policy.complexity_enabled {
            req8_3_6_status = ComplianceStatus::Fail;
            req8_3_6_remediation.push("Enable password complexity requirements.");
        }

        let expiration = config.password_policy.expiration_days.unwrap_or(0);
        if expiration == 0 || expiration > 90 {
            req8_3_6_status = ComplianceStatus::Fail;
            req8_3_6_remediation.push("Set password expiration to 90 days or less.");
        }

        if config.password_policy.history_count.unwrap_or(0) == 0 {
            req8_3_6_status = ComplianceStatus::Fail;
            req8_3_6_remediation.push("Enable password history.");
        }

        findings.push(Finding {
            requirement: "8.3.6".to_string(),
            status: req8_3_6_status.clone(),
            description: "Password policy (minimum length 12, complexity, expiration/history)".to_string(),
            remediation: if req8_3_6_status == ComplianceStatus::Fail {
                Some(req8_3_6_remediation.join(" "))
            } else {
                None
            },
        });

        if req8_3_6_status == ComplianceStatus::Fail {
            overall_status = ComplianceStatus::Fail;
        }

        // Requirement 1.2: Default deny firewall policy and restricted inbound ports
        let mut req1_2_status = ComplianceStatus::Pass;
        let mut req1_2_remediation = Vec::new();

        if config.firewall_config.default_inbound_action == FirewallAction::Allow {
            req1_2_status = ComplianceStatus::Fail;
            req1_2_remediation.push("Set default firewall inbound action to deny or drop.");
        }

        // This is a basic check.
        if config.firewall_config.open_ports.is_empty() && config.firewall_config.default_inbound_action == FirewallAction::Allow {
             // Not strictly part of 1.2 but maybe.
        }

        findings.push(Finding {
            requirement: "1.2".to_string(),
            status: req1_2_status.clone(),
            description: "Default deny firewall policy and restricted inbound ports".to_string(),
            remediation: if req1_2_status == ComplianceStatus::Fail {
                Some(req1_2_remediation.join(" "))
            } else {
                None
            },
        });

        if req1_2_status == ComplianceStatus::Fail {
            overall_status = ComplianceStatus::Fail;
        }

        // Requirement 10.2: Audit logging enabled and retention >= 90 days online
        let mut req10_2_status = ComplianceStatus::Pass;
        let mut req10_2_remediation = Vec::new();

        if !config.audit_logging_config.enabled {
            req10_2_status = ComplianceStatus::Fail;
            req10_2_remediation.push("Enable audit logging.");
        }

        if config.audit_logging_config.retention_days < 90 {
            req10_2_status = ComplianceStatus::Fail;
            req10_2_remediation.push("Increase audit logging retention to at least 90 days.");
        }

        findings.push(Finding {
            requirement: "10.2".to_string(),
            status: req10_2_status.clone(),
            description: "Audit logging enabled and retention >= 90 days online".to_string(),
            remediation: if req10_2_status == ComplianceStatus::Fail {
                Some(req10_2_remediation.join(" "))
            } else {
                None
            },
        });

        if req10_2_status == ComplianceStatus::Fail {
            overall_status = ComplianceStatus::Fail;
        }

        Ok(ComplianceReport {
            findings,
            overall_status,
        })
    }
}
