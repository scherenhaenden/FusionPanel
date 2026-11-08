#![forbid(unsafe_code)]

use hosting::brotli_static::{BrotliStaticMatcher, CompressionType};
use std::fs;
use tempfile::tempdir;

#[test]
fn test_negotiate_encoding() {
    assert_eq!(
        BrotliStaticMatcher::negotiate_encoding("gzip, deflate, br"),
        CompressionType::Brotli
    );
    assert_eq!(
        BrotliStaticMatcher::negotiate_encoding("br, gzip, deflate"),
        CompressionType::Brotli
    );
    assert_eq!(
        BrotliStaticMatcher::negotiate_encoding("gzip, deflate"),
        CompressionType::Gzip
    );
    assert_eq!(
        BrotliStaticMatcher::negotiate_encoding("deflate"),
        CompressionType::None
    );
    assert_eq!(
        BrotliStaticMatcher::negotiate_encoding(""),
        CompressionType::None
    );
}

#[test]
fn test_sanitize_path() {
    assert!(BrotliStaticMatcher::sanitize_path("valid/path/file.txt").is_some());
    assert!(BrotliStaticMatcher::sanitize_path("valid/./path.txt").is_some());

    // Rejects path traversal
    assert!(BrotliStaticMatcher::sanitize_path("../etc/passwd").is_none());
    assert!(BrotliStaticMatcher::sanitize_path("valid/../../etc/passwd").is_none());

    // Rejects absolute path
    assert!(BrotliStaticMatcher::sanitize_path("/etc/passwd").is_none());

    // Rejects null bytes
    assert!(BrotliStaticMatcher::sanitize_path("valid/path\0/file.txt").is_none());
}

#[test]
fn test_find_sidecar_brotli_preferred() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    // Create requested file
    let requested = "style.css";
    let base_path = root.join(requested);
    fs::write(&base_path, "body { color: red; }").unwrap();

    // Create .br sidecar
    let br_path = BrotliStaticMatcher::append_ext(&base_path, "br");
    fs::write(&br_path, "brotli_content").unwrap();

    // Create .gz sidecar
    let gz_path = BrotliStaticMatcher::append_ext(&base_path, "gz");
    fs::write(&gz_path, "gzip_content").unwrap();

    // Test with Brotli preference
    let result = BrotliStaticMatcher::find_sidecar(root, requested, "gzip, br");
    assert!(result.is_some());
    let (comp_type, path) = result.unwrap();
    assert_eq!(comp_type, CompressionType::Brotli);
    assert_eq!(path, br_path);
}

#[test]
fn test_find_sidecar_gzip_fallback() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    // Create requested file
    let requested = "style.css";
    let base_path = root.join(requested);
    fs::write(&base_path, "body { color: red; }").unwrap();

    // Create only .gz sidecar
    let gz_path = BrotliStaticMatcher::append_ext(&base_path, "gz");
    fs::write(&gz_path, "gzip_content").unwrap();

    // Test with Brotli preference (should fallback to Gzip)
    let result = BrotliStaticMatcher::find_sidecar(root, requested, "gzip, br");
    assert!(result.is_some());
    let (comp_type, path) = result.unwrap();
    assert_eq!(comp_type, CompressionType::Gzip);
    assert_eq!(path, gz_path);
}

#[test]
fn test_find_sidecar_no_sidecar() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    let requested = "style.css";
    let base_path = root.join(requested);
    fs::write(&base_path, "body { color: red; }").unwrap();

    // Test without any sidecar
    let result = BrotliStaticMatcher::find_sidecar(root, requested, "gzip, br");
    assert!(result.is_none());
}

#[test]
fn test_find_sidecar_path_traversal() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    // Path traversal attempt should return None before checking files
    let result = BrotliStaticMatcher::find_sidecar(root, "../style.css", "gzip, br");
    assert!(result.is_none());
}

#[test]
fn test_generate_nginx_directives() {
    assert_eq!(
        BrotliStaticMatcher::generate_nginx_directives(CompressionType::Brotli),
        "brotli_static on;\n"
    );
    assert_eq!(
        BrotliStaticMatcher::generate_nginx_directives(CompressionType::Gzip),
        "gzip_static on;\n"
    );
    assert_eq!(
        BrotliStaticMatcher::generate_nginx_directives(CompressionType::None),
        ""
    );
}
