#![forbid(unsafe_code)]

use dns::domain::{DomainName, Record, RecordType, TenantId, Zone};
use dns::zone_file::{generate_zone_file, next_serial};

fn domain(s: &str) -> DomainName {
    DomainName::parse(s).unwrap()
}

#[test]
fn test_next_serial() {
    // No previous serial
    assert_eq!(next_serial("20231024", None), 2023102401);

    // Old date serial
    assert_eq!(next_serial("20231024", Some(2023102301)), 2023102401);

    // Same date serial
    assert_eq!(next_serial("20231024", Some(2023102401)), 2023102402);

    // Max same date serial (should cap at 99 logic, or continue)
    assert_eq!(next_serial("20231024", Some(2023102498)), 2023102499);
    assert_eq!(next_serial("20231024", Some(2023102499)), 2023102499); // Capped according to logic
}

#[test]
fn test_generate_zone_file_basic() {
    let tenant_id = TenantId::new();
    let mut zone = Zone::new(domain("example.com"), tenant_id);

    zone.add_record(
        Record::new(
            domain("example.com"),
            RecordType::A,
            3600,
            "192.0.2.1".to_string(),
        )
        .unwrap(),
    )
    .unwrap();
    zone.add_record(
        Record::new(
            domain("www.example.com"),
            RecordType::CNAME,
            3600,
            "example.com".to_string(),
        )
        .unwrap(),
    )
    .unwrap();
    zone.add_record(
        Record::new(
            domain("example.com"),
            RecordType::MX,
            3600,
            "10 mail.example.com".to_string(),
        )
        .unwrap(),
    )
    .unwrap();
    zone.add_record(
        Record::new(
            domain("example.com"),
            RecordType::TXT,
            3600,
            "v=spf1 -all".to_string(),
        )
        .unwrap(),
    )
    .unwrap();

    let output = generate_zone_file(
        &zone,
        &domain("ns1.example.com"),
        "admin@example.com",
        "20231024",
        Some(2023102401),
    )
    .unwrap();

    assert!(output.contains("$ORIGIN example.com."));
    assert!(output.contains("@ 3600 IN SOA ns1.example.com. admin.example.com. ("));
    assert!(output.contains("2023102402 ; serial"));
    assert!(output.contains("@ 3600 IN A 192.0.2.1"));
    assert!(output.contains("www 3600 IN CNAME example.com."));
    assert!(output.contains("@ 3600 IN MX 10 mail.example.com"));
    assert!(output.contains("@ 3600 IN TXT \"v=spf1 -all\""));
}

#[test]
fn test_txt_escaping() {
    let tenant_id = TenantId::new();
    let mut zone = Zone::new(domain("example.com"), tenant_id);

    // Testing quotes inside txt data
    zone.add_record(
        Record::new(
            domain("example.com"),
            RecordType::TXT,
            3600,
            "has \"quotes\"".to_string(),
        )
        .unwrap(),
    )
    .unwrap();

    let output = generate_zone_file(
        &zone,
        &domain("ns1.example.com"),
        "admin@example.com",
        "20231024",
        None,
    )
    .unwrap();

    assert!(output.contains("@ 3600 IN TXT \"has \\\"quotes\\\"\""));
}

#[test]
fn test_relative_vs_absolute_names() {
    let tenant_id = TenantId::new();
    let mut zone = Zone::new(domain("example.com"), tenant_id);

    // Relative
    zone.add_record(
        Record::new(
            domain("sub.example.com"),
            RecordType::A,
            3600,
            "192.0.2.2".to_string(),
        )
        .unwrap(),
    )
    .unwrap();

    let output = generate_zone_file(
        &zone,
        &domain("ns1.example.com"),
        "admin.example.com", // Already dot formatted
        "20231024",
        None,
    )
    .unwrap();

    assert!(output.contains("sub 3600 IN A 192.0.2.2"));
    assert!(output.contains("@ 3600 IN SOA ns1.example.com. admin.example.com. ("));
    // Email unchanged since no @
}
