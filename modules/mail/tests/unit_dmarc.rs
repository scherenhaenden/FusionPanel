use mail::dmarc::*;
use mail::dmarc::{evaluate_dmarc, check_alignment, is_subdomain_or_exact};
use serde_json;

#[test]
fn test_is_subdomain_or_exact() {
    assert!(is_subdomain_or_exact("example.com", "example.com"));
    assert!(is_subdomain_or_exact("sub.example.com", "example.com"));
    assert!(is_subdomain_or_exact("a.b.example.com", "example.com"));
    assert!(!is_subdomain_or_exact("notexample.com", "example.com"));
    assert!(!is_subdomain_or_exact("example.com.org", "example.com"));
    assert!(!is_subdomain_or_exact("example", "example.com"));
}

#[test]
fn test_check_alignment_strict() {
    assert!(check_alignment("example.com", "example.com", &Alignment::Strict));
    assert!(!check_alignment("sub.example.com", "example.com", &Alignment::Strict));
    assert!(!check_alignment("example.com", "sub.example.com", &Alignment::Strict));
    assert!(!check_alignment("other.com", "example.com", &Alignment::Strict));
}

#[test]
fn test_check_alignment_relaxed() {
    assert!(check_alignment("example.com", "example.com", &Alignment::Relaxed));
    assert!(check_alignment("sub.example.com", "example.com", &Alignment::Relaxed));
    assert!(!check_alignment("example.com", "sub.example.com", &Alignment::Relaxed));
    assert!(!check_alignment("notexample.com", "example.com", &Alignment::Relaxed));
}

#[test]
fn test_evaluate_dmarc_pass_spf() {
    let spf = SpfStatus { result: AuthResult::Pass, domain: "example.com".to_string() };
    let dkim = DkimStatus { result: AuthResult::Fail, domain: "example.com".to_string() };
    let config = DmarcConfig {
        policy: DmarcPolicy::Reject,
        spf_alignment: Alignment::Relaxed,
        dkim_alignment: Alignment::Relaxed,
    };

    let evaluation = evaluate_dmarc("example.com", &spf, &dkim, &config);
    assert_eq!(evaluation.result, DmarcResult::Pass);
    assert_eq!(evaluation.applied_policy, DmarcPolicy::None);
    assert!(evaluation.spf_aligned);
    assert!(!evaluation.dkim_aligned);
}

#[test]
fn test_evaluate_dmarc_pass_dkim() {
    let spf = SpfStatus { result: AuthResult::SoftFail, domain: "example.com".to_string() };
    let dkim = DkimStatus { result: AuthResult::Pass, domain: "example.com".to_string() };
    let config = DmarcConfig {
        policy: DmarcPolicy::Quarantine,
        spf_alignment: Alignment::Relaxed,
        dkim_alignment: Alignment::Relaxed,
    };

    let evaluation = evaluate_dmarc("example.com", &spf, &dkim, &config);
    assert_eq!(evaluation.result, DmarcResult::Pass);
    assert_eq!(evaluation.applied_policy, DmarcPolicy::None);
    assert!(!evaluation.spf_aligned);
    assert!(evaluation.dkim_aligned);
}

#[test]
fn test_evaluate_dmarc_fail_both() {
    let spf = SpfStatus { result: AuthResult::Fail, domain: "example.com".to_string() };
    let dkim = DkimStatus { result: AuthResult::Fail, domain: "example.com".to_string() };
    let config = DmarcConfig {
        policy: DmarcPolicy::Reject,
        spf_alignment: Alignment::Relaxed,
        dkim_alignment: Alignment::Relaxed,
    };

    let evaluation = evaluate_dmarc("example.com", &spf, &dkim, &config);
    assert_eq!(evaluation.result, DmarcResult::Fail);
    assert_eq!(evaluation.applied_policy, DmarcPolicy::Reject);
    assert!(!evaluation.spf_aligned);
    assert!(!evaluation.dkim_aligned);
}

#[test]
fn test_evaluate_dmarc_pass_but_unaligned() {
    let spf = SpfStatus { result: AuthResult::Pass, domain: "other.com".to_string() };
    let dkim = DkimStatus { result: AuthResult::Pass, domain: "other.com".to_string() };
    let config = DmarcConfig {
        policy: DmarcPolicy::Quarantine,
        spf_alignment: Alignment::Relaxed,
        dkim_alignment: Alignment::Relaxed,
    };

    let evaluation = evaluate_dmarc("example.com", &spf, &dkim, &config);
    assert_eq!(evaluation.result, DmarcResult::Fail);
    assert_eq!(evaluation.applied_policy, DmarcPolicy::Quarantine);
    assert!(!evaluation.spf_aligned);
    assert!(!evaluation.dkim_aligned);
}

#[test]
fn test_evaluate_dmarc_strict_alignment() {
    let spf = SpfStatus { result: AuthResult::Pass, domain: "sub.example.com".to_string() };
    let dkim = DkimStatus { result: AuthResult::Pass, domain: "sub.example.com".to_string() };
    let config_strict = DmarcConfig {
        policy: DmarcPolicy::Reject,
        spf_alignment: Alignment::Strict,
        dkim_alignment: Alignment::Strict,
    };

    // Strict fails
    let eval_strict = evaluate_dmarc("example.com", &spf, &dkim, &config_strict);
    assert_eq!(eval_strict.result, DmarcResult::Fail);
    assert_eq!(eval_strict.applied_policy, DmarcPolicy::Reject);

    // Exact strict match passes
    let eval_strict_exact = evaluate_dmarc("sub.example.com", &spf, &dkim, &config_strict);
    assert_eq!(eval_strict_exact.result, DmarcResult::Pass);
    assert_eq!(eval_strict_exact.applied_policy, DmarcPolicy::None);
}

#[test]
fn test_aggregate_report_json() {
    let eval = DmarcEvaluation {
        result: DmarcResult::Pass,
        applied_policy: DmarcPolicy::None,
        spf_aligned: true,
        dkim_aligned: true,
    };

    let row = AggregateReportRow {
        source_ip: "192.168.1.1".to_string(),
        count: 10,
        evaluation: eval,
        header_from: "example.com".to_string(),
        spf_domain: "example.com".to_string(),
        spf_result: AuthResult::Pass,
        dkim_domain: "example.com".to_string(),
        dkim_result: AuthResult::Pass,
    };

    let report = AggregateReport {
        org_name: "FusionPanel".to_string(),
        email: "noreply@fusionpanel.example".to_string(),
        report_id: "report-12345".to_string(),
        domain: "example.com".to_string(),
        begin_date: 1600000000,
        end_date: 1600086400,
        rows: vec![row],
    };

    let json = report.to_json().unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed["org_name"], "FusionPanel");
    assert_eq!(parsed["rows"][0]["source_ip"], "192.168.1.1");
    assert_eq!(parsed["rows"][0]["evaluation"]["result"], "pass");
}

#[test]
fn test_is_subdomain_or_exact_case_insensitive() {
    assert!(is_subdomain_or_exact("sub.EXAMPLE.com", "example.com"));
    assert!(is_subdomain_or_exact("sub.example.com", "EXAMPLE.COM"));
}
