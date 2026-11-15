#![forbid(unsafe_code)]

use mail::rspamd_rules::{RspamdRuleFormatter, RspamdThresholds};

#[test]
fn test_default_thresholds() {
    let formatter = RspamdRuleFormatter::new();
    let actions = formatter.generate_actions_conf();

    assert!(actions.contains("reject = 15;"));
    assert!(actions.contains("add_header = 6;"));
    assert!(actions.contains("rewrite_subject = 6;"));
    assert!(actions.contains("greylist = 4;"));
}

#[test]
fn test_custom_thresholds() {
    let thresholds = RspamdThresholds {
        reject: Some(12.0),
        add_header: Some(5.0),
        rewrite_subject: None,
        greylist: Some(2.5),
    };

    let formatter = RspamdRuleFormatter::new().with_thresholds(thresholds);
    let actions = formatter.generate_actions_conf();

    assert!(actions.contains("reject = 12;"));
    assert!(actions.contains("add_header = 5;"));
    assert!(!actions.contains("rewrite_subject"));
    assert!(actions.contains("greylist = 2.5;"));
}

#[test]
fn test_no_symbol_weights() {
    let formatter = RspamdRuleFormatter::new();
    let metrics = formatter.generate_metrics_conf();

    assert_eq!(metrics, "");
}

#[test]
fn test_symbol_weights() {
    let formatter = RspamdRuleFormatter::new()
        .with_symbol_weight("BAYES_SPAM", 5.5)
        .with_symbol_weight("DKIM_REJECT", 8.0);

    let metrics = formatter.generate_metrics_conf();

    assert!(metrics.contains("group \"default\" {"));
    assert!(metrics.contains("symbol \"BAYES_SPAM\" {"));
    assert!(metrics.contains("weight = 5.50;"));
    assert!(metrics.contains("symbol \"DKIM_REJECT\" {"));
    assert!(metrics.contains("weight = 8.00;"));
}
