#![forbid(unsafe_code)]

use hosting::fastcgi_cache::{compute_md5, FastcgiCacheZoneManager};

#[test]
fn test_compute_md5() {
    let hash = compute_md5(b"httpGETexample.com/");
    // Pre-computed md5 for "httpGETexample.com/"
    assert_eq!(hash, "ddf158b292edfaa14788b3f19a50ebb7");

    let hash2 = compute_md5(b"test");
    // Pre-computed md5 for "test"
    assert_eq!(hash2, "098f6bcd4621d373cade4e832627b4f6");
}

#[test]
fn test_cache_key_generation() {
    let manager = FastcgiCacheZoneManager::new(
        "/var/run/nginx-cache",
        "MY_ZONE",
        "10m",
        "1g",
        "60m"
    );
    let key = manager.generate_cache_key("https", "GET", "example.com", "/api/data");
    assert_eq!(key, "httpsGETexample.com/api/data");
}

#[test]
fn test_cache_file_path_computation() {
    let manager = FastcgiCacheZoneManager::new(
        "/var/run/nginx-cache",
        "MY_ZONE",
        "10m",
        "1g",
        "60m"
    );

    let key = manager.generate_cache_key("http", "GET", "example.com", "/");
    let path = manager.compute_cache_file_path(&key);

    // MD5 for "httpGETexample.com/" is "ddf158b292edfaa14788b3f19a50ebb7"
    // levels=1:2 means path is <base_path>/7/bb/ddf158b292edfaa14788b3f19a50ebb7
    assert_eq!(path, "/var/run/nginx-cache/7/bb/ddf158b292edfaa14788b3f19a50ebb7");
}

#[test]
fn test_generate_path_directive() {
    let manager = FastcgiCacheZoneManager::new(
        "/var/run/nginx-cache",
        "TEST_ZONE",
        "20m",
        "2g",
        "30m"
    );

    let directive = manager.generate_path_directive();
    assert_eq!(
        directive,
        "fastcgi_cache_path /var/run/nginx-cache levels=1:2 keys_zone=TEST_ZONE:20m max_size=2g inactive=30m use_temp_path=off;"
    );
}

#[test]
fn test_generate_bypass_rules() {
    let manager = FastcgiCacheZoneManager::new(
        "/var/run/nginx-cache",
        "TEST_ZONE",
        "10m",
        "1g",
        "60m"
    );

    let rules = manager.generate_bypass_rules();
    assert!(rules.contains("fastcgi_cache_bypass $cookie_PHPSESSID $cookie_wp_logged_in_ $arg_nocache;"));
    assert!(rules.contains("fastcgi_no_cache $cookie_PHPSESSID $cookie_wp_logged_in_ $arg_nocache;"));
}
