#![forbid(unsafe_code)]

use hosting::brotli_level_tuner::{BrotliCompressionTuner, TunerError};

#[test]
fn test_content_too_short() {
    let tuner = BrotliCompressionTuner::new(1024);
    let result = tuner.recommend_level("text/html", 500);
    assert_eq!(
        result,
        Err(TunerError::ContentTooShort {
            length: 500,
            threshold: 1024
        })
    );
}

#[test]
fn test_excluded_mime_type() {
    let tuner = BrotliCompressionTuner::default(); // default minimum length 20
    let result = tuner.recommend_level("application/zip", 1024);
    assert_eq!(
        result,
        Err(TunerError::ExcludedMimeType("application/zip".to_string()))
    );

    let result = tuner.recommend_level("image/jpeg", 1024);
    assert_eq!(
        result,
        Err(TunerError::ExcludedMimeType("image/jpeg".to_string()))
    );
}

#[test]
fn test_recommend_dynamic() {
    let tuner = BrotliCompressionTuner::default();

    let result = tuner.recommend_level("text/html", 1024);
    assert_eq!(result, Ok(4));

    let result = tuner.recommend_level("application/json; charset=utf-8", 1024);
    assert_eq!(result, Ok(4));
}

#[test]
fn test_recommend_static() {
    let tuner = BrotliCompressionTuner::default();

    let result = tuner.recommend_level("text/css", 1024);
    assert_eq!(result, Ok(6));

    let result = tuner.recommend_level("application/javascript", 1024);
    assert_eq!(result, Ok(6));
}

#[test]
fn test_generate_nginx_config_valid() {
    let tuner = BrotliCompressionTuner::default();
    let config = tuner.generate_nginx_config(5).unwrap();

    assert!(config.contains("brotli on;"));
    assert!(config.contains("brotli_comp_level 5;"));
    assert!(config.contains("brotli_min_length 20;"));
    assert!(config.contains("brotli_types text/plain text/css application/json application/javascript text/xml application/xml application/xml+rss text/javascript image/svg+xml;"));
}

#[test]
fn test_generate_nginx_config_invalid_level() {
    let tuner = BrotliCompressionTuner::default();
    let result = tuner.generate_nginx_config(0);
    assert_eq!(result, Err(TunerError::InvalidCompressionLevel(0)));

    let result = tuner.generate_nginx_config(12);
    assert_eq!(result, Err(TunerError::InvalidCompressionLevel(12)));
}

#[test]
fn test_generate_apache_config_valid() {
    let tuner = BrotliCompressionTuner::default();
    let config = tuner.generate_apache_config(5).unwrap();

    assert!(config.contains("<IfModule mod_brotli.c>"));
    assert!(config.contains("BrotliCompressionQuality 5"));
}
