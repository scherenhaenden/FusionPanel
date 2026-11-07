#![forbid(unsafe_code)]

#[path = "../src/php/pool.rs"]
mod pool;

#[test]
fn test_generate_pool() {
    let config = pool::generate_pool(
        "example_com",
        "user123",
        "group123",
        "/var/www/example.com",
        10,
        2,
    );

    assert!(config.contains("[example_com]"));
    assert!(config.contains("user = user123"));
    assert!(config.contains("group = group123"));
    assert!(config.contains("listen = /run/php/php-fpm-example_com.sock"));
    assert!(config.contains("listen.owner = user123"));
    assert!(config.contains("listen.group = group123"));
    assert!(config.contains("listen.mode = 0660"));
    assert!(config.contains("pm = dynamic"));
    assert!(config.contains("pm.max_children = 10"));
    assert!(config.contains("pm.start_servers = 2"));
    assert!(config.contains("php_admin_value[open_basedir] = /var/www/example.com:/tmp"));
    assert!(config.contains("php_admin_flag[disable_functions] = exec,passthru,shell_exec,system"));
}
