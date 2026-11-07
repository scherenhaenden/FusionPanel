#![forbid(unsafe_code)]

use dns::dnssec_mgr::{KeyType, DnssecKeySpec, validate_rollover, format_ds_record};
use dns::domain::DomainName;

#[test]
fn test_key_spec_creation() {
    let ksk = DnssecKeySpec::new(KeyType::Ksk, 8, vec![1, 2, 3]);
    assert_eq!(ksk.flags, 257);
    assert_eq!(ksk.protocol, 3);
    assert_eq!(ksk.algorithm, 8);

    let zsk = DnssecKeySpec::new(KeyType::Zsk, 13, vec![4, 5, 6]);
    assert_eq!(zsk.flags, 256);
    assert_eq!(zsk.protocol, 3);
    assert_eq!(zsk.algorithm, 13);
}

#[test]
fn test_key_tag() {
    // RFC 4034 Appendix B example
    // RSA/SHA-1 key
    let flags: u16 = 256;
    let protocol: u8 = 3;
    let algorithm: u8 = 5; // RSASHA1

    let public_key = vec![
        0x03, 0x01, 0x00, 0x01, 0xA8, 0x00, 0x20, 0xA9, 0x55, 0x66, 0xBA, 0x42,
    ];
    let mut spec = DnssecKeySpec::new(KeyType::Zsk, algorithm, public_key);
    spec.flags = flags;
    spec.protocol = protocol;

    let ac = spec.key_tag();
    assert!(ac > 0);
}

#[test]
fn test_ds_record_format() {
    // RFC 4034 example: dskey.example.com
    let domain = DomainName::parse("dskey.example.com.").unwrap();

    let public_key = vec![
        0x03, 0x01, 0x00, 0x01, 0xA8, 0x00, 0x20, 0xA9, 0x55, 0x66, 0xBA, 0x42,
    ];
    let ksk = DnssecKeySpec::new(KeyType::Ksk, 5, public_key);

    let ds_record = format_ds_record(&domain, &ksk);

    let parts: Vec<&str> = ds_record.split_whitespace().collect();
    assert_eq!(parts.len(), 4);
    assert_eq!(parts[1], "5"); // algorithm
    assert_eq!(parts[2], "2"); // digest type 2 (SHA-256)
    assert_eq!(parts[3].len(), 64); // SHA-256 hex is 64 chars
}

#[test]
fn test_rollover_validation() {
    let ksk1 = DnssecKeySpec::new(KeyType::Ksk, 8, vec![1, 2, 3]);
    let ksk2 = DnssecKeySpec::new(KeyType::Ksk, 8, vec![4, 5, 6]);
    let zsk1 = DnssecKeySpec::new(KeyType::Zsk, 8, vec![7, 8, 9]);

    assert!(validate_rollover(&ksk1, &ksk2).is_ok());
    assert!(validate_rollover(&ksk1, &zsk1).is_err());
}
