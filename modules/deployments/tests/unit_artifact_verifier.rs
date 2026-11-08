#![forbid(unsafe_code)]

use std::fs::File;
use std::io::Write;
use std::path::Path;
use sha2::{Sha256, Digest};
use tempfile::tempdir;
use deployments::artifact_verifier::ArtifactVerifier;

#[test]
fn test_verify_checksum() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("archive.zip");
    
    let content = b"test content for checksum";
    let mut file = File::create(&file_path).unwrap();
    file.write_all(content).unwrap();

    let mut hasher = Sha256::new();
    hasher.update(content);
    let expected_hex = hex::encode(hasher.finalize());

    let verifier = ArtifactVerifier::default();
    
    assert!(verifier.verify_checksum(&file_path, &expected_hex).is_ok());
    
    // Mismatch
    assert!(verifier.verify_checksum(&file_path, "0000000000000000000000000000000000000000000000000000000000000000").is_err());
}

#[test]
fn test_validate_path() {
    let verifier = ArtifactVerifier::default();

    // Valid paths
    assert!(verifier.validate_path(Path::new("normal/file.txt")).is_ok());
    assert!(verifier.validate_path(Path::new("another/valid/path")).is_ok());

    // Invalid paths
    assert!(verifier.validate_path(Path::new("../escape.txt")).is_err());
    assert!(verifier.validate_path(Path::new("valid/../../escape.txt")).is_err());
    assert!(verifier.validate_path(Path::new("/absolute/path")).is_err());
    assert!(verifier.validate_path(Path::new("file\0with\0null")).is_err());
}

#[test]
fn test_validate_symlink() {
    let verifier = ArtifactVerifier::default();
    let link_path = Path::new("link");

    // Valid symlinks
    assert!(verifier.validate_symlink(link_path, Path::new("target.txt")).is_ok());
    assert!(verifier.validate_symlink(link_path, Path::new("dir/target.txt")).is_ok());
    
    // Invalid symlinks escaping root
    assert!(verifier.validate_symlink(link_path, Path::new("../target.txt")).is_err());
    assert!(verifier.validate_symlink(link_path, Path::new("dir/../../target.txt")).is_err());
    assert!(verifier.validate_symlink(link_path, Path::new("/absolute/path")).is_err());
    assert!(verifier.validate_symlink(link_path, Path::new("target\0with\0null")).is_err());
    
    // Valid symlink inside a deep directory pointing back slightly
    let link_path2 = Path::new("deep/dir/link");
    assert!(verifier.validate_symlink(link_path2, Path::new("../target.txt")).is_ok());
    assert!(verifier.validate_symlink(link_path2, Path::new("../../target.txt")).is_ok());
    
    // Same link escaping out of root
    assert!(verifier.validate_symlink(link_path2, Path::new("../../../target.txt")).is_err());
}

#[test]
fn test_validate_expansion() {
    let verifier = ArtifactVerifier::new(1000, 10, 10.0);

    // Valid expansion
    assert!(verifier.validate_expansion(100, 500, 5).is_ok());

    // Max uncompressed bytes exceeded
    assert!(verifier.validate_expansion(100, 1001, 5).is_err());

    // Max file count exceeded
    assert!(verifier.validate_expansion(100, 500, 11).is_err());

    // Max ratio exceeded
    assert!(verifier.validate_expansion(10, 101, 5).is_err());
    
    // Edge cases: ratio exactly at max
    assert!(verifier.validate_expansion(10, 100, 5).is_ok());
    
    // No compressed size (e.g. uncompressed archive or zero byte input, avoids divide by zero)
    assert!(verifier.validate_expansion(0, 100, 5).is_ok());
    assert!(verifier.validate_expansion(0, 1001, 5).is_err());
}
