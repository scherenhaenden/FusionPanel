pub mod models;
pub mod validator;

use models::ComposeFile;
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

pub fn parse_and_validate(yaml_content: &str) -> Result<ComposeFile, ComposeError> {
    let compose: ComposeFile = serde_yaml::from_str(yaml_content)?;
    validator::validate_compose(&compose)?;
    Ok(compose)
}

pub fn parse_and_validate_file(path: &str) -> Result<ComposeFile, ComposeError> {
    let content = fs::read_to_string(path)?;
    parse_and_validate(&content)
}
