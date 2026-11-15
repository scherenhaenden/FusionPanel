#![forbid(unsafe_code)]

use containers::secret_env::{ContainerSecretVault, SecretEnv, SecretEnvError};
use std::io::Write;
use tempfile::NamedTempFile;

#[test]
fn test_valid_variable_names() {
    let mut vault = ContainerSecretVault::new();
    assert!(vault.add_secret("VALID_NAME", "secret1").is_ok());
    assert!(vault.add_secret("valid_name", "secret2").is_ok());
    assert!(vault.add_secret("_VALID_NAME", "secret3").is_ok());
    assert!(vault.add_secret("VALID_NAME_1", "secret4").is_ok());
}

#[test]
fn test_invalid_variable_names() {
    let mut vault = ContainerSecretVault::new();
    assert_eq!(
        vault.add_secret("INVALID-NAME", "secret").unwrap_err(),
        SecretEnvError::InvalidVariableName("INVALID-NAME".to_string())
    );
    assert_eq!(
        vault.add_secret("1INVALID", "secret").unwrap_err(),
        SecretEnvError::InvalidVariableName("1INVALID".to_string())
    );
    assert_eq!(
        vault.add_secret("INVALID NAME", "secret").unwrap_err(),
        SecretEnvError::InvalidVariableName("INVALID NAME".to_string())
    );
    assert_eq!(
        vault.add_secret("INVALID=NAME", "secret").unwrap_err(),
        SecretEnvError::InvalidVariableName("INVALID=NAME".to_string())
    );
    assert_eq!(
        vault.add_secret("INVALID\nNAME", "secret").unwrap_err(),
        SecretEnvError::InvalidVariableName("INVALID\nNAME".to_string())
    );
    assert_eq!(
        vault.add_secret("INVALID\0NAME", "secret").unwrap_err(),
        SecretEnvError::InvalidVariableName("INVALID\0NAME".to_string())
    );
    assert_eq!(
        vault.add_secret("INVALID`NAME`", "secret").unwrap_err(),
        SecretEnvError::InvalidVariableName("INVALID`NAME`".to_string())
    );
    assert_eq!(
        vault.add_secret("INVALID;NAME", "secret").unwrap_err(),
        SecretEnvError::InvalidVariableName("INVALID;NAME".to_string())
    );
    assert_eq!(
        vault.add_secret("", "secret").unwrap_err(),
        SecretEnvError::InvalidVariableName("".to_string())
    );
}

#[test]
fn test_debug_redaction_secret_env() {
    let secret = SecretEnv::new("super_secret_value");
    let debug_output = format!("{:?}", secret);
    assert_eq!(debug_output, "\"[REDACTED]\"");
    assert!(!debug_output.contains("super_secret_value"));
}

#[test]
fn test_debug_redaction_vault() {
    let mut vault = ContainerSecretVault::new();
    vault.add_secret("DB_PASSWORD", "db_super_secret").unwrap();
    vault.add_secret("API_KEY", "api_super_secret").unwrap();

    let debug_output = format!("{:?}", vault);

    // The debug output should contain the keys
    assert!(debug_output.contains("DB_PASSWORD"));
    assert!(debug_output.contains("API_KEY"));

    // The debug output should NOT contain the values, but instead "[REDACTED]"
    assert!(debug_output.contains("\"[REDACTED]\""));
    assert!(!debug_output.contains("db_super_secret"));
    assert!(!debug_output.contains("api_super_secret"));
}

#[test]
fn test_generate_envfile() {
    let mut vault = ContainerSecretVault::new();
    vault.add_secret("A_KEY", "value_a").unwrap();
    vault.add_secret("B_KEY", "value_b").unwrap();

    let envfile = vault.generate_envfile();
    assert_eq!(envfile, "A_KEY=value_a\nB_KEY=value_b\n");

    let mut temp_file = NamedTempFile::new().unwrap();
    temp_file.write_all(envfile.as_bytes()).unwrap();

    let content = std::fs::read_to_string(temp_file.path()).unwrap();
    assert_eq!(content, "A_KEY=value_a\nB_KEY=value_b\n");
}

#[test]
fn test_as_tuples() {
    let mut vault = ContainerSecretVault::new();
    vault.add_secret("B_KEY", "value_b").unwrap();
    vault.add_secret("A_KEY", "value_a").unwrap();

    let tuples = vault.as_tuples();
    assert_eq!(
        tuples,
        vec![
            ("A_KEY".to_string(), "value_a".to_string()),
            ("B_KEY".to_string(), "value_b".to_string()),
        ]
    );
}
