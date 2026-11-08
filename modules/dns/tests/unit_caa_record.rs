#![forbid(unsafe_code)]

use dns::caa_record::{CaaError, CaaRecordFormatter};

#[test]
fn test_caa_record_formatting_issue() {
    let result = CaaRecordFormatter::format("example.com", 0, "issue", "letsencrypt.org").unwrap();
    assert_eq!(result, "example.com. IN CAA 0 issue \"letsencrypt.org\"");
}

#[test]
fn test_caa_record_formatting_issuewild() {
    let result = CaaRecordFormatter::format("example.com", 0, "issuewild", "letsencrypt.org").unwrap();
    assert_eq!(result, "example.com. IN CAA 0 issuewild \"letsencrypt.org\"");
}

#[test]
fn test_caa_record_formatting_iodef_mailto() {
    let result = CaaRecordFormatter::format("example.com", 128, "iodef", "mailto:security@example.com").unwrap();
    assert_eq!(result, "example.com. IN CAA 128 iodef \"mailto:security@example.com\"");
}

#[test]
fn test_caa_record_formatting_iodef_https() {
    let result = CaaRecordFormatter::format("example.com", 128, "iodef", "https://example.com/security").unwrap();
    assert_eq!(result, "example.com. IN CAA 128 iodef \"https://example.com/security\"");
}

#[test]
fn test_invalid_flag() {
    let result = CaaRecordFormatter::format("example.com", 1, "issue", "letsencrypt.org");
    assert_eq!(result, Err(CaaError::InvalidFlag(1)));

    let result = CaaRecordFormatter::format("example.com", 255, "issue", "letsencrypt.org");
    assert_eq!(result, Err(CaaError::InvalidFlag(255)));
}

#[test]
fn test_unsupported_tag() {
    let result = CaaRecordFormatter::format("example.com", 0, "unsupported", "letsencrypt.org");
    assert_eq!(result, Err(CaaError::UnsupportedTag("unsupported".to_string())));
}

#[test]
fn test_invalid_ca_domain() {
    let result = CaaRecordFormatter::format("example.com", 0, "issue", "invalid domain");
    assert_eq!(result, Err(CaaError::InvalidCaDomain("invalid domain".to_string())));
    
    let result = CaaRecordFormatter::format("example.com", 0, "issuewild", "invalid domain");
    assert_eq!(result, Err(CaaError::InvalidCaDomain("invalid domain".to_string())));
}

#[test]
fn test_invalid_uri_iodef() {
    let result = CaaRecordFormatter::format("example.com", 128, "iodef", "ftp://example.com");
    assert_eq!(result, Err(CaaError::InvalidUri("ftp://example.com".to_string())));

    let result = CaaRecordFormatter::format("example.com", 128, "iodef", "security@example.com");
    assert_eq!(result, Err(CaaError::InvalidUri("security@example.com".to_string())));
}

#[test]
fn test_fqdn_formatting() {
    let result = CaaRecordFormatter::format("example.com.", 0, "issue", "letsencrypt.org").unwrap();
    assert_eq!(result, "example.com. IN CAA 0 issue \"letsencrypt.org\"");
}
