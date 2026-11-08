#![forbid(unsafe_code)]

#[path = "../src/dns_audit.rs"]
pub mod dns_audit;

use dns_audit::{DomainSecurityAuditor, Grade};

#[test]
fn test_audit_aplus() {
    let auditor = DomainSecurityAuditor::new();
    let report = auditor.audit(
        Some("v=spf1 mx a -all"),
        &["v=DKIM1; k=rsa; p=MIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEA"],
        Some("v=DMARC1; p=reject; rua=mailto:dmarc@example.com; ruf=mailto:dmarc@example.com;"),
    );

    assert_eq!(report.grade, Grade::APlus);
    assert!(report.weaknesses.is_empty());
    assert!(report.recommendations.is_empty());
}

#[test]
fn test_audit_a() {
    let auditor = DomainSecurityAuditor::new();
    let report = auditor.audit(
        Some("v=spf1 mx a -all"),
        &["v=DKIM1; k=rsa; p=MIIBIj...; t=y;"],
        Some("v=DMARC1; p=reject; rua=mailto:dmarc@example.com; ruf=mailto:dmarc@example.com;"),
    );

    assert_eq!(report.grade, Grade::A);
    assert_eq!(report.weaknesses, vec!["DKIM in testing mode (t=y)"]);
}

#[test]
fn test_audit_b() {
    let auditor = DomainSecurityAuditor::new();
    let report = auditor.audit(
        Some("v=spf1 mx a ~all"),
        &["v=DKIM1; k=rsa; p=MIIBIj...;"],
        Some("v=DMARC1; p=quarantine; rua=mailto:dmarc@example.com; ruf=mailto:dmarc@example.com;"),
    );

    assert_eq!(report.grade, Grade::B);
    assert!(report
        .weaknesses
        .contains(&"SPF uses softfail (~all)".to_string()));
    assert!(report
        .weaknesses
        .contains(&"DMARC policy is quarantine, not reject".to_string()));
}

#[test]
fn test_audit_c() {
    let auditor = DomainSecurityAuditor::new();
    let report = auditor.audit(Some("v=spf1 mx a ~all"), &[], Some("v=DMARC1; p=none;"));

    assert_eq!(report.grade, Grade::C);
    assert!(report
        .weaknesses
        .contains(&"Missing DKIM record".to_string()));
    assert!(report
        .weaknesses
        .contains(&"DMARC policy is none".to_string()));
    assert!(report.weaknesses.contains(&"DMARC missing rua".to_string()));
    assert!(report.weaknesses.contains(&"DMARC missing ruf".to_string()));
}

#[test]
fn test_audit_f() {
    let auditor = DomainSecurityAuditor::new();
    let report = auditor.audit(None, &[], None);

    assert_eq!(report.grade, Grade::F);
    assert!(report
        .weaknesses
        .contains(&"Missing SPF record".to_string()));
    assert!(report
        .weaknesses
        .contains(&"Missing DKIM record".to_string()));
    assert!(report
        .weaknesses
        .contains(&"Missing DMARC record".to_string()));
}

#[test]
fn test_audit_f_spf_only_permissive() {
    let auditor = DomainSecurityAuditor::new();
    let report = auditor.audit(Some("v=spf1 mx a +all"), &[], None);

    assert_eq!(report.grade, Grade::F);
    assert!(report
        .weaknesses
        .contains(&"SPF is too permissive".to_string()));
}

#[test]
fn test_audit_f_spf_no_all() {
    let auditor = DomainSecurityAuditor::new();
    let report = auditor.audit(Some("v=spf1 mx a"), &[], None);

    assert_eq!(report.grade, Grade::F);
    assert!(report
        .weaknesses
        .contains(&"SPF does not specify an 'all' mechanism".to_string()));
}
