use crate::domain::{DomainName, Record, RecordType, TenantId, Zone};
use serde_json::json;

#[test]
fn test_domain_name_valid() {
    let name = DomainName::parse("example.com").unwrap();
    assert_eq!(name.as_str(), "example.com");

    let name2 = DomainName::parse("sub-domain.example.com.").unwrap();
    assert_eq!(name2.as_str(), "sub-domain.example.com");
}

#[test]
fn test_domain_name_invalid_cases() {
    assert!(DomainName::parse("").is_err());

    let long_label = "a".repeat(64) + ".com";
    assert!(DomainName::parse(&long_label).is_err());

    let long_domain = "a.com".repeat(60);
    assert!(DomainName::parse(&long_domain).is_err());

    assert!(DomainName::parse("xn--bcher-kva.ch").is_ok());
    assert!(DomainName::parse("bücher.ch").is_err());

    assert!(DomainName::parse("192.168.1.1").is_err());
    assert!(DomainName::parse("2001:db8::1").is_err());

    assert!(DomainName::parse("test_domain.com").is_ok());
    assert!(DomainName::parse("test!domain.com").is_err());

    assert!(DomainName::parse("-test.com").is_err());
    assert!(DomainName::parse("test-.com").is_err());
}

#[test]
fn test_domain_name_empty_label() {
    assert!(DomainName::parse("example..com").is_err());
    assert!(DomainName::parse("example.com..").is_err());
}

#[test]
fn test_record_validation() {
    let name = DomainName::parse("example.com").unwrap();

    assert!(Record::new(name.clone(), RecordType::A, 3600, "1.2.3.4".to_string()).is_ok());
    assert!(Record::new(name.clone(), RecordType::A, 3600, "not.an.ip".to_string()).is_err());
    assert!(Record::new(name.clone(), RecordType::AAAA, 3600, "2001:db8::1".to_string()).is_ok());
    assert!(Record::new(name.clone(), RecordType::CNAME, 3600, "target.example.com".to_string()).is_ok());
    assert!(Record::new(name.clone(), RecordType::MX, 3600, "10 mail.example.com".to_string()).is_ok());
    assert!(Record::new(name.clone(), RecordType::MX, 3600, "0 .".to_string()).is_ok());
    assert!(Record::new(name.clone(), RecordType::MX, 3600, "mail.example.com".to_string()).is_err());
    assert!(Record::new(name.clone(), RecordType::TXT, 3600, "v=spf1 -all".to_string()).is_ok());

    assert!(Record::new(name.clone(), RecordType::A, 0, "1.2.3.4".to_string()).is_err());
}

#[test]
fn test_zone_creation() {
    let name = DomainName::parse("example.com").unwrap();
    let tenant_id = TenantId::new();
    let mut zone = Zone::new(name.clone(), tenant_id);

    assert_eq!(zone.records().len(), 0);
    assert_eq!(zone.name, name);
    assert_eq!(zone.tenant_id, tenant_id);

    let record = Record::new(name.clone(), RecordType::A, 3600, "1.2.3.4".to_string()).unwrap();
    assert!(zone.add_record(record).is_ok());
    assert_eq!(zone.records().len(), 1);

    // Attacker record
    let attacker = DomainName::parse("attacker.net").unwrap();
    let attacker_record = Record::new(attacker, RecordType::A, 3600, "1.2.3.4".to_string()).unwrap();
    assert!(zone.add_record(attacker_record).is_err());
}

#[test]
fn test_deserialization_validation() {
    let invalid_json = json!({
        "id": uuid::Uuid::new_v4().to_string(),
        "name": "invalid..name",
        "tenant_id": uuid::Uuid::new_v4().to_string(),
        "records": []
    });

    let result: Result<Zone, _> = serde_json::from_value(invalid_json);
    assert!(result.is_err());
}

#[cfg(test)]
mod integration_tests {
    // use super::*;
    use std::process::Command;

    #[test]
    #[ignore = "BLOCKED: Real authoritative DNS backend (e.g., local bind9 or powerdns container) is not available at 127.0.0.1:5353. Dependency missing."]
    fn e2e_verify_dns_backend_blocked() {
        // According to instructions: "If runtime prerequisites are not available yet,
        // provide executable case definitions and record BLOCKED with the exact dependency.
        // A blocked/unrun case is not a pass."

        let dig_output = Command::new("dig")
            .arg("@127.0.0.1")
            .arg("-p")
            .arg("5353") // Hypothetical local authoritative backend port
            .arg("example.com")
            .output();

        match dig_output {
            Ok(output) => {
                if !output.status.success() || String::from_utf8_lossy(&output.stdout).contains("connection refused") {
                    panic!("BLOCKED: Real authoritative DNS backend (e.g., local bind9 or powerdns container) is not available at 127.0.0.1:5353.");
                }

                // If it succeeded, we would parse output and assert it matches the expected zone restoration.
                // 1. Create/update record
                // 2. Resolve via DNS
                // 3. Verify exported zone restoration
                assert!(String::from_utf8_lossy(&output.stdout).contains("status: NOERROR"));
            }
            Err(_) => {
                panic!("BLOCKED: 'dig' command not found or other execution error. Real authoritative DNS backend is not available.");
            }
        }
    }
}
