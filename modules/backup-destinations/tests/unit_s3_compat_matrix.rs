#![forbid(unsafe_code)]

use backup_destinations::s3_compat_matrix::{
    S3CompatError, S3CompatRouter, S3Provider, UrlSigningScheme,
};

#[test]
fn test_aws_s3_constraints() {
    let router = S3CompatRouter::new(S3Provider::AwsS3);

    assert_eq!(router.provider(), S3Provider::AwsS3);

    // Chunk size
    assert!(router.validate_chunk_size(5 * 1024 * 1024).is_ok());
    assert_eq!(
        router.validate_chunk_size(5 * 1024 * 1024 - 1),
        Err(S3CompatError::ChunkSizeTooSmall { size: 5242879, min: 5242880 })
    );

    // Signing scheme
    assert!(router.validate_signing_scheme(UrlSigningScheme::SigV4).is_ok());
    assert!(router.validate_signing_scheme(UrlSigningScheme::SigV2).is_ok());

    // ACLs
    assert!(router.validate_acl_support().is_ok());

    // Storage classes
    assert!(router.validate_storage_class("STANDARD").is_ok());
    assert!(router.validate_storage_class("GLACIER").is_ok());
    assert_eq!(
        router.validate_storage_class("UNKNOWN"),
        Err(S3CompatError::UnsupportedStorageClass { class: "UNKNOWN".to_string() })
    );
}

#[test]
fn test_cloudflare_r2_constraints() {
    let router = S3CompatRouter::new(S3Provider::CloudflareR2);

    // Chunk size
    assert!(router.validate_chunk_size(5 * 1024 * 1024).is_ok());

    // Signing scheme
    assert!(router.validate_signing_scheme(UrlSigningScheme::SigV4).is_ok());
    assert_eq!(
        router.validate_signing_scheme(UrlSigningScheme::SigV2),
        Err(S3CompatError::UnsupportedSigningScheme { scheme: UrlSigningScheme::SigV2 })
    );

    // ACLs
    assert_eq!(router.validate_acl_support(), Err(S3CompatError::AclsNotSupported));

    // Storage classes
    assert!(router.validate_storage_class("STANDARD").is_ok());
    assert_eq!(
        router.validate_storage_class("GLACIER"),
        Err(S3CompatError::UnsupportedStorageClass { class: "GLACIER".to_string() })
    );
}

#[test]
fn test_google_cloud_storage_constraints() {
    let router = S3CompatRouter::new(S3Provider::GoogleCloudStorage);

    // Signing scheme
    assert!(router.validate_signing_scheme(UrlSigningScheme::SigV4).is_ok());
    assert_eq!(
        router.validate_signing_scheme(UrlSigningScheme::SigV2),
        Err(S3CompatError::UnsupportedSigningScheme { scheme: UrlSigningScheme::SigV2 })
    );

    // ACLs
    assert!(router.validate_acl_support().is_ok());

    // Storage classes
    assert!(router.validate_storage_class("NEARLINE").is_ok());
    assert!(router.validate_storage_class("COLDLINE").is_ok());
    assert_eq!(
        router.validate_storage_class("GLACIER"),
        Err(S3CompatError::UnsupportedStorageClass { class: "GLACIER".to_string() })
    );
}

#[test]
fn test_backblaze_b2_constraints() {
    let router = S3CompatRouter::new(S3Provider::BackblazeB2);

    // Signing scheme
    assert!(router.validate_signing_scheme(UrlSigningScheme::SigV4).is_ok());
    assert_eq!(
        router.validate_signing_scheme(UrlSigningScheme::SigV2),
        Err(S3CompatError::UnsupportedSigningScheme { scheme: UrlSigningScheme::SigV2 })
    );

    // ACLs
    assert_eq!(router.validate_acl_support(), Err(S3CompatError::AclsNotSupported));

    // Storage classes
    assert!(router.validate_storage_class("STANDARD").is_ok());
    assert_eq!(
        router.validate_storage_class("NEARLINE"),
        Err(S3CompatError::UnsupportedStorageClass { class: "NEARLINE".to_string() })
    );
}

#[test]
fn test_minio_constraints() {
    let router = S3CompatRouter::new(S3Provider::MinIo);

    // Signing scheme
    assert!(router.validate_signing_scheme(UrlSigningScheme::SigV4).is_ok());
    assert!(router.validate_signing_scheme(UrlSigningScheme::SigV2).is_ok());

    // ACLs
    assert!(router.validate_acl_support().is_ok());

    // Storage classes
    assert!(router.validate_storage_class("STANDARD").is_ok());
    assert!(router.validate_storage_class("REDUCED_REDUNDANCY").is_ok());
    assert_eq!(
        router.validate_storage_class("GLACIER"),
        Err(S3CompatError::UnsupportedStorageClass { class: "GLACIER".to_string() })
    );
}
