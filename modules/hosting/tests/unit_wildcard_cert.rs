#![forbid(unsafe_code)]

use hosting::wildcard_cert::{validate_and_group_sans, San, WildcardCertError};

#[test]
fn test_san_parsing_valid() {
    assert_eq!(San::parse("example.com").unwrap(), San::Exact("example.com".to_string()));
    assert_eq!(San::parse("*.example.com").unwrap(), San::Wildcard("example.com".to_string()));
    assert_eq!(San::parse("sub.example.com").unwrap(), San::Exact("sub.example.com".to_string()));
    assert_eq!(San::parse("*.sub.example.com").unwrap(), San::Wildcard("sub.example.com".to_string()));
}

#[test]
fn test_san_parsing_invalid() {
    assert!(matches!(San::parse(""), Err(WildcardCertError::InvalidSyntax(_))));
    assert!(matches!(San::parse("*."), Err(WildcardCertError::InvalidSyntax(_))));
    assert!(matches!(San::parse(".example.com"), Err(WildcardCertError::InvalidSyntax(_))));
    assert!(matches!(San::parse("example..com"), Err(WildcardCertError::InvalidSyntax(_))));
    assert!(matches!(San::parse("-example.com"), Err(WildcardCertError::InvalidSyntax(_))));
    assert!(matches!(San::parse("example-.com"), Err(WildcardCertError::InvalidSyntax(_))));
    assert!(matches!(San::parse("ex ample.com"), Err(WildcardCertError::InvalidSyntax(_))));
    assert!(matches!(San::parse("ex@mple.com"), Err(WildcardCertError::InvalidSyntax(_))));
    assert!(matches!(San::parse(&"a".repeat(254)), Err(WildcardCertError::InvalidSyntax(_))));
    assert!(matches!(San::parse(&format!("{}.com", "a".repeat(64))), Err(WildcardCertError::InvalidSyntax(_))));
}

#[test]
fn test_validate_and_group_sans_success() {
    let sans = vec!["example.com", "*.example.com", "other.com", "*.other.com"];
    let grouped = validate_and_group_sans(&sans).unwrap();

    assert_eq!(grouped.len(), 2);

    let example_group = grouped.get("_acme-challenge.example.com").unwrap();
    assert_eq!(example_group.len(), 2);
    assert!(example_group.contains(&San::Exact("example.com".to_string())));
    assert!(example_group.contains(&San::Wildcard("example.com".to_string())));

    let other_group = grouped.get("_acme-challenge.other.com").unwrap();
    assert_eq!(other_group.len(), 2);
    assert!(other_group.contains(&San::Exact("other.com".to_string())));
    assert!(other_group.contains(&San::Wildcard("other.com".to_string())));
}

#[test]
fn test_duplicate_san() {
    let sans = vec!["example.com", "example.com"];
    assert!(matches!(
        validate_and_group_sans(&sans),
        Err(WildcardCertError::DuplicateSan(_))
    ));
}

#[test]
fn test_redundant_san_wildcard() {
    let sans = vec!["*.example.com", "sub.example.com"];
    assert!(matches!(
        validate_and_group_sans(&sans),
        Err(WildcardCertError::RedundantSan(_))
    ));
}

#[test]
fn test_non_redundant_apex_with_wildcard() {
    let sans = vec!["*.example.com", "example.com"];
    assert!(validate_and_group_sans(&sans).is_ok());
}
