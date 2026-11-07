#![forbid(unsafe_code)]

// Test DKIM Rotation logic
use mail::dkim_rotation::DkimSelectorLifecycle;
use mail::dns_records::{DkimKeyPair, DkimKeyType, DnsRecord, DnsRecordType};

#[test]
fn test_lifecycle() {
    let domain = "example.com";
    let lifecycle = DkimSelectorLifecycle::new(domain, "202301");

    assert_eq!(lifecycle.active_selector, "202301");
    assert!(lifecycle.retired_selectors.is_empty());
}

#[test]
fn test_rotate_keys_blocked() {
    let domain = "example.com";
    let mut lifecycle = DkimSelectorLifecycle::new(domain, "202301");

    let result = lifecycle.rotate_keys(DkimKeyType::Rsa);
    match result {
        Ok(kp) => {
            assert_ne!(kp.selector, "202301");
            assert_eq!(lifecycle.retired_selectors.len(), 1);
            assert_eq!(lifecycle.retired_selectors[0], "202301");
        }
        Err(e) => {
            let error_str: String = e;
            assert!(error_str.contains("BLOCKED"), "Expected BLOCKED error if opendkim fails");
        }
    }
}

#[test]
fn test_get_all_txt_records() {
    let lifecycle = DkimSelectorLifecycle {
        domain: "example.com".to_string(),
        active_selector: "202302".to_string(),
        retired_selectors: vec!["202301".to_string()],
    };

    let current_kp = DkimKeyPair {
        selector: "202302".to_string(),
        private_key: "priv2".to_string(),
        txt_record: DnsRecord {
            name: "202302._domainkey.example.com".to_string(),
            record_type: DnsRecordType::TXT,
            value: "\"v=DKIM1; k=rsa; p=...2\"".to_string(),
            priority: None,
        }
    };

    let old_kps = vec![
        DkimKeyPair {
            selector: "202301".to_string(),
            private_key: "priv1".to_string(),
            txt_record: DnsRecord {
                name: "202301._domainkey.example.com".to_string(),
                record_type: DnsRecordType::TXT,
                value: "\"v=DKIM1; k=rsa; p=...1\"".to_string(),
                priority: None,
            }
        },
        DkimKeyPair {
            selector: "202212".to_string(),
            private_key: "priv0".to_string(),
            txt_record: DnsRecord {
                name: "202212._domainkey.example.com".to_string(),
                record_type: DnsRecordType::TXT,
                value: "\"v=DKIM1; k=rsa; p=...0\"".to_string(),
                priority: None,
            }
        }
    ];

    let records = lifecycle.get_all_txt_records(&current_kp, &old_kps);
    assert_eq!(records.len(), 2);
    assert!(records.iter().any(|r| r.name == "202302._domainkey.example.com"));
    assert!(records.iter().any(|r| r.name == "202301._domainkey.example.com"));
    assert!(!records.iter().any(|r| r.name == "202212._domainkey.example.com"));
}
