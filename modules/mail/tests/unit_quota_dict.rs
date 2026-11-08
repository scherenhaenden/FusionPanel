#![forbid(unsafe_code)]

#[path = "../src/quota_dict.rs"]
mod quota_dict;

use quota_dict::{DictCommand, DictProtocolError, DictResponse, DovecotQuotaDictProtocol};

#[test]
fn test_parse_lookup_success() {
    let input = "Lshared/quota/storage\tjules@example.com\n";
    let cmd = DovecotQuotaDictProtocol::parse(input).unwrap();
    assert_eq!(
        cmd,
        DictCommand::Lookup {
            path: "shared/quota/storage".to_string(),
            username: "jules@example.com".to_string(),
        }
    );
}

#[test]
fn test_parse_set_success() {
    let input = "Sshared/quota/storage\tjules@example.com\t1048576\n";
    let cmd = DovecotQuotaDictProtocol::parse(input).unwrap();
    assert_eq!(
        cmd,
        DictCommand::Set {
            path: "shared/quota/storage".to_string(),
            username: "jules@example.com".to_string(),
            value: "1048576".to_string(),
        }
    );
}

#[test]
fn test_parse_inc_success() {
    let input = "Ishared/quota/storage\tjules@example.com\t-500\n";
    let cmd = DovecotQuotaDictProtocol::parse(input).unwrap();
    assert_eq!(
        cmd,
        DictCommand::Inc {
            path: "shared/quota/storage".to_string(),
            username: "jules@example.com".to_string(),
            diff: -500,
        }
    );
}

#[test]
fn test_format_response_ok_some() {
    let resp = DictResponse::Ok(Some("1048576".to_string()));
    assert_eq!(DovecotQuotaDictProtocol::format(&resp), "O1048576\n");
}

#[test]
fn test_format_response_ok_none() {
    let resp = DictResponse::Ok(None);
    assert_eq!(DovecotQuotaDictProtocol::format(&resp), "N\n");
}

#[test]
fn test_format_response_fail() {
    let resp = DictResponse::Fail;
    assert_eq!(DovecotQuotaDictProtocol::format(&resp), "F\n");
}

#[test]
fn test_parse_missing_newline() {
    let input = "Lshared/quota/storage\tjules@example.com";
    let err = DovecotQuotaDictProtocol::parse(input).unwrap_err();
    assert_eq!(err, DictProtocolError::InvalidFormat);
}

#[test]
fn test_parse_empty_string() {
    let input = "\n";
    let err = DovecotQuotaDictProtocol::parse(input).unwrap_err();
    assert_eq!(err, DictProtocolError::InvalidFormat);
}

#[test]
fn test_parse_unknown_command() {
    let input = "Xshared/quota/storage\tjules@example.com\n";
    let err = DovecotQuotaDictProtocol::parse(input).unwrap_err();
    assert_eq!(err, DictProtocolError::UnknownCommand);
}

#[test]
fn test_parse_invalid_diff() {
    let input = "Ishared/quota/storage\tjules@example.com\tnotanumber\n";
    let err = DovecotQuotaDictProtocol::parse(input).unwrap_err();
    assert_eq!(err, DictProtocolError::InvalidDiff);
}

#[test]
fn test_parse_injection_newline() {
    let input = "Lshared/quota/storage\tju\nles@example.com\n";
    let err = DovecotQuotaDictProtocol::parse(input).unwrap_err();
    assert_eq!(err, DictProtocolError::InjectionAttempt);
}

#[test]
fn test_parse_injection_carriage_return() {
    let input = "Lshared/quota/storage\r\tjules@example.com\n";
    let err = DovecotQuotaDictProtocol::parse(input).unwrap_err();
    assert_eq!(err, DictProtocolError::InjectionAttempt);
}

#[test]
fn test_parse_injection_null_byte() {
    let input = "Lshared/quota/storage\0\tjules@example.com\n";
    let err = DovecotQuotaDictProtocol::parse(input).unwrap_err();
    assert_eq!(err, DictProtocolError::InjectionAttempt);
}

#[test]
fn test_parse_injection_extra_tab() {
    let input = "Lshared/quota/storage\tju\tles@example.com\n";
    let err = DovecotQuotaDictProtocol::parse(input).unwrap_err();
    assert_eq!(err, DictProtocolError::InjectionAttempt);
}

#[test]
fn test_parse_set_missing_value() {
    let input = "Sshared/quota/storage\tjules@example.com\n";
    let err = DovecotQuotaDictProtocol::parse(input).unwrap_err();
    assert_eq!(err, DictProtocolError::InjectionAttempt); // In our code this ends up splitting to < 3 parts
}

#[test]
fn test_parse_inc_missing_diff() {
    let input = "Ishared/quota/storage\tjules@example.com\n";
    let err = DovecotQuotaDictProtocol::parse(input).unwrap_err();
    assert_eq!(err, DictProtocolError::InjectionAttempt);
}

#[test]
fn test_parse_lookup_too_many_tabs() {
    let input = "Lshared/quota/storage\tjules@example.com\textra\n";
    let err = DovecotQuotaDictProtocol::parse(input).unwrap_err();
    assert_eq!(err, DictProtocolError::InjectionAttempt);
}
