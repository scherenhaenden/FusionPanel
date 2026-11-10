#![forbid(unsafe_code)]

use hosting::brotli_dynamic::{BrotliConfig, BrotliError};

#[test]
fn test_default_config_generation() {
    let config = BrotliConfig::default();
    let result = config.generate_nginx_config().unwrap();

    assert!(result.contains("brotli on;"));
    assert!(result.contains("brotli_comp_level 6;"));
    assert!(result.contains("brotli_min_length 20;"));
    assert!(result.contains("brotli_types text/plain text/css application/json application/javascript text/xml application/xml application/xml+rss text/javascript image/svg+xml;"));
    assert!(!result.contains("brotli_window"));
}

#[test]
fn test_custom_config_generation() {
    let config = BrotliConfig::new()
        .with_comp_level(9)
        .with_min_length(256)
        .with_window_size(512)
        .with_types(vec!["text/html".to_string(), "text/plain".to_string()]);

    let result = config.generate_nginx_config().unwrap();

    assert!(result.contains("brotli on;"));
    assert!(result.contains("brotli_comp_level 9;"));
    assert!(result.contains("brotli_min_length 256;"));
    assert!(result.contains("brotli_window 512;"));
    assert!(result.contains("brotli_types text/html text/plain;"));
}

#[test]
fn test_invalid_compression_level_low() {
    let config = BrotliConfig::new().with_comp_level(0);
    let result = config.generate_nginx_config();

    assert_eq!(result, Err(BrotliError::InvalidCompressionLevel(0)));
}

#[test]
fn test_invalid_compression_level_high() {
    let config = BrotliConfig::new().with_comp_level(12);
    let result = config.generate_nginx_config();

    assert_eq!(result, Err(BrotliError::InvalidCompressionLevel(12)));
}

#[test]
fn test_invalid_min_length() {
    let config = BrotliConfig::new().with_min_length(19);
    let result = config.generate_nginx_config();

    assert_eq!(result, Err(BrotliError::InvalidMinLength(19)));
}

#[test]
fn test_precompressed_type_image_jpeg() {
    let config = BrotliConfig::new().with_types(vec!["image/jpeg".to_string()]);
    let result = config.generate_nginx_config();

    assert_eq!(
        result,
        Err(BrotliError::PrecompressedTypeRequested("image/jpeg".to_string()))
    );
}

#[test]
fn test_precompressed_type_application_zip() {
    let config = BrotliConfig::new().with_types(vec!["application/zip".to_string()]);
    let result = config.generate_nginx_config();

    assert_eq!(
        result,
        Err(BrotliError::PrecompressedTypeRequested("application/zip".to_string()))
    );
}

#[test]
fn test_precompressed_type_video_mp4() {
    let config = BrotliConfig::new().with_types(vec!["video/mp4".to_string()]);
    let result = config.generate_nginx_config();

    assert_eq!(
        result,
        Err(BrotliError::PrecompressedTypeRequested("video/mp4".to_string()))
    );
}

#[test]
fn test_valid_image_svg() {
    let config = BrotliConfig::new().with_types(vec!["image/svg+xml".to_string()]);
    let result = config.generate_nginx_config();

    assert!(result.is_ok());
    assert!(result.unwrap().contains("brotli_types image/svg+xml;"));
}
