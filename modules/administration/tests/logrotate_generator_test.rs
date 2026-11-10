#![forbid(unsafe_code)]

use administration::logrotate_generator::{Frequency, LogrotateConfigBuilder, LogrotateError};

#[test]
fn test_nginx_logrotate() {
    let config = LogrotateConfigBuilder::new(vec!["/var/log/nginx/*.log".to_string()])
        .frequency(Frequency::Daily)
        .missingok()
        .rotate(14)
        .compress()
        .delaycompress()
        .notifempty()
        .create("0640", "www-data", "adm")
        .sharedscripts()
        .postrotate("[ -s /run/nginx.pid ] && kill -USR1 `cat /run/nginx.pid`")
        .build()
        .unwrap();

    let expected = "\
/var/log/nginx/*.log {
    daily
    rotate 14
    missingok
    notifempty
    compress
    delaycompress
    create 0640 www-data adm
    sharedscripts
    postrotate
        [ -s /run/nginx.pid ] && kill -USR1 `cat /run/nginx.pid`
    endscript
}
";

    assert_eq!(config, expected);
}

#[test]
fn test_postgresql_logrotate() {
    let config = LogrotateConfigBuilder::new(vec!["/var/log/postgresql/*.log".to_string()])
        .frequency(Frequency::Weekly)
        .missingok()
        .rotate(52)
        .compress()
        .delaycompress()
        .notifempty()
        .create("0640", "postgres", "postgres")
        .sharedscripts()
        .postrotate("/usr/bin/systemctl reload postgresql.service > /dev/null 2>&1 || true")
        .build()
        .unwrap();

    let expected = "\
/var/log/postgresql/*.log {
    weekly
    rotate 52
    missingok
    notifempty
    compress
    delaycompress
    create 0640 postgres postgres
    sharedscripts
    postrotate
        /usr/bin/systemctl reload postgresql.service > /dev/null 2>&1 || true
    endscript
}
";
    assert_eq!(config, expected);
}

#[test]
fn test_custom_app_logrotate() {
    let config = LogrotateConfigBuilder::new(vec!["/var/log/myapp/app.log".to_string()])
        .size("100M")
        .rotate(5)
        .missingok()
        .copytruncate()
        .build()
        .unwrap();

    let expected = "\
/var/log/myapp/app.log {
    rotate 5
    size 100M
    missingok
    copytruncate
}
";

    assert_eq!(config, expected);
}

#[test]
fn test_multiple_paths() {
    let config = LogrotateConfigBuilder::new(vec![
        "/var/log/app1.log".to_string(),
        "/var/log/app2.log".to_string(),
    ])
    .frequency(Frequency::Monthly)
    .rotate(12)
    .build()
    .unwrap();

    let expected = "\
/var/log/app1.log /var/log/app2.log {
    monthly
    rotate 12
}
";

    assert_eq!(config, expected);
}

#[test]
fn test_conflict_copytruncate_create() {
    let result = LogrotateConfigBuilder::new(vec!["/var/log/app.log".to_string()])
        .copytruncate()
        .create("0644", "root", "root")
        .build();

    assert!(matches!(result, Err(LogrotateError::ConflictCopytruncateCreate)));
}

#[test]
fn test_invalid_mode() {
    let result = LogrotateConfigBuilder::new(vec!["/var/log/app.log".to_string()])
        .create("0888", "root", "root") // 8 is not a valid octal digit
        .build();

    assert!(matches!(result, Err(LogrotateError::InvalidMode(mode)) if mode == "0888"));

    let result2 = LogrotateConfigBuilder::new(vec!["/var/log/app.log".to_string()])
        .create("0644a", "root", "root") // Too long and invalid character
        .build();

    assert!(matches!(result2, Err(LogrotateError::InvalidMode(_))));
}
