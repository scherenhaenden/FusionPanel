#![forbid(unsafe_code)]

use administration::systemd_profile::{SystemdProfile, SystemdProfileError};

#[test]
fn test_systemd_profile_generation() {
    let mut profile = SystemdProfile::new("my-service", "My Awesome Service", "/usr/bin/my-service --start");
    profile = profile.with_user("appuser").with_group("appgroup");
    profile.add_env("APP_ENV", "production").unwrap();
    profile.add_env("APP_PORT", "8080").unwrap();

    let generated = profile.generate();

    assert!(generated.contains("[Unit]"));
    assert!(generated.contains("Description=My Awesome Service"));
    assert!(generated.contains("[Service]"));
    assert!(generated.contains("ExecStart=/usr/bin/my-service --start"));
    assert!(generated.contains("User=appuser"));
    assert!(generated.contains("Group=appgroup"));
    assert!(generated.contains("Environment=\"APP_ENV=production\""));
    assert!(generated.contains("Environment=\"APP_PORT=8080\""));

    // Sandbox Hardening Profile Checks
    assert!(generated.contains("ProtectSystem=strict"));
    assert!(generated.contains("ProtectHome=true"));
    assert!(generated.contains("PrivateTmp=true"));
    assert!(generated.contains("NoNewPrivileges=true"));
    assert!(generated.contains("RestrictNamespaces=true"));

    assert!(generated.contains("[Install]"));
    assert!(generated.contains("WantedBy=multi-user.target"));
}

#[test]
fn test_systemd_profile_default_wanted_by() {
    let profile = SystemdProfile::new("minimal", "Minimal", "/bin/true");
    let generated = profile.generate();

    assert!(generated.contains("WantedBy=multi-user.target"));
    assert!(!generated.contains("User="));
    assert!(!generated.contains("Group="));
}

#[test]
fn test_systemd_profile_custom_wanted_by() {
    let profile = SystemdProfile::new("custom", "Custom WantedBy", "/bin/true")
        .with_wanted_by("network-online.target");
    let generated = profile.generate();

    assert!(generated.contains("WantedBy=network-online.target"));
}

#[test]
fn test_invalid_env_key() {
    let mut profile = SystemdProfile::new("test", "test", "/bin/true");

    // Test empty key
    let err = profile.add_env("", "value").unwrap_err();
    assert_eq!(err, SystemdProfileError::InvalidEnvKey("".to_string()));

    // Test key starting with number
    let err = profile.add_env("1_INVALID", "value").unwrap_err();
    assert_eq!(err, SystemdProfileError::InvalidEnvKey("1_INVALID".to_string()));

    // Test key with special chars
    let err = profile.add_env("INV@LID", "value").unwrap_err();
    assert_eq!(err, SystemdProfileError::InvalidEnvKey("INV@LID".to_string()));
}

#[test]
fn test_invalid_env_value() {
    let mut profile = SystemdProfile::new("test", "test", "/bin/true");

    // Test newline
    let err = profile.add_env("VALID_KEY", "value\nwith\nnewline").unwrap_err();
    assert_eq!(err, SystemdProfileError::InvalidEnvValue("value\nwith\nnewline".to_string()));

    // Test null byte
    let err = profile.add_env("VALID_KEY", "value\0with\0null").unwrap_err();
    assert_eq!(err, SystemdProfileError::InvalidEnvValue("value\0with\0null".to_string()));

    // Test backtick
    let err = profile.add_env("VALID_KEY", "value`with`backtick").unwrap_err();
    assert_eq!(err, SystemdProfileError::InvalidEnvValue("value`with`backtick".to_string()));

    // Test equals sign
    let err = profile.add_env("VALID_KEY", "value=with=equals").unwrap_err();
    assert_eq!(err, SystemdProfileError::InvalidEnvValue("value=with=equals".to_string()));

    // Test semicolon
    let err = profile.add_env("VALID_KEY", "value;with;semicolon").unwrap_err();
    assert_eq!(err, SystemdProfileError::InvalidEnvValue("value;with;semicolon".to_string()));
}
