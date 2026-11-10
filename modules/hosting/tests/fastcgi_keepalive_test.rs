#![forbid(unsafe_code)]

use hosting::fastcgi_keepalive::{FastcgiKeepaliveTuner, TunerError};

#[test]
fn test_zero_concurrency() {
    let result = FastcgiKeepaliveTuner::compute_nginx(0, 4, 100);
    assert_eq!(result.unwrap_err(), TunerError::ZeroConcurrency);
}

#[test]
fn test_invalid_core_count() {
    let result = FastcgiKeepaliveTuner::compute_nginx(10, 0, 100);
    assert_eq!(result.unwrap_err(), TunerError::InvalidCoreCount);
}

#[test]
fn test_small_pool() {
    let config = FastcgiKeepaliveTuner::compute_nginx(10, 2, 50).unwrap();
    assert_eq!(config.keepalive, 5);
    assert_eq!(config.fastcgi_buffer_size, "4k");
    assert_eq!(config.fastcgi_buffers, "8 4k");
    assert_eq!(config.fastcgi_busy_buffers_size, "8k");
    assert_eq!(config.timeout_clamp, 30);
}

#[test]
fn test_medium_pool() {
    let config = FastcgiKeepaliveTuner::compute_nginx(50, 4, 600).unwrap();
    assert_eq!(config.keepalive, 50);
    assert_eq!(config.fastcgi_buffer_size, "8k");
    assert_eq!(config.fastcgi_buffers, "16 8k");
    assert_eq!(config.fastcgi_busy_buffers_size, "16k");
    assert_eq!(config.timeout_clamp, 10);
}

#[test]
fn test_high_concurrency_pool() {
    let config = FastcgiKeepaliveTuner::compute_nginx(200, 16, 2000).unwrap();
    assert_eq!(config.keepalive, 200);
    assert_eq!(config.fastcgi_buffer_size, "16k");
    assert_eq!(config.fastcgi_buffers, "16 16k");
    assert_eq!(config.fastcgi_busy_buffers_size, "32k");
    assert_eq!(config.timeout_clamp, 30);
}

#[test]
fn test_buffer_alignment_and_ceiling() {
    let config = FastcgiKeepaliveTuner::compute_nginx(1000, 32, 10000).unwrap();
    assert_eq!(config.fastcgi_buffer_size, "32k");
    assert_eq!(config.fastcgi_buffers, "32 32k");
    assert_eq!(config.fastcgi_busy_buffers_size, "64k");
    assert_eq!(config.timeout_clamp, 30);
}

#[test]
fn test_socket_exhaustion_clamp() {
    let config = FastcgiKeepaliveTuner::compute_nginx(20, 2, 300).unwrap();
    assert_eq!(config.timeout_clamp, 10);

    let config2 = FastcgiKeepaliveTuner::compute_nginx(50, 2, 150).unwrap();
    assert_eq!(config2.timeout_clamp, 30);

    let config3 = FastcgiKeepaliveTuner::compute_nginx(100, 2, 50).unwrap();
    assert_eq!(config3.timeout_clamp, 60);
}

#[test]
fn test_nginx_upstream_snippet() {
    let config = FastcgiKeepaliveTuner::compute_nginx(20, 2, 100).unwrap();
    let snippet = FastcgiKeepaliveTuner::emit_nginx_upstream_snippet("php_fpm_pool", &config);
    assert!(snippet.contains("upstream php_fpm_pool {"));
    assert!(snippet.contains("keepalive 10;"));
}

#[test]
fn test_nginx_location_snippet() {
    let config = FastcgiKeepaliveTuner::compute_nginx(20, 2, 100).unwrap();
    let snippet = FastcgiKeepaliveTuner::emit_nginx_location_snippet(&config);
    assert!(snippet.contains("fastcgi_keep_conn on;"));
    assert!(snippet.contains("fastcgi_buffer_size 4k;"));
    assert!(snippet.contains("fastcgi_buffers 8 4k;"));
    assert!(snippet.contains("fastcgi_busy_buffers_size 8k;"));
    assert!(snippet.contains("fastcgi_read_timeout 30s;"));
}

#[test]
fn test_apache_snippet() {
    let config = FastcgiKeepaliveTuner::compute_nginx(20, 2, 100).unwrap();
    let snippet = FastcgiKeepaliveTuner::emit_apache_snippet(&config);
    assert!(snippet.contains("enablereuse=on max=10>"));
    assert!(snippet.contains("ProxySet timeout=30"));
}
