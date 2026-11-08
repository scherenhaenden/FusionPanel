#![forbid(unsafe_code)]

#[path = "../src/dns_routes.rs"]
mod dns_routes;

use dns_routes::{validate_record, DnsRecord, DnsZone, RecordType};

#[test]
fn test_zone_creation() {
    let zone = DnsZone::new("example.com".to_string(), 2023110201);
    assert_eq!(zone.name, "example.com");
    assert_eq!(zone.serial, 2023110201);
    assert!(zone.records.is_empty());
}

#[test]
fn test_add_record_increments_serial() {
    let mut zone = DnsZone::new("example.com".to_string(), 100);
    let record = DnsRecord {
        name: "@".to_string(),
        record_type: RecordType::A,
        value: "192.168.1.1".to_string(),
        ttl: 3600,
        priority: None,
    };

    assert!(zone.add_record(record.clone()).is_ok());
    assert_eq!(zone.serial, 101);
    assert_eq!(zone.records.len(), 1);
}

#[test]
fn test_update_record_increments_serial() {
    let mut zone = DnsZone::new("example.com".to_string(), 100);
    let record1 = DnsRecord {
        name: "@".to_string(),
        record_type: RecordType::A,
        value: "192.168.1.1".to_string(),
        ttl: 3600,
        priority: None,
    };
    zone.add_record(record1).unwrap();
    assert_eq!(zone.serial, 101);

    let record2 = DnsRecord {
        name: "@".to_string(),
        record_type: RecordType::A,
        value: "192.168.1.2".to_string(),
        ttl: 3600,
        priority: None,
    };
    assert!(zone.update_record(0, record2.clone()).is_ok());
    assert_eq!(zone.serial, 102);
    assert_eq!(zone.records[0].value, "192.168.1.2");
}

#[test]
fn test_delete_record_increments_serial() {
    let mut zone = DnsZone::new("example.com".to_string(), 100);
    let record = DnsRecord {
        name: "@".to_string(),
        record_type: RecordType::A,
        value: "192.168.1.1".to_string(),
        ttl: 3600,
        priority: None,
    };
    zone.add_record(record).unwrap();
    assert_eq!(zone.serial, 101);

    assert!(zone.delete_record(0).is_ok());
    assert_eq!(zone.serial, 102);
    assert!(zone.records.is_empty());
}

#[test]
fn test_validate_a_record() {
    let mut record = DnsRecord {
        name: "@".to_string(),
        record_type: RecordType::A,
        value: "192.168.1.1".to_string(),
        ttl: 3600,
        priority: None,
    };
    assert!(validate_record(&record).is_ok());

    record.value = "invalid-ip".to_string();
    assert!(validate_record(&record).is_err());
}

#[test]
fn test_validate_aaaa_record() {
    let mut record = DnsRecord {
        name: "@".to_string(),
        record_type: RecordType::AAAA,
        value: "2001:db8::1".to_string(),
        ttl: 3600,
        priority: None,
    };
    assert!(validate_record(&record).is_ok());

    record.value = "192.168.1.1".to_string(); // IPv4 instead of IPv6
    assert!(validate_record(&record).is_err());
}

#[test]
fn test_validate_mx_record() {
    let mut record = DnsRecord {
        name: "@".to_string(),
        record_type: RecordType::MX,
        value: "mail.example.com".to_string(),
        ttl: 3600,
        priority: Some(10),
    };
    assert!(validate_record(&record).is_ok());

    record.priority = None;
    assert!(validate_record(&record).is_err());
}

#[test]
fn test_validate_srv_record() {
    let mut record = DnsRecord {
        name: "_sip._tcp".to_string(),
        record_type: RecordType::SRV,
        value: "100 5060 sip.example.com".to_string(),
        ttl: 3600,
        priority: Some(10),
    };
    assert!(validate_record(&record).is_ok());

    record.priority = None;
    assert!(validate_record(&record).is_err());

    record.priority = Some(10);
    record.value = "100 5060".to_string(); // Missing target
    assert!(validate_record(&record).is_err());
}

#[test]
fn test_validate_caa_record() {
    let mut record = DnsRecord {
        name: "@".to_string(),
        record_type: RecordType::CAA,
        value: "0 issue letsencrypt.org".to_string(),
        ttl: 3600,
        priority: None,
    };
    assert!(validate_record(&record).is_ok());

    record.value = "0 invalid letsencrypt.org".to_string(); // Invalid tag
    assert!(validate_record(&record).is_err());
}

#[test]
fn test_empty_name_or_value() {
    let mut record = DnsRecord {
        name: "".to_string(),
        record_type: RecordType::TXT,
        value: "some text".to_string(),
        ttl: 3600,
        priority: None,
    };
    assert!(validate_record(&record).is_err());

    record.name = "@".to_string();
    record.value = "".to_string();
    assert!(validate_record(&record).is_err());
}

#[test]
fn test_cname_record() {
    let record = DnsRecord {
        name: "www".to_string(),
        record_type: RecordType::CNAME,
        value: "example.com".to_string(),
        ttl: 3600,
        priority: None,
    };
    assert!(validate_record(&record).is_ok());
}
