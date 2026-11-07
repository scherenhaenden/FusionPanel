use crate::domain::{DomainName, Record, RecordType, TenantId, Zone};
use crate::provider::{DnssecKeyMaterial, DnssecStatus};

fn domain(value: &str) -> DomainName {
    DomainName::parse(value).expect("valid test domain")
}

#[test]
fn domain_names_are_canonicalized_and_accept_one_root_dot() {
    assert_eq!(domain("Example.COM").as_str(), "example.com");
    assert_eq!(
        domain("sub-domain.example.com.").as_str(),
        "sub-domain.example.com"
    );
    assert_eq!(domain("xn--bcher-kva.ch").as_str(), "xn--bcher-kva.ch");
}

#[test]
fn domain_names_reject_invalid_labels_and_lengths() {
    assert!(DomainName::parse("").is_err());
    assert!(DomainName::parse("example..com").is_err());
    assert!(DomainName::parse("example.com..").is_err());
    assert!(DomainName::parse("bücher.ch").is_err());
    assert!(DomainName::parse("test!domain.com").is_err());
    assert!(DomainName::parse("-test.com").is_err());
    assert!(DomainName::parse("test-.com").is_err());
    assert!(DomainName::parse(&format!("{}.com", "a".repeat(64))).is_err());
    assert!(DomainName::parse(&"a.com".repeat(60)).is_err());
}

#[test]
fn domain_names_reject_ipv4_literals_and_legacy_numeric_forms() {
    for address in [
        "192.168.1.1",
        "127.1",
        "2130706433",
        "0x7f000001",
        "0177.0.0.1",
        "0x7f.1",
        "127.0.0.1.",
    ] {
        assert!(
            DomainName::parse(address).is_err(),
            "numeric IP form {address:?} must not parse as a domain"
        );
    }
    assert!(DomainName::parse("2001:db8::1").is_err());
    assert!(DomainName::parse("example.123").is_ok());
}

#[test]
fn domain_name_deserialization_uses_the_validated_constructor() {
    let normalized: DomainName = serde_json::from_str("\"Example.COM.\"").unwrap();
    assert_eq!(normalized.as_str(), "example.com");

    for invalid in ["\"\"", "\"example..com\"", "\"127.1\"", "\"bücher.ch\""] {
        assert!(
            serde_json::from_str::<DomainName>(invalid).is_err(),
            "invalid serialized domain {invalid} must be rejected"
        );
    }
}

#[test]
fn records_validate_ttl_and_type_specific_data() {
    let owner = domain("example.com");
    assert!(Record::new(owner.clone(), RecordType::A, 3600, "192.0.2.1".into()).is_ok());
    assert!(Record::new(owner.clone(), RecordType::A, 3600, "not.an.ip".into()).is_err());
    assert!(Record::new(owner.clone(), RecordType::AAAA, 3600, "2001:db8::1".into()).is_ok());
    assert!(Record::new(
        owner.clone(),
        RecordType::CNAME,
        3600,
        "target.example.com".into()
    )
    .is_ok());
    assert!(Record::new(
        owner.clone(),
        RecordType::MX,
        3600,
        "10 mail.example.com".into()
    )
    .is_ok());
    assert!(Record::new(
        owner.clone(),
        RecordType::MX,
        3600,
        "mail.example.com".into()
    )
    .is_err());
    assert!(Record::new(owner.clone(), RecordType::TXT, 3600, "v=spf1 -all".into()).is_ok());

    let zero_ttl = Record::new(owner.clone(), RecordType::A, 0, "192.0.2.1".into()).unwrap();
    assert_eq!(zero_ttl.ttl(), 0);
    assert!(Record::new(
        owner.clone(),
        RecordType::A,
        2_147_483_647,
        "192.0.2.1".into()
    )
    .is_ok());
    assert!(Record::new(owner, RecordType::A, 2_147_483_648, "192.0.2.1".into()).is_err());
}

#[test]
fn records_support_only_the_valid_null_mx_form() {
    let owner = domain("example.com");
    let null_mx = Record::new(owner.clone(), RecordType::MX, 3600, "0 .".into()).unwrap();
    assert_eq!(null_mx.data(), "0 .");
    assert!(Record::new(owner, RecordType::MX, 3600, "10 .".into()).is_err());
}

