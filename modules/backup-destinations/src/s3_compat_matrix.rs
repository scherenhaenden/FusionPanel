#![forbid(unsafe_code)]

use std::collections::HashSet;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum S3Provider {
    AwsS3,
    CloudflareR2,
    GoogleCloudStorage,
    BackblazeB2,
    MinIo,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UrlSigningScheme {
    SigV2,
    SigV4,
}

#[derive(Debug, Clone)]
pub struct ProviderConstraints {
    pub min_multipart_chunk_size: u64,
    pub supported_signing_schemes: HashSet<UrlSigningScheme>,
    pub supports_acls: bool,
    pub storage_classes: HashSet<String>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum S3CompatError {
    #[error("Chunk size {size} is less than provider minimum {min}")]
    ChunkSizeTooSmall { size: u64, min: u64 },
    #[error("Signing scheme {scheme:?} is not supported by provider")]
    UnsupportedSigningScheme { scheme: UrlSigningScheme },
    #[error("ACLs are not supported by this provider")]
    AclsNotSupported,
    #[error("Storage class {class} is not supported by this provider")]
    UnsupportedStorageClass { class: String },
}

pub struct S3CompatRouter {
    provider: S3Provider,
    constraints: ProviderConstraints,
}

impl S3CompatRouter {
    pub fn new(provider: S3Provider) -> Self {
        let constraints = match provider {
            S3Provider::AwsS3 => ProviderConstraints {
                min_multipart_chunk_size: 5 * 1024 * 1024,
                supported_signing_schemes: HashSet::from([UrlSigningScheme::SigV2, UrlSigningScheme::SigV4]),
                supports_acls: true,
                storage_classes: HashSet::from([
                    "STANDARD".to_string(),
                    "STANDARD_IA".to_string(),
                    "INTELLIGENT_TIERING".to_string(),
                    "ONEZONE_IA".to_string(),
                    "GLACIER".to_string(),
                    "DEEP_ARCHIVE".to_string(),
                    "OUTPOSTS".to_string(),
                    "GLACIER_IR".to_string(),
                ]),
            },
            S3Provider::CloudflareR2 => ProviderConstraints {
                min_multipart_chunk_size: 5 * 1024 * 1024,
                supported_signing_schemes: HashSet::from([UrlSigningScheme::SigV4]),
                supports_acls: false,
                storage_classes: HashSet::from([
                    "STANDARD".to_string(),
                ]),
            },
            S3Provider::GoogleCloudStorage => ProviderConstraints {
                min_multipart_chunk_size: 5 * 1024 * 1024,
                supported_signing_schemes: HashSet::from([UrlSigningScheme::SigV4]),
                supports_acls: true,
                storage_classes: HashSet::from([
                    "STANDARD".to_string(),
                    "NEARLINE".to_string(),
                    "COLDLINE".to_string(),
                    "ARCHIVE".to_string(),
                ]),
            },
            S3Provider::BackblazeB2 => ProviderConstraints {
                min_multipart_chunk_size: 5 * 1024 * 1024,
                supported_signing_schemes: HashSet::from([UrlSigningScheme::SigV4]),
                supports_acls: false,
                storage_classes: HashSet::from([
                    "STANDARD".to_string(),
                ]),
            },
            S3Provider::MinIo => ProviderConstraints {
                min_multipart_chunk_size: 5 * 1024 * 1024,
                supported_signing_schemes: HashSet::from([UrlSigningScheme::SigV4, UrlSigningScheme::SigV2]),
                supports_acls: true,
                storage_classes: HashSet::from([
                    "STANDARD".to_string(),
                    "REDUCED_REDUNDANCY".to_string(),
                ]),
            },
        };
        Self {
            provider,
            constraints,
        }
    }

    pub fn provider(&self) -> S3Provider {
        self.provider
    }

    pub fn constraints(&self) -> &ProviderConstraints {
        &self.constraints
    }

    pub fn validate_chunk_size(&self, size: u64) -> Result<(), S3CompatError> {
        if size < self.constraints.min_multipart_chunk_size {
            return Err(S3CompatError::ChunkSizeTooSmall {
                size,
                min: self.constraints.min_multipart_chunk_size,
            });
        }
        Ok(())
    }

    pub fn validate_signing_scheme(&self, scheme: UrlSigningScheme) -> Result<(), S3CompatError> {
        if !self.constraints.supported_signing_schemes.contains(&scheme) {
            return Err(S3CompatError::UnsupportedSigningScheme { scheme });
        }
        Ok(())
    }

    pub fn validate_acl_support(&self) -> Result<(), S3CompatError> {
        if !self.constraints.supports_acls {
            return Err(S3CompatError::AclsNotSupported);
        }
        Ok(())
    }

    pub fn validate_storage_class(&self, class: &str) -> Result<(), S3CompatError> {
        if !self.constraints.storage_classes.contains(class) {
            return Err(S3CompatError::UnsupportedStorageClass {
                class: class.to_string(),
            });
        }
        Ok(())
    }
}
