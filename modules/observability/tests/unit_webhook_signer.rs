#![forbid(unsafe_code)]

use observability::webhook_signer::{WebhookSigner, WebhookVerificationError};

#[test]
fn test_valid_signature() {
    let secret = b"my_super_secret_key";
    let signer = WebhookSigner::new(secret.to_vec(), 300);

    let payload = r#"{"event": "alert", "message": "CPU High"}"#;
    let timestamp = 1620000000;

    let signature_header = signer.sign(payload, timestamp);
    assert!(signature_header.starts_with("t=1620000000,v1="));

    let result = signer.verify(payload, &signature_header, timestamp + 100);
    assert_eq!(result, Ok(()));
}

#[test]
fn test_expired_timestamp() {
    let secret = b"my_super_secret_key";
    let signer = WebhookSigner::new(secret.to_vec(), 300); // 5 minutes max age

    let payload = r#"{"event": "alert"}"#;
    let timestamp = 1620000000;

    let signature_header = signer.sign(payload, timestamp);

    // Simulate current time being 301 seconds after timestamp
    let result = signer.verify(payload, &signature_header, timestamp + 301);
    assert_eq!(result, Err(WebhookVerificationError::ExpiredTimestamp));
}

#[test]
fn test_future_timestamp() {
    let secret = b"my_super_secret_key";
    let signer = WebhookSigner::new(secret.to_vec(), 300);

    let payload = r#"{"event": "alert"}"#;
    let timestamp = 1620000000;

    let signature_header = signer.sign(payload, timestamp);

    // Simulate current time being 10 seconds before timestamp
    let result = signer.verify(payload, &signature_header, timestamp - 10);
    assert_eq!(result, Err(WebhookVerificationError::FutureTimestamp));
}

#[test]
fn test_invalid_signature() {
    let secret = b"my_super_secret_key";
    let signer = WebhookSigner::new(secret.to_vec(), 300);

    let payload = r#"{"event": "alert"}"#;
    let timestamp = 1620000000;

    let mut signature_header = signer.sign(payload, timestamp);
    // Tamper with the signature
    signature_header.replace_range(signature_header.len() - 5.., "abcde");

    let result = signer.verify(payload, &signature_header, timestamp);
    assert!(matches!(result, Err(WebhookVerificationError::InvalidFormat) | Err(WebhookVerificationError::InvalidSignature)));
}

#[test]
fn test_invalid_signature_hex() {
    let secret = b"my_super_secret_key";
    let signer = WebhookSigner::new(secret.to_vec(), 300);

    let payload = r#"{"event": "alert"}"#;
    let timestamp = 1620000000;

    let mut signature_header = signer.sign(payload, timestamp);
    // Tamper with the signature but keep valid hex
    if signature_header.ends_with('0') {
        signature_header.replace_range(signature_header.len() - 1.., "1");
    } else {
        signature_header.replace_range(signature_header.len() - 1.., "0");
    }

    let result = signer.verify(payload, &signature_header, timestamp);
    assert_eq!(result, Err(WebhookVerificationError::InvalidSignature));
}


#[test]
fn test_invalid_format() {
    let secret = b"my_super_secret_key";
    let signer = WebhookSigner::new(secret.to_vec(), 300);

    let payload = r#"{"event": "alert"}"#;
    let current_timestamp = 1620000000;

    let bad_header_1 = "v1=abcdef, t=123";
    assert_eq!(signer.verify(payload, bad_header_1, current_timestamp), Err(WebhookVerificationError::InvalidFormat));

    let bad_header_2 = "t=notanumber,v1=abcdef";
    assert_eq!(signer.verify(payload, bad_header_2, current_timestamp), Err(WebhookVerificationError::InvalidFormat));

    let bad_header_3 = "t=1620000000";
    assert_eq!(signer.verify(payload, bad_header_3, current_timestamp), Err(WebhookVerificationError::InvalidFormat));
}

#[test]
fn test_tampered_payload() {
    let secret = b"my_super_secret_key";
    let signer = WebhookSigner::new(secret.to_vec(), 300);

    let payload = r#"{"event": "alert"}"#;
    let tampered_payload = r#"{"event": "alert", "hacked": true}"#;
    let timestamp = 1620000000;

    let signature_header = signer.sign(payload, timestamp);

    let result = signer.verify(tampered_payload, &signature_header, timestamp);
    assert_eq!(result, Err(WebhookVerificationError::InvalidSignature));
}
