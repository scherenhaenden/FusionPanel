#![forbid(unsafe_code)]

#[path = "../src/compression.rs"]
pub mod compression;
#[path = "../src/domain/mod.rs"]
pub mod domain;

use compression::{generate_compression_config, CompressionSettings};

#[test]
fn test_default_compression_settings() {
    let settings = CompressionSettings::default();
    let config = generate_compression_config(&settings).unwrap();

    assert!(config.contains("gzip on;"));
    assert!(config.contains("brotli on;"));
    assert!(config.contains("add_header Cache-Control \"public\";"));
    assert!(config.contains("etag on;"));
}

#[test]
fn test_disabled_compression_settings() {
    let settings = CompressionSettings {
        gzip_enabled: false,
        brotli_enabled: false,
        static_asset_caching_enabled: false,
        etag_enabled: false,
    };
    let config = generate_compression_config(&settings).unwrap();

    assert!(!config.contains("gzip on;"));
    assert!(!config.contains("brotli on;"));
    assert!(!config.contains("add_header Cache-Control \"public\";"));
    assert!(config.contains("etag off;"));
}

#[test]
fn test_mixed_compression_settings() {
    let settings = CompressionSettings {
        gzip_enabled: true,
        brotli_enabled: false,
        static_asset_caching_enabled: true,
        etag_enabled: false,
    };
    let config = generate_compression_config(&settings).unwrap();

    assert!(config.contains("gzip on;"));
    assert!(!config.contains("brotli on;"));
    assert!(config.contains("add_header Cache-Control \"public\";"));
    assert!(config.contains("etag off;"));
}
