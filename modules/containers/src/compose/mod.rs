pub mod models;
pub mod validator;

use models::ComposeFile;
use regex::Regex;
use std::collections::HashMap;
use std::fs;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ComposeError {
    #[error("Failed to read file: {0}")]
    Io(#[from] std::io::Error),
    #[error("Failed to parse YAML: {0}")]
    Parse(#[from] serde_yaml::Error),
    #[error("Validation failed: {0}")]
    Validation(#[from] validator::ValidationError),
}

pub fn parse_and_validate_with_env(
    yaml_content: &str,
    env: &HashMap<String, String>,
) -> Result<ComposeFile, ComposeError> {
    let substituted_yaml = substitute_env_vars(yaml_content, env);
    let compose: ComposeFile = serde_yaml::from_str(&substituted_yaml)?;
    validator::validate_compose(&compose)?;
    Ok(compose)
}

pub fn parse_and_validate(yaml_content: &str) -> Result<ComposeFile, ComposeError> {
    parse_and_validate_with_env(yaml_content, &HashMap::new())
}

pub fn parse_and_validate_file(path: &str) -> Result<ComposeFile, ComposeError> {
    let content = fs::read_to_string(path)?;
    parse_and_validate(&content)
}

fn substitute_env_vars(content: &str, env: &HashMap<String, String>) -> String {
    // Standard Docker Compose environment variable interpolation format:
    // Support $VARIABLE, ${VARIABLE}, ${VARIABLE:-default}, and escape $$VARIABLE

    // Replace $$ with a temporary placeholder that won't match our real variables
    let temp_escaped = content.replace("$$", "\x00ESCAPED_DOLLAR\x00");

    // Matches $VAR and ${VAR} and ${VAR:-default}
    let re = Regex::new(r"\$(?:([A-Za-z0-9_]+)|\{([A-Za-z0-9_]+)(?::-([^}]+))?\}?)").unwrap();

    let substituted = re
        .replace_all(&temp_escaped, |caps: &regex::Captures| {
            // cap 1 is $VAR
            // cap 2 is ${VAR} or ${VAR:-default}
            // cap 3 is default

            let var_name = caps
                .get(1)
                .or_else(|| caps.get(2))
                .map(|m| m.as_str())
                .unwrap_or("");
            let default_val = caps.get(3).map(|m| m.as_str());

            if let Some(val) = env.get(var_name) {
                val.clone()
            } else if let Some(val) = default_val {
                val.to_string()
            } else {
                "".to_string() // Empty string if not found and no default
            }
        })
        .to_string();

    // Restore the escaped $$ as $
    substituted.replace("\x00ESCAPED_DOLLAR\x00", "$$")
}
