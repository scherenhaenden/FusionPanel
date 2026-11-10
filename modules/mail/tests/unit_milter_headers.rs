#![forbid(unsafe_code)]

use mail::milter_headers::{AuthenticationResults, AuthResult, AuthProperty};
use std::str::FromStr;

#[test]
fn test_format_authentication_results_none() {
    let auth = AuthenticationResults {
        authserv_id: "example.com".to_string(),
        results: vec![],
    };

    assert_eq!(
        auth.to_string(),
        "Authentication-Results: example.com; none"
    );
}

#[test]
fn test_format_authentication_results_dkim_spf() {
    let auth = AuthenticationResults {
        authserv_id: "example.com".to_string(),
        results: vec![
            AuthResult {
                method: "dkim".to_string(),
                result: "pass".to_string(),
                reason: None,
                properties: vec![
                    AuthProperty {
                        ptype: "header".to_string(),
                        property: "d".to_string(),
                        value: "example.net".to_string(),
                    },
                ],
            },
            AuthResult {
                method: "spf".to_string(),
                result: "fail".to_string(),
                reason: Some("No valid SPF record found".to_string()),
                properties: vec![
                    AuthProperty {
                        ptype: "smtp".to_string(),
                        property: "mailfrom".to_string(),
                        value: "sender@example.net".to_string(),
                    },
                ],
            },
        ],
    };

    let expected = "Authentication-Results: example.com; dkim=pass header.d=example.net; spf=fail reason=\"No valid SPF record found\" smtp.mailfrom=sender@example.net";
    assert_eq!(auth.to_string(), expected);
}

#[test]
fn test_parse_authentication_results_none() {
    let s = "Authentication-Results: example.com; none";
    let auth = AuthenticationResults::from_str(s).unwrap();
    assert_eq!(auth.authserv_id, "example.com");
    assert!(auth.results.is_empty());
}

#[test]
fn test_parse_authentication_results_dkim_spf() {
    let s = "Authentication-Results: example.com; dkim=pass header.d=example.net; spf=fail reason=\"No valid SPF record found\" smtp.mailfrom=sender@example.net";
    let auth = AuthenticationResults::from_str(s).unwrap();

    assert_eq!(auth.authserv_id, "example.com");
    assert_eq!(auth.results.len(), 2);

    let dkim = &auth.results[0];
    assert_eq!(dkim.method, "dkim");
    assert_eq!(dkim.result, "pass");
    assert_eq!(dkim.reason, None);
    assert_eq!(dkim.properties.len(), 1);
    assert_eq!(dkim.properties[0].ptype, "header");
    assert_eq!(dkim.properties[0].property, "d");
    assert_eq!(dkim.properties[0].value, "example.net");

    let spf = &auth.results[1];
    assert_eq!(spf.method, "spf");
    assert_eq!(spf.result, "fail");
    assert_eq!(spf.reason, Some("No valid SPF record found".to_string()));
    assert_eq!(spf.properties.len(), 1);
    assert_eq!(spf.properties[0].ptype, "smtp");
    assert_eq!(spf.properties[0].property, "mailfrom");
    assert_eq!(spf.properties[0].value, "sender@example.net");
}

#[test]
fn test_parse_authentication_results_dmarc() {
    let s = "Authentication-Results: example.com; dmarc=pass (p=reject dis=none) header.from=example.com";
    let auth = AuthenticationResults::from_str(s).unwrap();

    assert_eq!(auth.authserv_id, "example.com");
    assert_eq!(auth.results.len(), 1);

    let dmarc = &auth.results[0];
    assert_eq!(dmarc.method, "dmarc");
    assert_eq!(dmarc.result, "pass");
    assert_eq!(dmarc.properties.len(), 1);
    assert_eq!(dmarc.properties[0].ptype, "header");
    assert_eq!(dmarc.properties[0].property, "from");
    assert_eq!(dmarc.properties[0].value, "example.com");
}

#[test]
fn test_parse_invalid() {
    assert!(AuthenticationResults::from_str("").is_err());
    assert!(AuthenticationResults::from_str("Authentication-Results:").is_err());
}
