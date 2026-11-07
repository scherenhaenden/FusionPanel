use security::tls_audit::{TlsConfig, TlsProtocol};

#[test]
fn test_modern_profile() {
    let config = TlsConfig::new(
        vec![TlsProtocol::Tls1_3],
        vec![
            "TLS_AES_128_GCM_SHA256".to_string(),
            "TLS_AES_256_GCM_SHA384".to_string(),
            "TLS_CHACHA20_POLY1305_SHA256".to_string(),
        ],
    );
    let audit = config.audit();
    assert!(audit.is_modern);
    assert!(!audit.is_intermediate);
    assert!(!audit.has_weak_ciphers);
    assert!(audit.weak_ciphers.is_empty());
    assert!(audit.deprecated_protocols.is_empty());
}

#[test]
fn test_intermediate_profile() {
    let config = TlsConfig::new(
        vec![TlsProtocol::Tls1_2, TlsProtocol::Tls1_3],
        vec![
            "ECDHE-ECDSA-AES128-GCM-SHA256".to_string(),
            "ECDHE-RSA-AES128-GCM-SHA256".to_string(),
            "TLS_AES_128_GCM_SHA256".to_string(),
        ],
    );
    let audit = config.audit();
    assert!(!audit.is_modern);
    assert!(audit.is_intermediate);
    assert!(!audit.has_weak_ciphers);
    assert!(audit.weak_ciphers.is_empty());
    assert!(audit.deprecated_protocols.is_empty());
}

#[test]
fn test_weak_ciphers() {
    let config = TlsConfig::new(
        vec![TlsProtocol::Tls1_2],
        vec![
            "ECDHE-RSA-AES256-SHA384".to_string(),
            "AES256-SHA256".to_string(),
            "DES-CBC3-SHA".to_string(),
            "RC4-SHA".to_string(),
        ],
    );
    let audit = config.audit();
    assert!(!audit.is_modern);
    assert!(!audit.is_intermediate);
    assert!(audit.has_weak_ciphers);
    assert_eq!(audit.weak_ciphers, vec!["DES-CBC3-SHA".to_string(), "RC4-SHA".to_string()]);
    assert!(audit.deprecated_protocols.is_empty());
}

#[test]
fn test_deprecated_protocols() {
    let config = TlsConfig::new(
        vec![TlsProtocol::Tls1_0, TlsProtocol::Tls1_1, TlsProtocol::Tls1_2],
        vec![
            "ECDHE-RSA-AES128-GCM-SHA256".to_string(),
        ],
    );
    let audit = config.audit();
    assert!(!audit.is_modern);
    assert!(!audit.is_intermediate);
    assert!(!audit.has_weak_ciphers);
    assert_eq!(audit.deprecated_protocols, vec![TlsProtocol::Tls1_0, TlsProtocol::Tls1_1]);
}
