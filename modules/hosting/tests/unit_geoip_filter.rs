#![forbid(unsafe_code)]

use hosting::geoip_filter::{Action, GeoIpFilterConfig, GeoIpRuleBuilder, RuleType};

#[test]
fn test_allowlist_generation() {
    let config = GeoIpFilterConfig {
        rule_type: RuleType::Allowlist(vec!["US".to_string(), "CA".to_string()]),
        bypass_private_ips: false,
        custom_action: Action::Redirect("https://example.com/blocked".to_string()),
    };

    let builder = GeoIpRuleBuilder::new(config);
    let map = builder.build_nginx_map();

    assert!(map.contains("map $geoip2_data_country_code $allowed_country {"));
    assert!(map.contains("default redirect:https://example.com/blocked;"));
    assert!(map.contains("US yes;"));
    assert!(map.contains("CA yes;"));

    let bypass_map = builder.build_bypass_map();
    assert!(bypass_map.is_none());
}

#[test]
fn test_blocklist_generation_with_custom_action() {
    let config = GeoIpFilterConfig {
        rule_type: RuleType::Blocklist(vec!["RU".to_string(), "CN".to_string()]),
        bypass_private_ips: true,
        custom_action: Action::Redirect("https://example.com/blocked".to_string()),
    };

    let builder = GeoIpRuleBuilder::new(config);
    let map = builder.build_nginx_map();

    assert!(map.contains("map $geoip2_data_country_code $allowed_country {"));
    assert!(map.contains("default yes;"));
    assert!(map.contains("RU redirect:https://example.com/blocked;"));
    assert!(map.contains("CN redirect:https://example.com/blocked;"));

    let bypass_map = builder.build_bypass_map().unwrap();
    assert!(bypass_map.contains("geo $bypass_geoip {"));
    assert!(bypass_map.contains("10.0.0.0/8 1;"));
    assert!(bypass_map.contains("192.168.0.0/16 1;"));
}

#[test]
fn test_action_display() {
    assert_eq!(Action::Allow.to_string(), "yes");
    assert_eq!(Action::Block403.to_string(), "no");
    assert_eq!(Action::Redirect("http://url".to_string()).to_string(), "redirect:http://url");
}
