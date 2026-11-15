#![forbid(unsafe_code)]

use backup_destinations::azure_sas::{AzureSasBuilder, decode_b64, encode_b64};

#[test]
fn test_base64_decode() {
    let b64 = "dGVzdGtleQ==";
    let decoded = decode_b64(b64).unwrap();
    assert_eq!(decoded, b"testkey");
}

#[test]
fn test_base64_encode() {
    let data = b"testkey";
    let encoded = encode_b64(data);
    assert_eq!(encoded, "dGVzdGtleQ==");
}

#[test]
fn test_azure_sas_generation() {
    // Expected output based on Python generation using version 2020-02-10
    // st=2024-01-01T00:00:00Z
    // se=2025-01-01T00:00:00Z
    // sp=rwd
    // spr=https
    // sv=2020-02-10
    // sr=c
    // sig computed in python test_hmac_azure_2020_02_10.py: QyKl1a9BXDiybbzYYzmYLj2Gyc93mqVFsGtQOwUTuZY= (urldencoded: QyKl1a9BXDiybbzYYzmYLj2Gyc93mqVFsGtQOwUTuZY%3D)

    let builder = AzureSasBuilder::new("testaccount", "dGVzdGtleQ==", "testcontainer")
        .with_start_time("2024-01-01T00:00:00Z")
        .with_expiry_time("2025-01-01T00:00:00Z")
        .with_permissions("rwd")
        .with_protocol("https")
        .with_version("2020-02-10");

    let sas = builder.generate_sas_token().expect("Failed to generate sas token");

    // The order of the query string depends on the generate_sas_token implementation
    assert!(sas.contains("st=2024-01-01T00%3A00%3A00Z"));
    assert!(sas.contains("se=2025-01-01T00%3A00%3A00Z"));
    assert!(sas.contains("sp=rwd"));
    assert!(sas.contains("sv=2020-02-10"));
    assert!(sas.contains("sr=c"));
    assert!(sas.contains("spr=https"));
    assert!(sas.contains("sig=QyKl1a9BXDiybbzYYzmYLj2Gyc93mqVFsGtQOwUTuZY%3D"));
}