#[test]
fn record_deserialization_cannot_bypass_validation() {
    let invalid_ttl =
        r#"{"name":"example.com","record_type":"A","ttl":2147483648,"data":"192.0.2.1"}"#;
    let invalid_data = r#"{"name":"example.com","record_type":"A","ttl":60,"data":"invalid"}"#;
    let invalid_name = r#"{"name":"example..com","record_type":"A","ttl":60,"data":"192.0.2.1"}"#;
    for invalid in [invalid_ttl, invalid_data, invalid_name] {
        assert!(serde_json::from_str::<Record>(invalid).is_err());
    }

    let valid_zero_ttl = r#"{"name":"example.com","record_type":"A","ttl":0,"data":"192.0.2.1"}"#;
    assert_eq!(
        serde_json::from_str::<Record>(valid_zero_ttl)
            .unwrap()
            .ttl(),
        0
    );
}

#[test]
fn zones_accept_only_records_owned_by_the_zone() {
    let zone_name = domain("example.com");
    let tenant_id = TenantId::new();
    let mut zone = Zone::new(zone_name.clone(), tenant_id);
    assert_eq!(zone.records().len(), 0);
    assert_eq!(zone.name(), &zone_name);
    assert_eq!(zone.tenant_id(), tenant_id);

    let apex = Record::new(zone_name, RecordType::A, 60, "192.0.2.1".into()).unwrap();
    zone.add_record(apex).unwrap();
    let child = Record::new(
        domain("www.example.com"),
        RecordType::A,
        60,
        "192.0.2.2".into(),
    )
    .unwrap();
    zone.add_record(child).unwrap();
    let outside = Record::new(
        domain("notexample.com"),
        RecordType::A,
        60,
        "192.0.2.3".into(),
    )
    .unwrap();
    assert!(zone.add_record(outside).is_err());
    assert_eq!(zone.records().len(), 2);
}

#[test]
fn zones_reject_mixed_null_mx_and_normal_mx_records() {
    let mut zone = Zone::new(domain("example.com"), TenantId::new());
    let null_mx = Record::new(domain("example.com"), RecordType::MX, 60, "0 .".into()).unwrap();
    zone.add_record(null_mx).unwrap();
    let normal_mx = Record::new(
        domain("example.com"),
        RecordType::MX,
        60,
        "10 mail.example.com".into(),
    )
    .unwrap();
    assert!(zone.add_record(normal_mx).is_err());
}

#[test]
fn zone_deserialization_preserves_record_validation_and_ownership() {
    let tenant_id = TenantId::new();
    let outside_zone = serde_json::json!({
        "id": uuid::Uuid::new_v4(),
        "name": "example.com",
        "tenant_id": tenant_id,
        "records": [{
            "name": "attacker.net",
            "record_type": "A",
            "ttl": 60,
            "data": "192.0.2.1"
        }]
    });
    assert!(serde_json::from_value::<Zone>(outside_zone).is_err());

    let invalid_record = serde_json::json!({
        "id": uuid::Uuid::new_v4(),
        "name": "example.com",
        "tenant_id": tenant_id,
        "records": [{
            "name": "www.example.com",
            "record_type": "A",
            "ttl": 60,
            "data": "not-an-ip"
        }]
    });
    assert!(serde_json::from_value::<Zone>(invalid_record).is_err());
}

#[test]
fn dnssec_status_uses_discriminated_encoded_material() {
    let status = DnssecStatus {
        enabled: true,
        key_material: vec![
            DnssecKeyMaterial::Dnskey {
                flags: 257,
                protocol: 3,
                algorithm: 13,
                public_key_base64: "AQID".into(),
            },
            DnssecKeyMaterial::Ds {
                key_tag: 12345,
                algorithm: 13,
                digest_type: 2,
                digest_hex: "A1B2C3".into(),
            },
        ],
    };
    let value = serde_json::to_value(status).unwrap();
    assert_eq!(value["key_material"][0]["kind"], "dnskey");
    assert_eq!(value["key_material"][0]["public_key_base64"], "AQID");
    assert_eq!(value["key_material"][1]["kind"], "ds");
    assert_eq!(value["key_material"][1]["digest_hex"], "A1B2C3");
}

#[cfg(test)]
mod blocked_integration_tests {
    const BLOCKED_REASON: &str = "BLOCKED: this package has no concrete DNS provider implementation and no disposable authoritative DNS fixture, test zone, or endpoint. The S13 create/update, answer verification, and zone export/restore E2E cannot run until those dependencies are implemented and provisioned.";

    #[test]
    #[ignore = "BLOCKED: no concrete DNS provider or disposable authoritative DNS fixture is implemented/provisioned; see modules/dns/README.md"]
    fn e2e_authoritative_dns_create_update_resolve_restore() {
        panic!("{BLOCKED_REASON}");
    }
}
