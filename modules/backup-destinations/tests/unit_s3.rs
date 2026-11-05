#![forbid(unsafe_code)]
use backup_destinations::s3::{S3Credentials, S3Error, S3StorageBackend};

#[test]
fn test_credential_redaction() {
    let creds = S3Credentials {
        access_key: "AKIAIOSFODNN7EXAMPLE".to_string(),
        secret_key: "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY".to_string(),
    };

    let debug_str = format!("{:?}", creds);
    assert!(!debug_str.contains("AKIAIOSFODNN7EXAMPLE"));
    assert!(!debug_str.contains("wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY"));
    assert!(debug_str.contains("***REDACTED***"));

    let display_str = format!("{}", creds);
    assert!(!display_str.contains("AKIAIOSFODNN7EXAMPLE"));
    assert!(!display_str.contains("wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY"));
    assert!(display_str.contains("***REDACTED***"));
}

#[test]
fn test_valid_endpoint_and_bucket() {
    let creds = S3Credentials {
        access_key: "test".to_string(),
        secret_key: "test".to_string(),
    };

    let backend = S3StorageBackend::new("https://s3.amazonaws.com", "my-bucket", creds.clone());
    assert!(backend.is_ok());

    let backend = S3StorageBackend::new(
        "https://<account_id>.r2.cloudflarestorage.com",
        "my-bucket",
        creds.clone(),
    );
    assert!(backend.is_ok());

    let backend = S3StorageBackend::new("http://127.0.0.1:9000", "my-bucket", creds.clone());
    assert!(backend.is_ok());
}

#[test]
fn test_invalid_endpoint() {
    let creds = S3Credentials {
        access_key: "test".to_string(),
        secret_key: "test".to_string(),
    };

    let backend = S3StorageBackend::new("ftp://s3.amazonaws.com", "my-bucket", creds.clone());
    assert!(matches!(backend, Err(S3Error::InvalidEndpoint)));

    let backend = S3StorageBackend::new("s3.amazonaws.com", "my-bucket", creds.clone());
    assert!(matches!(backend, Err(S3Error::InvalidEndpoint)));

    let backend = S3StorageBackend::new("https://s3 amazonaws com", "my-bucket", creds.clone());
    assert!(matches!(backend, Err(S3Error::InvalidEndpoint)));
}

#[test]
fn test_invalid_bucket() {
    let creds = S3Credentials {
        access_key: "test".to_string(),
        secret_key: "test".to_string(),
    };

    // Too short
    let backend = S3StorageBackend::new("https://s3.amazonaws.com", "my", creds.clone());
    assert!(matches!(backend, Err(S3Error::InvalidBucket)));

    // Invalid chars
    let backend = S3StorageBackend::new("https://s3.amazonaws.com", "my_bucket", creds.clone());
    assert!(matches!(backend, Err(S3Error::InvalidBucket)));

    let backend = S3StorageBackend::new("https://s3.amazonaws.com", "My-Bucket", creds.clone());
    assert!(matches!(backend, Err(S3Error::InvalidBucket)));
}

#[test]
fn test_chunk_calculation_less_than_5mb() {
    let total_size = 3 * 1024 * 1024; // 3MB
    let chunk_size = 5 * 1024 * 1024; // 5MB
    let chunks = S3StorageBackend::calculate_chunks(total_size, chunk_size);

    assert_eq!(chunks.len(), 1);
    assert_eq!(chunks[0], (0, 3 * 1024 * 1024));
}

#[test]
fn test_chunk_calculation_greater_than_5mb() {
    let total_size = 12 * 1024 * 1024; // 12MB
    let chunk_size = 5 * 1024 * 1024; // 5MB minimum
    let chunks = S3StorageBackend::calculate_chunks(total_size, chunk_size);

    assert_eq!(chunks.len(), 3);
    assert_eq!(chunks[0], (0, 5 * 1024 * 1024));
    assert_eq!(chunks[1], (5 * 1024 * 1024, 5 * 1024 * 1024));
    assert_eq!(chunks[2], (10 * 1024 * 1024, 2 * 1024 * 1024));
}

#[test]
fn test_chunk_calculation_exact_multiple() {
    let total_size = 10 * 1024 * 1024; // 10MB
    let chunk_size = 5 * 1024 * 1024; // 5MB minimum
    let chunks = S3StorageBackend::calculate_chunks(total_size, chunk_size);

    assert_eq!(chunks.len(), 2);
    assert_eq!(chunks[0], (0, 5 * 1024 * 1024));
    assert_eq!(chunks[1], (5 * 1024 * 1024, 5 * 1024 * 1024));
}

#[test]
fn test_chunk_calculation_zero_size() {
    let chunks = S3StorageBackend::calculate_chunks(0, 5 * 1024 * 1024);
    assert_eq!(chunks.len(), 1);
    assert_eq!(chunks[0], (0, 0));
}

#[test]
fn test_chunk_calculation_ignores_small_requested_chunk_size() {
    let total_size = 7 * 1024 * 1024; // 7MB
    let requested_chunk_size = 1024 * 1024; // 1MB - should be ignored and increased to 5MB
    let chunks = S3StorageBackend::calculate_chunks(total_size, requested_chunk_size);

    assert_eq!(chunks.len(), 2);
    assert_eq!(chunks[0], (0, 5 * 1024 * 1024));
    assert_eq!(chunks[1], (5 * 1024 * 1024, 2 * 1024 * 1024));
}
#[test]
fn test_sigv4_signing() {
    use backup_destinations::s3::sigv4::{SigV4Request, sign};
    use chrono::{TimeZone, Utc};
    use std::collections::BTreeMap;

    let datetime = Utc.with_ymd_and_hms(2023, 5, 24, 12, 0, 0).unwrap();

    let mut req = SigV4Request {
        method: "GET",
        endpoint: "https://s3.amazonaws.com/test-bucket/test-key?versionId=123",
        headers: BTreeMap::new(),
        body: b"",
        region: "us-east-1",
        access_key: "AKIAIOSFODNN7EXAMPLE",
        secret_key: "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY",
        datetime,
    };

    let headers = sign(&mut req);

    assert!(headers.contains_key("authorization"));
    let auth = headers.get("authorization").unwrap().to_str().unwrap();
    assert!(auth.starts_with("AWS4-HMAC-SHA256 Credential=AKIAIOSFODNN7EXAMPLE/20230524/us-east-1/s3/aws4_request,"));
    assert!(auth.contains("SignedHeaders=host;x-amz-content-sha256;x-amz-date"));
    assert!(auth.contains("Signature="));
}

#[test]
fn test_abort_on_failure_logic() {
    // Tests S3Error implementations
    let e = backup_destinations::s3::S3Error::UploadFailed;
    assert_eq!(e.to_string(), "Upload failed after retries");

    let e = backup_destinations::s3::S3Error::InvalidEndpoint;
    assert_eq!(e.to_string(), "Invalid endpoint URL");

    let e = backup_destinations::s3::S3Error::NetworkError("timeout".into());
    assert_eq!(e.to_string(), "Network error: timeout");
}
