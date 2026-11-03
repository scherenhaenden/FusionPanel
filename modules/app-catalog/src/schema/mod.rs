#![forbid(unsafe_code)]

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppTemplate {
    pub name: String,
    pub version: String,
    pub category: String,
    #[serde(default)]
    pub install_modes: Vec<String>,
    #[serde(default)]
    pub unsupported_hardware: Vec<String>,
    #[serde(default)]
    pub variables: HashMap<String, Variable>,
    #[serde(default)]
    pub volumes: Vec<Volume>,
    pub compose_template: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Variable {
    pub description: Option<String>,
    #[serde(default)]
    pub required: bool,
    pub default: Option<String>,
    pub regex_validation: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Volume {
    pub name: String,
    pub mount_path: String,
}

#[derive(Debug, Error)]
pub enum TemplateError {
    #[error("Missing required variable: {0}")]
    MissingVariable(String),
    #[error("Validation failed for variable '{0}': value '{1}' does not match regex '{2}'")]
    ValidationFailed(String, String, String),
    #[error("Invalid regex in schema for variable '{0}': {1}")]
    InvalidRegex(String, String),
    #[error("Failed to parse template: {0}")]
    ParseError(String),
}

impl AppTemplate {
    pub fn render(&self, values: &HashMap<String, String>) -> Result<String, TemplateError> {
        // Validate and gather all variable values
        let mut final_values = HashMap::new();
        for (var_name, var_def) in &self.variables {
            let value = match values.get(var_name) {
                Some(v) => v.clone(),
                None => {
                    if var_def.required {
                        return Err(TemplateError::MissingVariable(var_name.clone()));
                    }
                    var_def.default.clone().unwrap_or_default()
                }
            };

            // Apply regex validation if specified
            if let Some(regex_str) = &var_def.regex_validation {
                let re = Regex::new(regex_str)
                    .map_err(|e| TemplateError::InvalidRegex(var_name.clone(), e.to_string()))?;
                if !re.is_match(&value) {
                    return Err(TemplateError::ValidationFailed(
                        var_name.clone(),
                        value.clone(),
                        regex_str.clone(),
                    ));
                }
            }

            final_values.insert(var_name.clone(), value);
        }

        // Check for any extra variables passed that are not in the schema (optional, depending on requirements, let's just ignore them)

        // Simple string replacement for now.
        // We will replace ${VAR_NAME} with the value.
        let mut rendered = self.compose_template.clone();
        for (k, v) in final_values {
            let placeholder = format!("${{{}}}", k);
            rendered = rendered.replace(&placeholder, &v);
        }

        Ok(rendered)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialization() {
        let yaml = r#"
name: My App
version: 1.0.0
category: Web
variables:
  PORT:
    description: "The port to listen on"
    required: true
    regex_validation: "^[0-9]+$"
  MODE:
    default: "production"
volumes:
  - name: data
    mount_path: /app/data
compose_template: |
  services:
    app:
      image: myapp:${MODE}
      ports:
        - "${PORT}:80"
"#;
        let template: AppTemplate = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(template.name, "My App");
        assert_eq!(template.version, "1.0.0");
        assert_eq!(template.category, "Web");
        assert_eq!(template.variables.len(), 2);
        assert_eq!(template.volumes.len(), 1);
        assert_eq!(template.volumes[0].name, "data");
        assert_eq!(template.volumes[0].mount_path, "/app/data");
        assert!(template.variables["PORT"].required);
        assert_eq!(
            template.variables["PORT"].regex_validation.as_deref(),
            Some("^[0-9]+$")
        );
        assert_eq!(
            template.variables["MODE"].default.as_deref(),
            Some("production")
        );
        assert!(!template.variables["MODE"].required);
    }

    #[test]
    fn test_interpolation_success() {
        let template = AppTemplate {
            name: "Test".to_string(),
            version: "1.0".to_string(),
            category: "Test".to_string(),
            install_modes: vec![],
            unsupported_hardware: vec![],
            variables: {
                let mut m = HashMap::new();
                m.insert(
                    "VAR1".to_string(),
                    Variable {
                        description: None,
                        required: true,
                        default: None,
                        regex_validation: None,
                    },
                );
                m.insert(
                    "VAR2".to_string(),
                    Variable {
                        description: None,
                        required: false,
                        default: Some("default_val".to_string()),
                        regex_validation: None,
                    },
                );
                m
            },
            volumes: vec![],
            compose_template: "VAR1 is ${VAR1} and VAR2 is ${VAR2}".to_string(),
        };

        let mut values = HashMap::new();
        values.insert("VAR1".to_string(), "val1".to_string());

        let result = template.render(&values).unwrap();
        assert_eq!(result, "VAR1 is val1 and VAR2 is default_val");

        values.insert("VAR2".to_string(), "override".to_string());
        let result2 = template.render(&values).unwrap();
        assert_eq!(result2, "VAR1 is val1 and VAR2 is override");
    }

    #[test]
    fn test_missing_required_variable() {
        let template = AppTemplate {
            name: "Test".to_string(),
            version: "1.0".to_string(),
            category: "Test".to_string(),
            install_modes: vec![],
            unsupported_hardware: vec![],
            variables: {
                let mut m = HashMap::new();
                m.insert(
                    "VAR1".to_string(),
                    Variable {
                        description: None,
                        required: true,
                        default: None,
                        regex_validation: None,
                    },
                );
                m
            },
            volumes: vec![],
            compose_template: "VAR1 is ${VAR1}".to_string(),
        };

        let values = HashMap::new();
        let result = template.render(&values);
        assert!(matches!(result, Err(TemplateError::MissingVariable(v)) if v == "VAR1"));
    }

    #[test]
    fn test_regex_validation() {
        let template = AppTemplate {
            name: "Test".to_string(),
            version: "1.0".to_string(),
            category: "Test".to_string(),
            install_modes: vec![],
            unsupported_hardware: vec![],
            variables: {
                let mut m = HashMap::new();
                m.insert(
                    "PORT".to_string(),
                    Variable {
                        description: None,
                        required: true,
                        default: None,
                        regex_validation: Some("^[0-9]{1,5}$".to_string()),
                    },
                );
                m
            },
            volumes: vec![],
            compose_template: "PORT is ${PORT}".to_string(),
        };

        let mut values = HashMap::new();
        values.insert("PORT".to_string(), "8080".to_string());
        assert!(template.render(&values).is_ok());

        values.insert("PORT".to_string(), "abc".to_string());
        let err = template.render(&values);
        assert!(
            matches!(err, Err(TemplateError::ValidationFailed(v, val, _)) if v == "PORT" && val == "abc")
        );
    }

    #[test]
    fn test_invalid_regex_in_schema() {
        let template = AppTemplate {
            name: "Test".to_string(),
            version: "1.0".to_string(),
            category: "Test".to_string(),
            install_modes: vec![],
            unsupported_hardware: vec![],
            variables: {
                let mut m = HashMap::new();
                m.insert(
                    "VAR1".to_string(),
                    Variable {
                        description: None,
                        required: true,
                        default: None,
                        regex_validation: Some("[unclosed".to_string()),
                    },
                );
                m
            },
            volumes: vec![],
            compose_template: "VAR1 is ${VAR1}".to_string(),
        };

        let mut values = HashMap::new();
        values.insert("VAR1".to_string(), "val".to_string());
        let err = template.render(&values);
        assert!(matches!(err, Err(TemplateError::InvalidRegex(v, _)) if v == "VAR1"));
    }
}
