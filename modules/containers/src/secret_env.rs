#![forbid(unsafe_code)]

use std::collections::HashMap;
use std::fmt;

/// A wrapper around a secret string that prevents its value from being logged or debugged.
pub struct SecretEnv {
    value: String,
}

impl SecretEnv {
    pub fn new(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
        }
    }

    pub fn value(&self) -> &str {
        &self.value
    }
}

impl fmt::Debug for SecretEnv {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "\"[REDACTED]\"")
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum SecretEnvError {
    InvalidVariableName(String),
}

impl fmt::Display for SecretEnvError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidVariableName(name) => write!(f, "Invalid environment variable name: {}", name),
        }
    }
}

impl std::error::Error for SecretEnvError {}

fn is_valid_env_name(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    let mut chars = name.chars();
    let first = chars.next().unwrap();
    if !first.is_ascii_alphabetic() && first != '_' {
        return false;
    }
    for c in chars {
        if !c.is_ascii_alphanumeric() && c != '_' {
            return false;
        }
    }
    true
}

/// A vault for securely storing container environment secrets.
#[derive(Default)]
pub struct ContainerSecretVault {
    secrets: HashMap<String, SecretEnv>,
}

impl fmt::Debug for ContainerSecretVault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ContainerSecretVault")
            .field("secrets", &self.secrets)
            .finish()
    }
}

impl ContainerSecretVault {
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a secret to the vault. Validates that the variable name is POSIX compliant.
    pub fn add_secret(&mut self, name: &str, value: impl Into<String>) -> Result<(), SecretEnvError> {
        if !is_valid_env_name(name) {
            return Err(SecretEnvError::InvalidVariableName(name.to_string()));
        }

        self.secrets.insert(name.to_string(), SecretEnv::new(value));
        Ok(())
    }

    /// Generates a Docker envfile format representation of the secrets.
    pub fn generate_envfile(&self) -> String {
        let mut lines: Vec<String> = self.secrets
            .iter()
            .map(|(k, v)| format!("{}={}", k, v.value()))
            .collect();
        lines.sort(); // Sorting for deterministic output
        if lines.is_empty() {
            String::new()
        } else {
            lines.join("\n") + "\n"
        }
    }

    /// Returns the secrets as key-value tuples.
    pub fn as_tuples(&self) -> Vec<(String, String)> {
        let mut tuples: Vec<(String, String)> = self.secrets
            .iter()
            .map(|(k, v)| (k.clone(), v.value().to_string()))
            .collect();
        tuples.sort_by(|a, b| a.0.cmp(&b.0)); // Sorting for deterministic output
        tuples
    }
}
