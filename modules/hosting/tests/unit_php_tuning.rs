#![forbid(unsafe_code)]

use hosting::php_tuning::PhpTuning;

#[test]
fn test_default_php_tuning() {
    let tuning = PhpTuning::default();

    assert_eq!(tuning.opcache_memory_consumption, 128);
    assert_eq!(tuning.revalidate_freq, 2);
    assert_eq!(tuning.request_slowlog_timeout, "5s");
    assert_eq!(tuning.trace_logging_depth, 20);

    let ini_fragment = tuning.generate_php_ini_fragment();
    assert_eq!(
        ini_fragment,
        "opcache.memory_consumption=128\n\
         opcache.revalidate_freq=2\n"
    );

    let pool_fragment = tuning.generate_pool_fragment("example.com");
    assert_eq!(
        pool_fragment,
        "request_slowlog_timeout = 5s\n\
         request_slowlog_trace_depth = 20\n\
         slowlog = /var/log/fusionpanel/example.com-slow.log\n"
    );
}

#[test]
fn test_custom_php_tuning() {
    let tuning = PhpTuning::new(256, 0, "10s", 50);

    assert_eq!(tuning.opcache_memory_consumption, 256);
    assert_eq!(tuning.revalidate_freq, 0);
    assert_eq!(tuning.request_slowlog_timeout, "10s");
    assert_eq!(tuning.trace_logging_depth, 50);

    let ini_fragment = tuning.generate_php_ini_fragment();
    assert_eq!(
        ini_fragment,
        "opcache.memory_consumption=256\n\
         opcache.revalidate_freq=0\n"
    );

    let pool_fragment = tuning.generate_pool_fragment("example.com");
    assert_eq!(
        pool_fragment,
        "request_slowlog_timeout = 10s\n\
         request_slowlog_trace_depth = 50\n\
         slowlog = /var/log/fusionpanel/example.com-slow.log\n"
    );
}
