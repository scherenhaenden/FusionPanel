#![forbid(unsafe_code)]

use std::collections::HashMap;
use thiserror::Error;

#[derive(Error, Debug, PartialEq, Eq)]
pub enum SystemdProfileError {
    #[error("Invalid environment variable key: {0}")]
    InvalidEnvKey(String),
    #[error("Invalid environment variable value: {0}")]
    InvalidEnvValue(String),
}

#[derive(Debug, Clone)]
pub struct SystemdProfile {
    pub name: String,
    pub description: String,
    pub exec_start: String,
    pub user: Option<String>,
    pub group: Option<String>,
    pub environment: HashMap<String, String>,
    pub wanted_by: String,
}

impl SystemdProfile {
    pub fn new(name: impl Into<String>, description: impl Into<String>, exec_start: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            exec_start: exec_start.into(),
            user: None,
            group: None,
            environment: HashMap::new(),
            wanted_by: "multi-user.target".to_string(),
        }
    }

    pub fn with_user(mut self, user: impl Into<String>) -> Self {
        self.user = Some(user.into());
        self
    }

    pub fn with_group(mut self, group: impl Into<String>) -> Self {
        self.group = Some(group.into());
        self
    }

    pub fn with_wanted_by(mut self, wanted_by: impl Into<String>) -> Self {
        self.wanted_by = wanted_by.into();
        self
    }

    pub fn add_env(&mut self, key: impl Into<String>, value: impl Into<String>) -> Result<&mut Self, SystemdProfileError> {
        let key = key.into();
        let value = value.into();

        if !Self::is_valid_env_key(&key) {
            return Err(SystemdProfileError::InvalidEnvKey(key));
        }

        if !Self::is_valid_env_value(&value) {
            return Err(SystemdProfileError::InvalidEnvValue(value));
        }

        self.environment.insert(key, value);
        Ok(self)
    }

    fn is_valid_env_key(key: &str) -> bool {
        if key.is_empty() {
            return false;
        }
        let mut chars = key.chars();
        let first = chars.next().unwrap();
        if !(first.is_ascii_alphabetic() || first == '_') {
            return false;
        }
        for c in chars {
            if !(c.is_ascii_alphanumeric() || c == '_') {
                return false;
            }
        }
        true
    }

    fn is_valid_env_value(value: &str) -> bool {
        let invalid = ['\n', '\r', '\0', '`', '=', ';'];
        !value.contains(&invalid[..])
    }

    pub fn generate(&self) -> String {
        let mut out = String::new();
        out.push_str("[Unit]\n");
        out.push_str(&format!("Description={}\n", self.description));
        out.push_str("\n");

        out.push_str("[Service]\n");
        out.push_str(&format!("ExecStart={}\n", self.exec_start));
        if let Some(user) = &self.user {
            out.push_str(&format!("User={}\n", user));
        }
        if let Some(group) = &self.group {
            out.push_str(&format!("Group={}\n", group));
        }

        let mut sorted_env: Vec<_> = self.environment.iter().collect();
        sorted_env.sort_by_key(|(k, _)| *k);
        for (k, v) in sorted_env {
            out.push_str(&format!("Environment=\"{}={}\"\n", k, v));
        }

        out.push_str("\n");
        out.push_str("# Sandbox Hardening Profile\n");
        out.push_str("ProtectSystem=strict\n");
        out.push_str("ProtectHome=true\n");
        out.push_str("PrivateTmp=true\n");
        out.push_str("NoNewPrivileges=true\n");
        out.push_str("RestrictNamespaces=true\n");
        out.push_str("\n");

        out.push_str("[Install]\n");
        out.push_str(&format!("WantedBy={}\n", self.wanted_by));

        out
    }
}
