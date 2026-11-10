#![forbid(unsafe_code)]

use databases::postgres_pitr::{PitrValidator, WalFile};

#[test]
fn test_wal_parse_valid() {
    let wal = WalFile::parse("0000000100000002000000FF").unwrap();
    assert_eq!(wal.timeline, 1);
    assert_eq!(wal.log, 2);
    assert_eq!(wal.seg, 255);
    assert_eq!(wal.to_filename(), "0000000100000002000000FF");
}

#[test]
fn test_wal_parse_invalid_length() {
    assert!(WalFile::parse("0000000100000002000000F").is_err());
    assert!(WalFile::parse("0000000100000002000000FFF").is_err());
}

#[test]
fn test_wal_parse_invalid_hex() {
    assert!(WalFile::parse("G000000100000002000000FF").is_err());
    assert!(WalFile::parse("00000001G0000002000000FF").is_err());
    assert!(WalFile::parse("0000000100000002000000FG").is_err());
}

#[test]
fn test_wal_absolute_index() {
    let wal1 = WalFile::parse("000000010000000000000000").unwrap();
    assert_eq!(wal1.absolute_index(), 0);

    let wal2 = WalFile::parse("0000000100000000000000FF").unwrap();
    assert_eq!(wal2.absolute_index(), 255);

    let wal3 = WalFile::parse("000000010000000100000000").unwrap();
    assert_eq!(wal3.absolute_index(), 256);
}

#[test]
fn test_validate_sequence_empty() {
    let result = PitrValidator::validate_sequence(&[]);
    assert!(result.is_continuous);
    assert!(result.missing_segments.is_empty());
    assert_eq!(result.target_timeline, 0);
}

#[test]
fn test_validate_sequence_continuous_single_timeline() {
    let files = vec![
        WalFile::parse("0000000100000000000000FE").unwrap(),
        WalFile::parse("0000000100000000000000FF").unwrap(),
        WalFile::parse("000000010000000100000000").unwrap(),
        WalFile::parse("000000010000000100000001").unwrap(),
    ];
    let result = PitrValidator::validate_sequence(&files);
    assert!(result.is_continuous);
    assert!(result.missing_segments.is_empty());
    assert_eq!(result.target_timeline, 1);
}

#[test]
fn test_validate_sequence_gap() {
    let files = vec![
        WalFile::parse("0000000100000000000000FE").unwrap(),
        WalFile::parse("000000010000000100000001").unwrap(),
    ];
    let result = PitrValidator::validate_sequence(&files);
    assert!(!result.is_continuous);
    assert_eq!(result.missing_segments.len(), 2);
    assert_eq!(result.missing_segments[0], "0000000100000000000000FF");
    assert_eq!(result.missing_segments[1], "000000010000000100000000");
    assert_eq!(result.target_timeline, 1);
}

#[test]
fn test_validate_sequence_timeline_switch() {
    let files = vec![
        WalFile::parse("0000000100000000000000FE").unwrap(),
        WalFile::parse("0000000100000000000000FF").unwrap(),
        WalFile::parse("0000000200000000000000FF").unwrap(),
        WalFile::parse("000000020000000100000000").unwrap(),
    ];
    let result = PitrValidator::validate_sequence(&files);
    assert!(result.is_continuous);
    assert!(result.missing_segments.is_empty());
    assert_eq!(result.target_timeline, 2);
}

#[test]
fn test_validate_sequence_gap_timeline_switch() {
    let files = vec![
        WalFile::parse("0000000100000000000000FE").unwrap(),
        WalFile::parse("000000020000000100000000").unwrap(),
    ];
    let result = PitrValidator::validate_sequence(&files);
    assert!(!result.is_continuous);
    assert_eq!(result.missing_segments.len(), 1);
    // The missing segment should be on the previous timeline before the switch
    assert_eq!(result.missing_segments[0], "0000000100000000000000FF");
    assert_eq!(result.target_timeline, 2);
}

#[test]
fn test_validate_sequence_unsorted() {
    let files = vec![
        WalFile::parse("000000010000000100000001").unwrap(),
        WalFile::parse("0000000100000000000000FF").unwrap(),
        WalFile::parse("0000000100000000000000FE").unwrap(),
        WalFile::parse("000000010000000100000000").unwrap(),
    ];
    let result = PitrValidator::validate_sequence(&files);
    assert!(result.is_continuous);
    assert!(result.missing_segments.is_empty());
    assert_eq!(result.target_timeline, 1);
}

#[test]
fn test_validate_sequence_gap_after_continuous_timeline_switch() {
    let files = vec![
        WalFile::parse("0000000100000000000000FE").unwrap(),
        WalFile::parse("0000000200000000000000FF").unwrap(), // continuous timeline switch
        WalFile::parse("000000020000000100000001").unwrap(), // gap afterwards
    ];
    let result = PitrValidator::validate_sequence(&files);
    assert!(!result.is_continuous);
    assert_eq!(result.missing_segments.len(), 1);
    // The missing segment should be on the new timeline (2)
    assert_eq!(result.missing_segments[0], "000000020000000100000000");
    assert_eq!(result.target_timeline, 2);
}
