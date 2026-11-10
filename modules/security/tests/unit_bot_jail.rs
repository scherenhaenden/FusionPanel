#![forbid(unsafe_code)]

use security::bot_jail::NginxBadBotJail;

#[test]
fn test_default_bot_jail() {
    let jail = NginxBadBotJail::default();

    // Check default values
    assert_eq!(jail.name, "nginx-badbots");
    assert_eq!(jail.maxretry, 2);
    assert_eq!(jail.findtime, 600);
    assert_eq!(jail.bantime, 86400);

    assert_eq!(jail.log_paths.len(), 2);
    assert_eq!(jail.log_paths[0], "/var/log/nginx/access.log");
    assert_eq!(jail.log_paths[1], "/var/log/nginx/*access.log");

    assert!(jail.bad_bots.contains(&"Aboundex".to_string()));
    assert!(jail.bad_bots.contains(&"Zeus".to_string()));
}

#[test]
fn test_generate_jail_config() {
    let mut jail = NginxBadBotJail::default();
    jail.name = "custom-badbots".to_string();
    jail.maxretry = 5;
    jail.findtime = 300;
    jail.bantime = 3600;
    jail.log_paths = vec![
        "/opt/nginx/logs/access.log".to_string()
    ];

    let config = jail.generate_jail_config();

    assert!(config.contains("[custom-badbots]"));
    assert!(config.contains("enabled = true"));
    assert!(config.contains("port = http,https"));
    assert!(config.contains("filter = custom-badbots"));
    assert!(config.contains("logpath = /opt/nginx/logs/access.log"));
    assert!(config.contains("maxretry = 5"));
    assert!(config.contains("findtime = 300"));
    assert!(config.contains("bantime = 3600"));
    // Ensure default log paths aren't there
    assert!(!config.contains("logpath = /var/log/nginx/access.log"));
}

#[test]
fn test_generate_filter_config() {
    let mut jail = NginxBadBotJail::default();
    jail.bad_bots = vec![
        "EvilBot".to_string(),
        "NaughtySpider".to_string(),
    ];

    let config = jail.generate_filter_config();

    assert!(config.contains("[Definition]"));
    assert!(config.contains("ignoreregex = "));

    let expected_failregex = "failregex = ^<HOST> -.*\"(GET|POST|HEAD).*HTTP.*\" .*\"(?:EvilBot|NaughtySpider)\".*$";
    assert!(config.contains(expected_failregex));
}

#[test]
fn test_multiple_log_paths() {
    let mut jail = NginxBadBotJail::default();
    jail.log_paths = vec![
        "/var/log/nginx/access.log".to_string(),
        "/var/log/nginx/error.log".to_string(),
        "/var/log/nginx/custom.log".to_string(),
    ];

    let config = jail.generate_jail_config();

    assert!(config.contains("logpath = /var/log/nginx/access.log /var/log/nginx/error.log /var/log/nginx/custom.log"));

    let logpath_count = config.lines().filter(|line| line.starts_with("logpath = ")).count();
    assert_eq!(logpath_count, 1);
}
