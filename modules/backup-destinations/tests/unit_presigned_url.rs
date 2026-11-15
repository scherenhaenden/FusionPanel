#![forbid(unsafe_code)]

use backup_destinations::presigned_url::{PresignedUrlGenerator, PresignedUrlError};
use chrono::{TimeZone, Utc};
use std::time::Duration;

#[test]
fn test_generate_presigned_url_valid() {
    let generator = PresignedUrlGenerator::new(
        "AKIAIOSFODNN7EXAMPLE",
        "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY",
        "us-east-1",
        "https://s3.amazonaws.com",
    );

    let now = Utc.with_ymd_and_hms(2013, 5, 24, 0, 0, 0).unwrap();
    let url_str = generator
        .generate("GET", "examplebucket", "test.txt", Duration::from_secs(86400), now)
        .expect("Failed to generate URL");

    assert!(url_str.starts_with("https://s3.amazonaws.com/examplebucket/test.txt?"));
    assert!(url_str.contains("X-Amz-Algorithm=AWS4-HMAC-SHA256"));
    assert!(url_str.contains("X-Amz-Credential=AKIAIOSFODNN7EXAMPLE%2F20130524%2Fus-east-1%2Fs3%2Faws4_request"));
    assert!(url_str.contains("X-Amz-Date=20130524T000000Z"));
    assert!(url_str.contains("X-Amz-Expires=86400"));
    assert!(url_str.contains("X-Amz-SignedHeaders=host"));
    assert!(url_str.contains("X-Amz-Signature="));
}

#[test]
fn test_generate_presigned_url_invalid_expiration_zero() {
    let generator = PresignedUrlGenerator::new("access", "secret", "us-east-1", "https://s3.amazonaws.com");
    let now = Utc::now();
    let result = generator.generate("GET", "bucket", "key", Duration::from_secs(0), now);
    assert_eq!(result, Err(PresignedUrlError::InvalidExpiration));
}

#[test]
fn test_generate_presigned_url_invalid_expiration_too_long() {
    let generator = PresignedUrlGenerator::new("access", "secret", "us-east-1", "https://s3.amazonaws.com");
    let now = Utc::now();
    let result = generator.generate("GET", "bucket", "key", Duration::from_secs(604801), now);
    assert_eq!(result, Err(PresignedUrlError::InvalidExpiration));
}

#[test]
fn test_generate_presigned_url_valid_expiration_max() {
    let generator = PresignedUrlGenerator::new("access", "secret", "us-east-1", "https://s3.amazonaws.com");
    let now = Utc::now();
    let result = generator.generate("GET", "bucket", "key", Duration::from_secs(604800), now);
    assert!(result.is_ok());
}

#[test]
fn test_generate_presigned_url_valid_expiration_min() {
    let generator = PresignedUrlGenerator::new("access", "secret", "us-east-1", "https://s3.amazonaws.com");
    let now = Utc::now();
    let result = generator.generate("GET", "bucket", "key", Duration::from_secs(1), now);
    assert!(result.is_ok());
}

#[test]
fn test_generate_presigned_url_endpoint_with_trailing_slash() {
    let generator = PresignedUrlGenerator::new(
        "AKIAIOSFODNN7EXAMPLE",
        "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY",
        "us-east-1",
        "https://s3.amazonaws.com/",
    );

    let now = Utc.with_ymd_and_hms(2013, 5, 24, 0, 0, 0).unwrap();
    let url_str = generator
        .generate("GET", "examplebucket", "test.txt", Duration::from_secs(3600), now)
        .unwrap();

    assert!(url_str.starts_with("https://s3.amazonaws.com/examplebucket/test.txt?"));
}

#[test]
fn test_generate_presigned_url_key_with_slashes() {
    let generator = PresignedUrlGenerator::new(
        "AKIAIOSFODNN7EXAMPLE",
        "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY",
        "us-east-1",
        "https://s3.amazonaws.com",
    );

    let now = Utc.with_ymd_and_hms(2013, 5, 24, 0, 0, 0).unwrap();
    let url_str = generator
        .generate("GET", "examplebucket", "path/to/my/test.txt", Duration::from_secs(3600), now)
        .unwrap();

    assert!(url_str.starts_with("https://s3.amazonaws.com/examplebucket/path/to/my/test.txt?"));
}
