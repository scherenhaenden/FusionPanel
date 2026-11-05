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

        if let Some(owner) = values.get("OWNER") {
            final_values
                .entry("OWNER".to_string())
                .or_insert_with(|| owner.clone());
        }

        render_yaml_template(&self.compose_template, &final_values, false)
    }
}

pub(crate) fn render_yaml_template(
    template: &str,
    values: &HashMap<String, String>,
    allow_unbraced: bool,
) -> Result<String, TemplateError> {
    let mut document: serde_yaml::Value = serde_yaml::from_str(template)
        .map_err(|error| TemplateError::ParseError(error.to_string()))?;
    render_yaml_value(&mut document, values, allow_unbraced)?;
    serde_yaml::to_string(&document).map_err(|error| TemplateError::ParseError(error.to_string()))
}

fn render_yaml_value(
    value: &mut serde_yaml::Value,
    values: &HashMap<String, String>,
    allow_unbraced: bool,
) -> Result<(), TemplateError> {
    match value {
        serde_yaml::Value::String(scalar) => {
            *scalar = replace_template_tokens(scalar, values, allow_unbraced);
        }
        serde_yaml::Value::Sequence(sequence) => {
            for item in sequence {
                render_yaml_value(item, values, allow_unbraced)?;
            }
        }
        serde_yaml::Value::Mapping(mapping) => {
            for (key, item) in mapping.iter_mut() {
                if let Some(key) = key.as_str() {
                    if values.keys().any(|name| {
                        key.contains(&format!("${{{name}}}"))
                            || (allow_unbraced && key.contains(&format!("${name}")))
                    }) {
                        return Err(TemplateError::ParseError(
                            "Template variables in YAML mapping keys are not supported".to_string(),
                        ));
                    }
                }
                render_yaml_value(item, values, allow_unbraced)?;
            }
        }
        serde_yaml::Value::Tagged(tagged) => {
            render_yaml_value(&mut tagged.value, values, allow_unbraced)?;
        }
        serde_yaml::Value::Null | serde_yaml::Value::Bool(_) | serde_yaml::Value::Number(_) => {}
    }
    Ok(())
}

fn replace_template_tokens(
    template: &str,
    values: &HashMap<String, String>,
    allow_unbraced: bool,
) -> String {
    let mut rendered = String::new();
    let mut cursor = 0;

    for (dollar, _) in template.match_indices('$') {
        if dollar < cursor || (dollar > 0 && template.as_bytes()[dollar - 1] == b'$') {
            continue;
        }
        let after_dollar = dollar + 1;
        let (name, end) = if template[after_dollar..].starts_with('{') {
            let name_start = after_dollar + 1;
            let Some(relative_end) = template[name_start..].find('}') else {
                continue;
            };
            let end = name_start + relative_end + 1;
            (&template[name_start..end - 1], end)
        } else if allow_unbraced {
            let name_start = after_dollar;
            let mut end = name_start;
            for (offset, character) in template[name_start..].char_indices() {
                if character.is_ascii_alphanumeric() || character == '_' {
                    end = name_start + offset + character.len_utf8();
                } else {
                    break;
                }
            }
            if end == name_start {
                continue;
            }
            (&template[name_start..end], end)
        } else {
            continue;
        };

        if let Some(value) = values.get(name) {
            rendered.push_str(&template[cursor..dollar]);
            rendered.push_str(&value.replace('$', "$$"));
            cursor = end;
        }
    }
    rendered.push_str(&template[cursor..]);
    rendered
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
        assert_eq!(
            serde_yaml::from_str::<String>(&result).unwrap(),
            "VAR1 is val1 and VAR2 is default_val"
        );

        values.insert("VAR2".to_string(), "override".to_string());
        let result2 = template.render(&values).unwrap();
        assert_eq!(
            serde_yaml::from_str::<String>(&result2).unwrap(),
            "VAR1 is val1 and VAR2 is override"
        );
    }

    #[test]
    fn test_variable_values_cannot_change_compose_yaml_structure() {
        let template = AppTemplate {
            name: "Test".to_string(),
            version: "1.0".to_string(),
            category: "Test".to_string(),
            install_modes: vec![],
            unsupported_hardware: vec![],
            variables: {
                let mut variables = HashMap::new();
                variables.insert(
                    "VALUE".to_string(),
                    Variable {
                        description: None,
                        required: true,
                        default: None,
                        regex_validation: None,
                    },
                );
                variables
            },
            volumes: vec![],
            compose_template: "services:\n  app:\n    labels:\n      value: \"${VALUE}\"\n"
                .to_string(),
        };
        let values = HashMap::from([(
            "VALUE".to_string(),
            "safe\n  privileged: true ${LATE}".to_string(),
        )]);

        let rendered = template.render(&values).unwrap();
        let document: serde_yaml::Value = serde_yaml::from_str(&rendered).unwrap();
        let labels = &document["services"]["app"]["labels"];
        assert_eq!(
            labels["value"].as_str(),
            Some("safe\n  privileged: true $${LATE}")
        );
        assert!(document.get("privileged").is_none());
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
