#![forbid(unsafe_code)]

use dns::tsig_keyring::{Algorithm, TSIGKeyRing, TsigError, hmac_md5, hmac_sha256};

#[test]
fn test_hmac_sha256_basic() {
    let key = b"secret";
    let data = b"hello world";
    let mac = hmac_sha256(key, data);

    // A known good HMAC-SHA256 for key="secret" and data="hello world"
    // Computed with: echo -n "hello world" | openssl dgst -sha256 -hmac "secret"
    let expected = vec![
        0x73, 0x4c, 0xc6, 0x2f, 0x32, 0x84, 0x15, 0x68,
        0xf4, 0x57, 0x15, 0xae, 0xb9, 0xf4, 0xd7, 0x89,
        0x13, 0x24, 0xe6, 0xd9, 0x48, 0xe4, 0xc6, 0xc6,
        0x0c, 0x06, 0x21, 0xcd, 0xac, 0x48, 0x62, 0x3a,
    ];
    assert_eq!(mac, expected);
}

#[test]
fn test_hmac_md5_basic() {
    let key = b"secret";
    let data = b"hello world";
    let mac = hmac_md5(key, data);

    // A known good HMAC-MD5 for key="secret" and data="hello world"
    // Computed with: echo -n "hello world" | openssl dgst -md5 -hmac "secret"
    let expected = vec![
        0x78, 0xd6, 0x99, 0x7b, 0x12, 0x30, 0xf3, 0x8e,
        0x59, 0xb6, 0xd1, 0x64, 0x2d, 0xfa, 0xa3, 0xa4,
    ];
    assert_eq!(mac, expected);
}

#[test]
fn test_verify_tsig_success() {
    let mut keyring = TSIGKeyRing::new();
    keyring.add_key("test_key", Algorithm::HmacSha256, b"secret_key");

    let message = b"dns_update_payload";
    let valid_mac = hmac_sha256(b"secret_key", message);

    let current_time = 10000;

    let result = keyring.verify_tsig_request(
        "test_key",
        message,
        &valid_mac,
        current_time,
        300,
        current_time,
    );
    assert_eq!(result, Ok(()));
}

#[test]
fn test_verify_tsig_invalid_mac() {
    let mut keyring = TSIGKeyRing::new();
    keyring.add_key("test_key", Algorithm::HmacSha256, b"secret_key");

    let message = b"dns_update_payload";
    let mut invalid_mac = hmac_sha256(b"secret_key", message);
    invalid_mac[0] ^= 0xff; // Corrupt MAC

    let current_time = 10000;

    let result = keyring.verify_tsig_request(
        "test_key",
        message,
        &invalid_mac,
        current_time,
        300,
        current_time,
    );
    assert_eq!(result, Err(TsigError::InvalidSignature));
}

#[test]
fn test_verify_tsig_time_fudge_exceeded() {
    let mut keyring = TSIGKeyRing::new();
    keyring.add_key("test_key", Algorithm::HmacSha256, b"secret_key");

    let message = b"dns_update_payload";
    let valid_mac = hmac_sha256(b"secret_key", message);

    let request_time = 10000;
    let fudge = 300;

    // Future time outside fudge
    let result = keyring.verify_tsig_request(
        "test_key",
        message,
        &valid_mac,
        request_time,
        fudge,
        request_time + fudge + 1,
    );
    assert_eq!(result, Err(TsigError::TimeFudgeExceeded));

    // Past time outside fudge
    let result = keyring.verify_tsig_request(
        "test_key",
        message,
        &valid_mac,
        request_time,
        fudge,
        request_time - fudge - 1,
    );
    assert_eq!(result, Err(TsigError::TimeFudgeExceeded));
}

#[test]
fn test_verify_tsig_replay_detected() {
    let mut keyring = TSIGKeyRing::new();
    keyring.add_key("test_key", Algorithm::HmacSha256, b"secret_key");

    let message = b"dns_update_payload";
    let valid_mac = hmac_sha256(b"secret_key", message);

    let current_time = 10000;

    // First request should succeed
    let result = keyring.verify_tsig_request(
        "test_key",
        message,
        &valid_mac,
        current_time,
        300,
        current_time,
    );
    assert_eq!(result, Ok(()));

    // Second identical request should be detected as a replay
    let result = keyring.verify_tsig_request(
        "test_key",
        message,
        &valid_mac,
        current_time,
        300,
        current_time,
    );
    assert_eq!(result, Err(TsigError::ReplayDetected));
}

#[test]
fn test_verify_tsig_replay_cleans_up_old_requests() {
    let mut keyring = TSIGKeyRing::new();
    keyring.add_key("test_key", Algorithm::HmacSha256, b"secret_key");

    let message1 = b"dns_update_payload_1";
    let valid_mac1 = hmac_sha256(b"secret_key", message1);

    let time1 = 10000;

    // First request should succeed
    let result = keyring.verify_tsig_request(
        "test_key",
        message1,
        &valid_mac1,
        time1,
        300,
        time1,
    );
    assert_eq!(result, Ok(()));

    let message2 = b"dns_update_payload_2";
    let valid_mac2 = hmac_sha256(b"secret_key", message2);

    let time2 = time1 + 301; // Advance time past the fudge window of the first request

    // Second request should succeed, and internally it should prune the first request
    let result = keyring.verify_tsig_request(
        "test_key",
        message2,
        &valid_mac2,
        time2,
        300,
        time2,
    );
    assert_eq!(result, Ok(()));

    // Try replaying the first request again. It will fail with TimeFudgeExceeded first,
    // but if we were to replay something in the valid window it would check the replay cache.
    // Let's do a more precise test for replay buffer cleanup.

    let time3 = time2;
    // We replay message1 but fast forward request_time so it passes time bounds.
    let valid_mac1_time3 = hmac_sha256(b"secret_key", message1);

    let result = keyring.verify_tsig_request(
        "test_key",
        message1,
        &valid_mac1_time3,
        time3,
        300,
        time3,
    );
    assert_eq!(result, Ok(())); // It should succeed because it's essentially a new request at a new time.
}
