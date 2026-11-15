#![forbid(unsafe_code)]

use security::{
    AuditLoggingConfig, ComplianceStatus, FirewallAction, FirewallConfig, PasswordPolicy,
    PciDssAuditor, PciTlsConfig, ServerConfigSnapshot, TlsVersion,
};

#[test]
fn test_compliant_snapshot() {
    let snapshot = ServerConfigSnapshot {
        tls_config: PciTlsConfig {
            versions: vec![TlsVersion::Tls1_2, TlsVersion::Tls1_3],
            weak_ciphers_enabled: false,
        },
        password_policy: PasswordPolicy {
            min_length: 12,
            complexity_enabled: true,
            expiration_days: Some(90),
            history_count: Some(5),
        },
        firewall_config: FirewallConfig {
            default_inbound_action: FirewallAction::Deny,
            open_ports: vec![443],
        },
        audit_logging_config: AuditLoggingConfig {
            enabled: true,
            retention_days: 90,
        },
    };

    let auditor = PciDssAuditor::new();
    let report = auditor.audit(&snapshot).unwrap();

    assert_eq!(report.overall_status, ComplianceStatus::Pass);
    assert_eq!(report.findings.len(), 4);
    for finding in report.findings {
        assert_eq!(finding.status, ComplianceStatus::Pass);
        assert!(finding.remediation.is_none());
    }
}

#[test]
fn test_non_compliant_snapshot() {
    let snapshot = ServerConfigSnapshot {
        tls_config: PciTlsConfig {
            versions: vec![TlsVersion::Tls1_0, TlsVersion::Tls1_2],
            weak_ciphers_enabled: true,
        },
        password_policy: PasswordPolicy {
            min_length: 8,
            complexity_enabled: false,
            expiration_days: Some(180),
            history_count: None,
        },
        firewall_config: FirewallConfig {
            default_inbound_action: FirewallAction::Allow,
            open_ports: vec![80, 443],
        },
        audit_logging_config: AuditLoggingConfig {
            enabled: false,
            retention_days: 30,
        },
    };

    let auditor = PciDssAuditor::new();
    let report = auditor.audit(&snapshot).unwrap();

    assert_eq!(report.overall_status, ComplianceStatus::Fail);

    // Check TLS finding
    let tls_finding = report.findings.iter().find(|f| f.requirement == "4.2.1").unwrap();
    assert_eq!(tls_finding.status, ComplianceStatus::Fail);
    assert_eq!(tls_finding.remediation.as_deref(), Some("Disable TLS 1.0 and TLS 1.1. Disable weak ciphers."));

    // Check Password finding
    let pwd_finding = report.findings.iter().find(|f| f.requirement == "8.3.6").unwrap();
    assert_eq!(pwd_finding.status, ComplianceStatus::Fail);
    assert!(pwd_finding.remediation.as_ref().unwrap().contains("Increase minimum password length"));

    // Check Firewall finding
    let fw_finding = report.findings.iter().find(|f| f.requirement == "1.2").unwrap();
    assert_eq!(fw_finding.status, ComplianceStatus::Fail);
    assert!(fw_finding.remediation.as_ref().unwrap().contains("Set default firewall inbound action to deny or drop."));

    // Check Audit finding
    let audit_finding = report.findings.iter().find(|f| f.requirement == "10.2").unwrap();
    assert_eq!(audit_finding.status, ComplianceStatus::Fail);
    assert!(audit_finding.remediation.as_ref().unwrap().contains("Enable audit logging."));
}
