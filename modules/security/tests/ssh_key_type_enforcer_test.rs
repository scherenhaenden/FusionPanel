#![forbid(unsafe_code)]

use security::ssh_key_type_enforcer::{
    KeyClassification, SshKeySecurityAuditor,
};

fn encode_base64(data: &[u8]) -> String {
    let charset = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::new();
    let mut buffer = 0u32;
    let mut bits_in_buffer = 0;

    for &byte in data {
        buffer = (buffer << 8) | (byte as u32);
        bits_in_buffer += 8;

        while bits_in_buffer >= 6 {
            bits_in_buffer -= 6;
            let index = (buffer >> bits_in_buffer) & 0x3F;
            result.push(charset[index as usize] as char);
        }
    }

    if bits_in_buffer > 0 {
        buffer <<= 6 - bits_in_buffer;
        let index = buffer & 0x3F;
        result.push(charset[index as usize] as char);
    }

    while result.len() % 4 != 0 {
        result.push('=');
    }

    result
}

fn create_rsa_key_data(key_type: &str, bit_len: usize) -> Vec<u8> {
    let mut data = Vec::new();

    // Key type string
    let type_bytes = key_type.as_bytes();
    data.extend_from_slice(&(type_bytes.len() as u32).to_be_bytes());
    data.extend_from_slice(type_bytes);

    // 'e' string
    let e_bytes = [0x01, 0x00, 0x01]; // 65537
    data.extend_from_slice(&(e_bytes.len() as u32).to_be_bytes());
    data.extend_from_slice(&e_bytes);

    // 'n' string
    let byte_len = (bit_len + 7) / 8;
    let mut n_bytes = vec![0u8; byte_len];
    if byte_len > 0 {
        n_bytes[0] = 0x80; // Set highest bit so size doesn't truncate
    }

    // Sometimes 'n' is preceded by a leading zero if highest bit is set
    // In our simplified parser, we check leading zero. Let's just output it without leading zero
    // actually, let's prepend 0x00 so it counts correctly in OpenSSH format
    let mut actual_n = vec![0x00];
    actual_n.extend(n_bytes);

    data.extend_from_slice(&(actual_n.len() as u32).to_be_bytes());
    data.extend_from_slice(&actual_n);

    data
}

fn create_dsa_key_data() -> Vec<u8> {
    let mut data = Vec::new();
    let type_bytes = b"ssh-dss";
    data.extend_from_slice(&(type_bytes.len() as u32).to_be_bytes());
    data.extend_from_slice(type_bytes);
    data
}

fn create_ed25519_key_data() -> Vec<u8> {
    let mut data = Vec::new();
    let type_bytes = b"ssh-ed25519";
    data.extend_from_slice(&(type_bytes.len() as u32).to_be_bytes());
    data.extend_from_slice(type_bytes);
    data
}

#[test]
fn test_audit_authorized_keys_allowed() {
    let auditor = SshKeySecurityAuditor::default();

    let rsa_data = create_rsa_key_data("ssh-rsa", 4096);
    let rsa_b64 = encode_base64(&rsa_data);
    let ed_data = create_ed25519_key_data();
    let ed_b64 = encode_base64(&ed_data);

    let content = format!(
        "ssh-rsa {} user@example.com\nssh-ed25519 {} other@example.com",
        rsa_b64, ed_b64
    );

    let results = auditor.audit_authorized_keys(&content).unwrap();
    assert_eq!(results.len(), 2);

    assert_eq!(results[0].key_type, "ssh-rsa");
    assert_eq!(results[0].classification, KeyClassification::Allowed);
    assert_eq!(results[0].comment, Some("user@example.com".to_string()));

    assert_eq!(results[1].key_type, "ssh-ed25519");
    assert_eq!(results[1].classification, KeyClassification::Allowed);
}

#[test]
fn test_audit_authorized_keys_warning_and_rejected() {
    let auditor = SshKeySecurityAuditor::default();

    let rsa_2048 = encode_base64(&create_rsa_key_data("ssh-rsa", 2048));
    let rsa_1024 = encode_base64(&create_rsa_key_data("ssh-rsa", 1024));
    let dsa_key = encode_base64(&create_dsa_key_data());

    let content = format!(
        "ssh-rsa {} warn\nssh-rsa {} reject\nssh-dss {} dsa",
        rsa_2048, rsa_1024, dsa_key
    );

    let results = auditor.audit_authorized_keys(&content).unwrap();
    assert_eq!(results.len(), 3);

    assert_eq!(results[0].classification, KeyClassification::Warning("RSA key size 2048 is below recommended minimum 3072".to_string()));
    assert_eq!(results[1].classification, KeyClassification::Rejected("RSA key size 1024 is below allowed minimum 2048".to_string()));
    assert!(matches!(results[2].classification, KeyClassification::Rejected(_)));
}

#[test]
fn test_audit_sshd_config() {
    let auditor = SshKeySecurityAuditor::default();

    let content = "
        Port 22
        # Weak ciphers
        Ciphers 3des-cbc,aes128-ctr,arcfour

        # Weak MACs
        MACs hmac-md5,hmac-sha2-256

        # Weak KEX
        KexAlgorithms diffie-hellman-group1-sha1,curve25519-sha256
    ";

    let result = auditor.audit_sshd_config(content);

    assert_eq!(result.weak_ciphers, vec!["3des-cbc", "arcfour"]);
    assert_eq!(result.weak_macs, vec!["hmac-md5"]);
    assert_eq!(result.weak_kex, vec!["diffie-hellman-group1-sha1"]);

    assert_eq!(result.recommendations.len(), 3);
    assert!(result.recommendations[0].contains("3des-cbc"));
}
