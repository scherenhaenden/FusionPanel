#![forbid(unsafe_code)]

use std::collections::HashMap;
use mail::milter_rate_limit::{PostscreenAction, PostscreenConfigBuilder, PostscreenError};

#[test]
fn test_postscreen_config_builder() {
    let config = PostscreenConfigBuilder::new()
        .add_dnsbl_site("zen.spamhaus.org", 3)
        .add_dnsbl_site("bl.spamcop.net", 2)
        .add_dnsbl_site("b.barracudacentral.org", 1)
        .dnsbl_threshold(3)
        .greet_action(PostscreenAction::Enforce)
        .build()
        .expect("Failed to build config");

    assert_eq!(config.dnsbl_threshold, 3);
    assert_eq!(config.dnsbl_sites.len(), 3);

    let main_cf = config.generate_main_cf();
    assert!(main_cf.contains("postscreen_greet_action = enforce"));
    assert!(main_cf.contains("postscreen_dnsbl_sites = zen.spamhaus.org*3, bl.spamcop.net*2, b.barracudacentral.org*1"));
    assert!(main_cf.contains("postscreen_dnsbl_threshold = 3"));
    assert!(main_cf.contains("postscreen_dnsbl_action = enforce"));

    let master_cf = config.generate_master_cf();
    assert!(master_cf.contains("smtp      inet  n       -       y       -       1       postscreen"));
    assert!(master_cf.contains("smtpd     pass  -       -       y       -       -       smtpd"));
}

#[test]
fn test_invalid_dnsbl_threshold() {
    let result = PostscreenConfigBuilder::new()
        .dnsbl_threshold(0)
        .build();

    assert!(matches!(result, Err(PostscreenError::InvalidThreshold)));
}

#[test]
fn test_invalid_dnsbl_name() {
    let result = PostscreenConfigBuilder::new()
        .add_dnsbl_site("  ", 2)
        .build();

    assert!(matches!(result, Err(PostscreenError::InvalidDnsblName)));
}

#[test]
fn test_score_calculation() {
    let config = PostscreenConfigBuilder::new()
        .add_dnsbl_site("zen.spamhaus.org", 3)
        .add_dnsbl_site("bl.spamcop.net", 2)
        .add_dnsbl_site("list.dnswl.org", -1) // whitelist
        .dnsbl_threshold(3)
        .build()
        .unwrap();

    let mut query_responses = HashMap::new();

    // Listed on spamcop (weight 2)
    query_responses.insert("bl.spamcop.net".to_string(), vec!["127.0.0.2".to_string()]);
    assert_eq!(config.calculate_score(&query_responses), 2);
    assert!(!config.is_rejected(&query_responses));

    // Whitelisted (-1), so score drops to 1
    query_responses.insert("list.dnswl.org".to_string(), vec!["127.0.5.1".to_string()]);
    assert_eq!(config.calculate_score(&query_responses), 1);
    assert!(!config.is_rejected(&query_responses));

    // Listed on spamhaus (weight 3), so score jumps to 4
    query_responses.insert("zen.spamhaus.org".to_string(), vec!["127.0.0.3".to_string()]);
    assert_eq!(config.calculate_score(&query_responses), 4);
    assert!(config.is_rejected(&query_responses));
}
