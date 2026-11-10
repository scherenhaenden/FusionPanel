#![forbid(unsafe_code)]

use mail::maildir_flags::{DovecotKeywords, MaildirFlags, MaildirError};
use std::collections::HashSet;
use tempfile::NamedTempFile;

#[test]
fn test_parse_maildir_flags() {
    let filename = "12345.M6789P123.host:2,S";
    let flags = MaildirFlags::parse_from_filename(filename).unwrap();
    assert!(flags.seen);
    assert!(!flags.replied);
    assert!(!flags.trashed);
    assert!(flags.custom_keywords.is_empty());
}

#[test]
fn test_parse_maildir_flags_multiple() {
    let filename = "12345.M6789P123.host:2,PRS";
    let flags = MaildirFlags::parse_from_filename(filename).unwrap();
    assert!(flags.passed);
    assert!(flags.replied);
    assert!(flags.seen);
    assert!(!flags.trashed);
    assert!(!flags.draft);
    assert!(!flags.flagged);
    assert!(flags.custom_keywords.is_empty());
}

#[test]
fn test_parse_maildir_flags_with_custom() {
    let filename = "12345.M6789P123.host:2,Sabc";
    let flags = MaildirFlags::parse_from_filename(filename).unwrap();
    assert!(flags.seen);
    assert!(!flags.replied);

    let expected: HashSet<char> = vec!['a', 'b', 'c'].into_iter().collect();
    assert_eq!(flags.custom_keywords, expected);
}

#[test]
fn test_parse_maildir_flags_none() {
    let filename = "12345.M6789P123.host:2,";
    let flags = MaildirFlags::parse_from_filename(filename).unwrap();
    assert!(!flags.passed);
    assert!(!flags.replied);
    assert!(!flags.seen);
    assert!(!flags.trashed);
    assert!(!flags.draft);
    assert!(!flags.flagged);
    assert!(flags.custom_keywords.is_empty());
}

#[test]
fn test_parse_maildir_flags_no_suffix() {
    let filename = "12345.M6789P123.host";
    let flags = MaildirFlags::parse_from_filename(filename).unwrap();
    assert_eq!(flags, MaildirFlags::new());
}

#[test]
fn test_serialize_maildir_flags() {
    let mut flags = MaildirFlags::new();
    flags.seen = true;
    flags.replied = true;
    flags.trashed = true;
    flags.draft = true;
    flags.flagged = true;
    flags.passed = true;

    flags.custom_keywords.insert('c');
    flags.custom_keywords.insert('a');
    flags.custom_keywords.insert('b');

    // Expected order: D, F, P, R, S, T, a, b, c
    let expected = ":2,DFPRSTabc";
    assert_eq!(flags.serialize(), expected);
}

#[test]
fn test_apply_to_filename() {
    let filename = "12345.M6789P123.host";
    let mut flags = MaildirFlags::new();
    flags.seen = true;

    assert_eq!(flags.apply_to_filename(filename), "12345.M6789P123.host:2,S");
}

#[test]
fn test_apply_to_filename_existing() {
    let filename = "12345.M6789P123.host:2,R";
    let mut flags = MaildirFlags::new();
    flags.seen = true;
    flags.replied = true;

    assert_eq!(flags.apply_to_filename(filename), "12345.M6789P123.host:2,RS");
}

#[test]
fn test_dovecot_keywords_load_save() {
    let file = NamedTempFile::new().unwrap();
    let path = file.path();

    let mut keywords = DovecotKeywords::new();
    keywords.register_keyword("NonJunk").unwrap();
    keywords.register_keyword("Junk").unwrap();

    keywords.save(path).unwrap();

    let loaded = DovecotKeywords::load(path).unwrap();
    assert_eq!(loaded.index_to_keyword.len(), 2);
    assert_eq!(loaded.index_to_keyword.get(&0).unwrap(), "NonJunk");
    assert_eq!(loaded.index_to_keyword.get(&1).unwrap(), "Junk");
    assert_eq!(*loaded.keyword_to_index.get("NonJunk").unwrap(), 0);
    assert_eq!(*loaded.keyword_to_index.get("Junk").unwrap(), 1);
}

#[test]
fn test_dovecot_keywords_register() {
    let mut keywords = DovecotKeywords::new();
    let c1 = keywords.register_keyword("NonJunk").unwrap();
    assert_eq!(c1, 'a');

    let c2 = keywords.register_keyword("Junk").unwrap();
    assert_eq!(c2, 'b');

    let c3 = keywords.register_keyword("NonJunk").unwrap();
    assert_eq!(c3, 'a'); // Returns existing index
}

#[test]
fn test_dovecot_keywords_limit() {
    let mut keywords = DovecotKeywords::new();
    for i in 0..26 {
        keywords.register_keyword(&format!("Keyword{}", i)).unwrap();
    }

    let result = keywords.register_keyword("Keyword26");
    assert!(matches!(result, Err(MaildirError::KeywordLimitExceeded)));
}

#[test]
fn test_dovecot_keywords_resolve() {
    let mut keywords = DovecotKeywords::new();
    keywords.register_keyword("NonJunk").unwrap(); // 'a'
    keywords.register_keyword("Junk").unwrap(); // 'b'
    keywords.register_keyword("Test").unwrap(); // 'c'

    let mut flags = HashSet::new();
    flags.insert('a');
    flags.insert('c');
    flags.insert('z'); // Does not exist

    let mut resolved = keywords.resolve_flags(&flags);
    resolved.sort();

    let mut expected = vec!["NonJunk".to_string(), "Test".to_string()];
    expected.sort();

    assert_eq!(resolved, expected);
}
