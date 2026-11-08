#![forbid(unsafe_code)]

use backup_destinations::checksum_verifier::{
    format_sha256_file, parse_sha256_file, ChecksumVerifier, VerificationResult,
};
use tempfile::NamedTempFile;
use sha2::{Sha256, Digest};
use std::io::Write;

#[test]
fn test_format_sha256_file() {
    let hash = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
    let filename = "archive.tar.gz";
    let formatted = format_sha256_file(hash, filename);
    assert_eq!(
        formatted,
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855  archive.tar.gz\n"
    );
}

#[test]
fn test_parse_sha256_file() {
    let contents = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855  archive.tar.gz\n";
    let parsed = parse_sha256_file(contents);
    assert_eq!(
        parsed,
        Some((
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855".to_string(),
            "archive.tar.gz".to_string()
        ))
    );

    // Test with single space and asterisk
    let contents_asterisk = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855 *archive.tar.gz\n";
    let parsed_asterisk = parse_sha256_file(contents_asterisk);
    assert_eq!(
        parsed_asterisk,
        Some((
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855".to_string(),
            "archive.tar.gz".to_string()
        ))
    );

    // Test invalid format
    let invalid = "invalid format";
    assert_eq!(parse_sha256_file(invalid), None);
}

#[tokio::test]
async fn test_verify_file_valid() {
    let data = b"hello world";
    let mut hasher = Sha256::new();
    hasher.update(data);
    let expected_hash = hex::encode(hasher.finalize());

    let mut temp_file = NamedTempFile::new().unwrap();
    temp_file.write_all(data).unwrap();
    temp_file.flush().unwrap();

    let result = ChecksumVerifier::verify_file(temp_file.path(), &expected_hash).await;
    assert_eq!(result, VerificationResult::Valid);
}

#[tokio::test]
async fn test_verify_file_mismatch() {
    let data = b"hello world";
    let corrupted_data = b"hello warld";

    let mut hasher = Sha256::new();
    hasher.update(data);
    let expected_hash = hex::encode(hasher.finalize());

    let mut temp_file = NamedTempFile::new().unwrap();
    temp_file.write_all(corrupted_data).unwrap();
    temp_file.flush().unwrap();

    let result = ChecksumVerifier::verify_file(temp_file.path(), &expected_hash).await;
    match result {
        VerificationResult::Mismatch { expected, actual } => {
            assert_eq!(expected, expected_hash);

            let mut corrupted_hasher = Sha256::new();
            corrupted_hasher.update(corrupted_data);
            assert_eq!(actual, hex::encode(corrupted_hasher.finalize()));
        }
        _ => panic!("Expected mismatch"),
    }
}

#[tokio::test]
async fn test_verify_file_missing() {
    let expected_hash = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
    let result = ChecksumVerifier::verify_file("/non/existent/path/to/archive", expected_hash).await;
    assert_eq!(result, VerificationResult::ReadError);
}
