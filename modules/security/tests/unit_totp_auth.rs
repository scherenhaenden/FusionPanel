#![forbid(unsafe_code)]

use security::totp_auth::{TotpAuthenticator, TotpError, generate_totp};

#[test]
fn test_rfc_6238_vectors() {
    let secret = b"12345678901234567890";
    let vectors = [
        (59, "287082"),
        (1111111109, "081804"),
        (1111111111, "050471"),
        (1234567890, "005924"),
        (2000000000, "279037"),
        (20000000000, "353130"),
    ];

    for (time, expected_code) in vectors {
        let step = time / 30;
        let code = generate_totp(secret, step);
        assert_eq!(code, expected_code);
    }
}

#[test]
fn test_drift_and_replay() {
    let mut auth = TotpAuthenticator::new();
    let secret_b32 = "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ"; // Base32 for "12345678901234567890"

    let time = 59;

    // Test code for current step
    let code_curr = "287082";

    // Verify successful for current step
    assert!(auth.verify(secret_b32, code_curr, time).is_ok());

    // Replay attack should fail
    let err = auth.verify(secret_b32, code_curr, time).unwrap_err();
    assert!(matches!(err, TotpError::ReplayedCode));
}

#[test]
fn test_invalid_base32() {
    let mut auth = TotpAuthenticator::new();
    let secret_b32 = "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ1"; // Invalid char '1'

    let err = auth.verify(secret_b32, "123456", 59).unwrap_err();
    assert!(matches!(err, TotpError::InvalidSecret));
}

#[test]
fn test_drift_tolerance() {
    let mut auth = TotpAuthenticator::new();
    let secret_b32 = "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ"; // Base32 for "12345678901234567890"
    let time = 1000;
    let step = time / 30;

    let secret = b"12345678901234567890";
    let code_prev = generate_totp(secret, step - 1);
    let code_next = generate_totp(secret, step + 1);
    let code_future = generate_totp(secret, step + 2);

    // Prev step should work
    assert!(auth.verify(secret_b32, &code_prev, time).is_ok());

    // Next step should work
    assert!(auth.verify(secret_b32, &code_next, time).is_ok());

    // Future step should fail
    let err = auth.verify(secret_b32, &code_future, time).unwrap_err();
    assert!(matches!(err, TotpError::InvalidCode));
}
