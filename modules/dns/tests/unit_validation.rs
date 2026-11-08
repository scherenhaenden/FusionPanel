#![forbid(unsafe_code)]

use dns::validation::{
    validate_a, validate_aaaa, validate_caa, validate_cname, validate_mx, validate_srv,
    validate_txt,
};

#[test]
fn test_validate_a() {
    assert_eq!(validate_a("192.0.2.1"), Ok(()));
    assert_eq!(validate_a("8.8.8.8"), Ok(()));
    assert_eq!(validate_a("255.255.255.255"), Ok(()));

    assert!(validate_a("256.0.0.1").is_err());
    assert!(validate_a("192.0.2").is_err());
    assert!(validate_a("not-an-ip").is_err());
    assert!(validate_a("2001:db8::1").is_err()); // IPv6 in A record
}

#[test]
fn test_validate_aaaa() {
    assert_eq!(validate_aaaa("2001:db8::1"), Ok(()));
    assert_eq!(validate_aaaa("::1"), Ok(()));
    assert_eq!(
        validate_aaaa("2001:0db8:85a3:0000:0000:8a2e:0370:7334"),
        Ok(())
    );

    assert!(validate_aaaa("192.0.2.1").is_err()); // IPv4 in AAAA record
    assert!(validate_aaaa("not-an-ip").is_err());
    assert!(validate_aaaa("2001:db8:::1").is_err());
}

#[test]
fn test_validate_cname() {
    assert_eq!(validate_cname("example.com"), Ok(()));
    assert_eq!(validate_cname("sub.example.com."), Ok(())); // With root dot
    assert_eq!(validate_cname("a.b-c.d"), Ok(()));

    assert!(validate_cname("").is_err());
    assert!(validate_cname("example..com").is_err());
    assert!(validate_cname("192.168.1.1").is_err()); // IP literal
}

#[test]
fn test_validate_mx() {
    assert_eq!(validate_mx("10 mail.example.com"), Ok(()));
    assert_eq!(validate_mx("0 ."), Ok(())); // Null MX

    assert!(validate_mx("mail.example.com").is_err()); // Missing priority
    assert!(validate_mx("10 20 mail.example.com").is_err()); // Too many parts
    assert!(validate_mx("abc mail.example.com").is_err()); // Invalid priority
    assert!(validate_mx("10 .").is_err()); // Null MX must have priority 0
    assert!(validate_mx("10 invalid..domain").is_err()); // Invalid target
}

#[test]
fn test_validate_txt() {
    assert_eq!(validate_txt("v=spf1 -all"), Ok(()));
    assert_eq!(validate_txt("\"v=spf1 -all\""), Ok(()));
    assert_eq!(validate_txt("\"chunk 1\" \"chunk 2\""), Ok(()));
    assert_eq!(validate_txt("\"has \\\"quotes\\\"\""), Ok(())); // Escaped quotes

    assert!(validate_txt("").is_err()); // Empty
    assert!(validate_txt("\"unmatched").is_err()); // Unmatched quote

    let long_str = "a".repeat(256);
    assert!(validate_txt(&long_str).is_err()); // Unquoted > 255

    let long_quoted_str = format!("\"{}\"", long_str);
    assert!(validate_txt(&long_quoted_str).is_err()); // Quoted > 255

    let multi_chunk = format!("\"{}\" \"{}\"", "a".repeat(200), "b".repeat(200));
    assert_eq!(validate_txt(&multi_chunk), Ok(())); // Multiple valid chunks
}

#[test]
fn test_validate_caa() {
    assert_eq!(validate_caa("0 issue \"letsencrypt.org\""), Ok(()));
    assert_eq!(validate_caa("128 issuewild \"letsencrypt.org\""), Ok(()));
    assert_eq!(
        validate_caa("0 iodef \"mailto:security@example.com\""),
        Ok(())
    );

    assert!(validate_caa("0 issue letsencrypt.org").is_err()); // Missing quotes
    assert!(validate_caa("issue \"letsencrypt.org\"").is_err()); // Missing flags
    assert!(validate_caa("256 issue \"letsencrypt.org\"").is_err()); // Invalid flags
    assert!(validate_caa("0 invalid_tag \"letsencrypt.org\"").is_err()); // Invalid tag format
                                                                         // Not testing specific tag keywords like "issue", since any alphanumeric is technically valid by RFC 6844,
                                                                         // although standard tags are issue, issuewild, iodef. The code currently checks `is_ascii_alphanumeric`.
}

#[test]
fn test_validate_srv() {
    assert_eq!(validate_srv("10 5 5060 sipserver.example.com"), Ok(()));
    assert_eq!(validate_srv("0 100 80 www.example.com."), Ok(()));

    assert!(validate_srv("10 5 sipserver.example.com").is_err()); // Missing port
    assert!(validate_srv("abc 5 5060 sipserver.example.com").is_err()); // Invalid priority
    assert!(validate_srv("10 abc 5060 sipserver.example.com").is_err()); // Invalid weight
    assert!(validate_srv("10 5 abc sipserver.example.com").is_err()); // Invalid port
    assert!(validate_srv("10 5 5060 invalid..domain").is_err()); // Invalid target
}
