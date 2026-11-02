use mail::dns_records::{DkimKeyType, DnsGenerator, DnsRecordType};

#[test]
fn test_mx_record_generation() {
    let generator = DnsGenerator::new("example.com").unwrap();
    let mx = generator.generate_mx("mail.example.com");

    assert_eq!(mx.name, "example.com");
    assert_eq!(mx.record_type, DnsRecordType::MX);
    assert_eq!(mx.value, "mail.example.com");
    assert_eq!(mx.priority, Some(10));
}

#[test]
fn test_spf_record_generation() {
    let generator = DnsGenerator::new("example.com").unwrap();
    let spf = generator.generate_spf(&["192.0.2.1", "2001:db8::1"], &["_spf.example.net"]);

    assert_eq!(spf.name, "example.com");
    assert_eq!(spf.record_type, DnsRecordType::TXT);
    assert_eq!(
        spf.value,
        "\"v=spf1 ip4:192.0.2.1 ip6:2001:db8::1 include:_spf.example.net -all\""
    );
    assert_eq!(spf.priority, None);
}

#[test]
fn test_dmarc_record_generation() {
    let generator = DnsGenerator::new("example.com").unwrap();
    let dmarc = generator.generate_dmarc("postmaster@example.com");

    assert_eq!(dmarc.name, "_dmarc.example.com");
    assert_eq!(dmarc.record_type, DnsRecordType::TXT);
    assert_eq!(
        dmarc.value,
        "\"v=DMARC1; p=reject; rua=mailto:postmaster@example.com;\""
    );
    assert_eq!(dmarc.priority, None);
}

#[test]
fn test_dkim_wrapper_graceful_blocked() {
    let generator = DnsGenerator::new("example.com").unwrap();
    // This will likely fail in a sandbox environment without opendkim-genkey installed
    // We expect it to fail gracefully with BLOCKED error string
    let result = generator.generate_dkim("selector1", DkimKeyType::Rsa);

    match result {
        Ok(_) => {
            // If it surprisingly succeeds, that's fine too.
        }
        Err(e) => {
            assert!(
                e.contains("BLOCKED"),
                "Error should contain BLOCKED indicator: {}",
                e
            );
        }
    }
}

#[test]
fn test_invalid_domain_rejection() {
    // We reject non-ASCII domains as per instructions (or returning Err)
    assert!(DnsGenerator::new("exàmple.com").is_err());
    assert!(DnsGenerator::new("").is_err());
}

#[test]
fn test_string_escaping() {
    let generator = DnsGenerator::new("example.com").unwrap();

    let spf = generator.generate_spf(&["1.2.3.4\""], &["evil.com\" include:other"]);
    assert!(
        !spf.value.contains('"')
            || spf.value.starts_with('"')
                && spf.value.ends_with('"')
                && spf.value.chars().filter(|&c| c == '"').count() == 2
    );

    let dmarc = generator.generate_dmarc("postmaster@example.com\"");
    assert!(!dmarc.value.contains("postmaster@example.com\""));
}

#[test]
fn test_selector_validation() {
    let generator = DnsGenerator::new("example.com").unwrap();
    // Test invalid selectors for path traversal prevention
    assert!(generator
        .generate_dkim("../../../etc/shadow", DkimKeyType::Rsa)
        .is_err());
    assert!(generator
        .generate_dkim("select/or", DkimKeyType::Rsa)
        .is_err());
    assert!(generator
        .generate_dkim("select#or", DkimKeyType::Rsa)
        .is_err());
}
