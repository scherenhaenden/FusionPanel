#![forbid(unsafe_code)]
use dns::powerdns_sync::{canonicalize_domain, PdnsChangeType, PdnsRecord, PdnsRrset, PdnsZonePatch};

#[test]
fn test_canonicalize_domain() {
    assert_eq!(canonicalize_domain("example.com"), "example.com.");
    assert_eq!(canonicalize_domain("example.com."), "example.com.");
    assert_eq!(canonicalize_domain("  example.com  "), "example.com.");
    assert_eq!(canonicalize_domain("  example.com.  "), "example.com.");
}

#[test]
fn test_pdns_record_creation() {
    let record = PdnsRecord::new("192.168.1.1", false);
    assert_eq!(record.content, "192.168.1.1");
    assert!(!record.disabled);
}

#[test]
fn test_pdns_rrset_replace() {
    let records = vec![
        PdnsRecord::new("192.168.1.1", false),
        PdnsRecord::new("192.168.1.2", true),
    ];
    let rrset = PdnsRrset::replace("example.com", "A", 300, records.clone());

    assert_eq!(rrset.name, "example.com.");
    assert_eq!(rrset.record_type, "A");
    assert_eq!(rrset.ttl, 300);
    assert_eq!(rrset.changetype, PdnsChangeType::Replace);
    assert_eq!(rrset.records, records);
}

#[test]
fn test_pdns_rrset_delete() {
    let rrset = PdnsRrset::delete("example.com", "A");

    assert_eq!(rrset.name, "example.com.");
    assert_eq!(rrset.record_type, "A");
    assert_eq!(rrset.ttl, 0);
    assert_eq!(rrset.changetype, PdnsChangeType::Delete);
    assert!(rrset.records.is_empty());
}

#[test]
fn test_pdns_zone_patch_serialization() {
    let records = vec![PdnsRecord::new("192.168.1.1", false)];
    let rrset = PdnsRrset::replace("example.com", "A", 300, records);
    let patch = PdnsZonePatch::new(vec![rrset]);

    let json = serde_json::to_string(&patch).unwrap();
    let expected_json = r#"{"rrsets":[{"name":"example.com.","type":"A","ttl":300,"changetype":"REPLACE","records":[{"content":"192.168.1.1","disabled":false}]}]}"#;
    assert_eq!(json, expected_json);
}

#[test]
fn test_pdns_zone_patch_deserialization() {
    let json = r#"{
        "rrsets": [
            {
                "name": "example.com.",
                "type": "A",
                "ttl": 300,
                "changetype": "REPLACE",
                "records": [
                    {
                        "content": "192.168.1.1",
                        "disabled": false
                    }
                ]
            },
            {
                "name": "www.example.com.",
                "type": "CNAME",
                "ttl": 0,
                "changetype": "DELETE",
                "records": []
            }
        ]
    }"#;

    let patch: PdnsZonePatch = serde_json::from_str(json).unwrap();
    assert_eq!(patch.rrsets.len(), 2);

    assert_eq!(patch.rrsets[0].name, "example.com.");
    assert_eq!(patch.rrsets[0].record_type, "A");
    assert_eq!(patch.rrsets[0].ttl, 300);
    assert_eq!(patch.rrsets[0].changetype, PdnsChangeType::Replace);
    assert_eq!(patch.rrsets[0].records.len(), 1);
    assert_eq!(patch.rrsets[0].records[0].content, "192.168.1.1");
    assert!(!patch.rrsets[0].records[0].disabled);

    assert_eq!(patch.rrsets[1].name, "www.example.com.");
    assert_eq!(patch.rrsets[1].record_type, "CNAME");
    assert_eq!(patch.rrsets[1].ttl, 0);
    assert_eq!(patch.rrsets[1].changetype, PdnsChangeType::Delete);
    assert!(patch.rrsets[1].records.is_empty());
}
