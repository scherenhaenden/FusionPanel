use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::error::DeploymentError;
use crate::types::{BuildSpec, ProjectType};

/// Redacts secrets from build logs given known secret patterns or explicit sensitive values.
pub fn redact_secrets(input: &str, secrets: &[&str]) -> String {
    let mut sanitized = input.to_string();
    for secret in secrets {
        if !secret.trim().is_empty() {
            sanitized = sanitized.replace(secret, "[REDACTED]");
        }
    }
    sanitized
}

/// Detects project type based on files present in the root of the repository.
pub fn detect_project_type(repo_dir: &Path) -> ProjectType {
    if repo_dir.join("Cargo.toml").exists() {
        ProjectType::Rust
    } else if repo_dir.join("package.json").exists() {
        ProjectType::Node
    } else if repo_dir.join("requirements.txt").exists()
        || repo_dir.join("pyproject.toml").exists()
        || repo_dir.join("Pipfile").exists()
    {
        ProjectType::Python
    } else if repo_dir.join("index.html").exists() {
        ProjectType::Static
    } else {
        ProjectType::Custom
    }
}

/// Standard build plan derived for a project
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedBuildCommand {
    pub program: String,
    pub args: Vec<String>,
    pub env_vars: HashMap<String, String>,
    pub output_dir: PathBuf,
}

/// Generates standard safe build instructions without shell interpolation
pub fn plan_build(
    repo_dir: &Path,
    spec: &BuildSpec,
    output_dir: &Path,
) -> Result<PlannedBuildCommand, DeploymentError> {
    let project_type = match spec.project_type {
        ProjectType::Custom => detect_project_type(repo_dir),
        other => other,
    };

    match project_type {
        ProjectType::Static => {
            // Static sites don't need compilation, artifact is repo root or artifact_path
            let src_dir = match &spec.artifact_path {
                Some(p) => repo_dir.join(p),
                None => repo_dir.to_path_buf(),
            };
            Ok(PlannedBuildCommand {
                program: "cp".to_string(),
                args: vec![
                    "-r".to_string(),
                    format!("{}/.", src_dir.display()),
                    output_dir.display().to_string(),
                ],
                env_vars: HashMap::new(),
                output_dir: output_dir.to_path_buf(),
            })
        }
        ProjectType::Rust => {
            let args = vec!["build".to_string(), "--release".to_string()];
            if let Some(cmd) = &spec.command {
                // If custom args specified
                let split_args: Vec<String> =
                    cmd.split_whitespace().map(|s| s.to_string()).collect();
                if !split_args.is_empty() {
                    return Ok(PlannedBuildCommand {
                        program: split_args[0].clone(),
                        args: split_args[1..].to_vec(),
                        env_vars: HashMap::new(),
                        output_dir: output_dir.to_path_buf(),
                    });
                }
            }
            Ok(PlannedBuildCommand {
                program: "cargo".to_string(),
                args,
                env_vars: HashMap::new(),
                output_dir: output_dir.to_path_buf(),
            })
        }
        ProjectType::Node => {
            let (prog, args) = match &spec.command {
                Some(cmd) => {
                    let split: Vec<String> =
                        cmd.split_whitespace().map(|s| s.to_string()).collect();
                    if split.is_empty() {
                        (
                            "npm".to_string(),
                            vec!["run".to_string(), "build".to_string()],
                        )
                    } else {
                        (split[0].clone(), split[1..].to_vec())
                    }
                }
                None => (
                    "npm".to_string(),
                    vec!["run".to_string(), "build".to_string()],
                ),
            };
            Ok(PlannedBuildCommand {
                program: prog,
                args,
                env_vars: HashMap::new(),
                output_dir: output_dir.to_path_buf(),
            })
        }
        ProjectType::Python => {
            let (prog, args) = match &spec.command {
                Some(cmd) => {
                    let split: Vec<String> =
                        cmd.split_whitespace().map(|s| s.to_string()).collect();
                    if split.is_empty() {
                        (
                            "python3".to_string(),
                            vec!["-m".to_string(), "build".to_string()],
                        )
                    } else {
                        (split[0].clone(), split[1..].to_vec())
                    }
                }
                None => (
                    "python3".to_string(),
                    vec!["-m".to_string(), "build".to_string()],
                ),
            };
            Ok(PlannedBuildCommand {
                program: prog,
                args,
                env_vars: HashMap::new(),
                output_dir: output_dir.to_path_buf(),
            })
        }
        ProjectType::Custom => {
            let cmd = spec.command.as_deref().ok_or_else(|| {
                DeploymentError::InvalidInput(
                    "custom project type requires an explicit build command".to_string(),
                )
            })?;
            let split: Vec<String> = cmd.split_whitespace().map(|s| s.to_string()).collect();
            if split.is_empty() {
                return Err(DeploymentError::InvalidInput(
                    "empty build command".to_string(),
                ));
            }
            Ok(PlannedBuildCommand {
                program: split[0].clone(),
                args: split[1..].to_vec(),
                env_vars: HashMap::new(),
                output_dir: output_dir.to_path_buf(),
            })
        }
    }
}

/// Execution outcome of a build
#[derive(Debug, Clone)]
pub struct BuildOutcome {
    pub success: bool,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

/// Executes a planned build command in the isolated working directory
pub fn execute_build(
    working_dir: &Path,
    planned: &PlannedBuildCommand,
    secret_redaction_list: &[&str],
) -> Result<BuildOutcome, DeploymentError> {
    if !working_dir.exists() {
        return Err(DeploymentError::InvalidInput(format!(
            "working directory does not exist: {}",
            working_dir.display()
        )));
    }

    if !planned.output_dir.exists() {
        fs::create_dir_all(&planned.output_dir).map_err(|e| {
            DeploymentError::OperationFailed(format!(
                "failed to create build output directory {}: {e}",
                planned.output_dir.display()
            ))
        })?;
    }

    let mut cmd = Command::new(&planned.program);
    cmd.current_dir(working_dir);
    cmd.args(&planned.args);

    for (k, v) in &planned.env_vars {
        cmd.env(k, v);
    }

    let output = cmd.output().map_err(|e| {
        DeploymentError::OperationFailed(format!(
            "failed to spawn build process '{}': {e}",
            planned.program
        ))
    })?;

    let stdout_raw = String::from_utf8_lossy(&output.stdout);
    let stderr_raw = String::from_utf8_lossy(&output.stderr);

    let sanitized_stdout = redact_secrets(&stdout_raw, secret_redaction_list);
    let sanitized_stderr = redact_secrets(&stderr_raw, secret_redaction_list);

    Ok(BuildOutcome {
        success: output.status.success(),
        exit_code: output.status.code(),
        stdout: sanitized_stdout,
        stderr: sanitized_stderr,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_secret_redaction() {
        let text = "Connecting with token ghp_secret12345 to repo";
        let redacted = redact_secrets(text, &["ghp_secret12345"]);
        assert_eq!(redacted, "Connecting with token [REDACTED] to repo");
        assert!(!redacted.contains("ghp_secret12345"));
    }

    #[test]
    fn test_detect_project_types() {
        let temp = TempDir::new().unwrap();
        assert_eq!(detect_project_type(temp.path()), ProjectType::Custom);

        fs::write(temp.path().join("Cargo.toml"), b"[package]").unwrap();
        assert_eq!(detect_project_type(temp.path()), ProjectType::Rust);

        let temp2 = TempDir::new().unwrap();
        fs::write(temp2.path().join("package.json"), b"{}").unwrap();
        assert_eq!(detect_project_type(temp2.path()), ProjectType::Node);

        let temp3 = TempDir::new().unwrap();
        fs::write(temp3.path().join("requirements.txt"), b"requests").unwrap();
        assert_eq!(detect_project_type(temp3.path()), ProjectType::Python);

        let temp4 = TempDir::new().unwrap();
        fs::write(temp4.path().join("index.html"), b"<h1>Hello</h1>").unwrap();
        assert_eq!(detect_project_type(temp4.path()), ProjectType::Static);
    }

    #[test]
    fn test_plan_and_execute_static_build() {
        let repo_dir = TempDir::new().unwrap();
        let out_dir = TempDir::new().unwrap();
        fs::write(repo_dir.path().join("index.html"), b"<h1>Static</h1>").unwrap();

        let spec = BuildSpec {
            project_type: ProjectType::Static,
            command: None,
            artifact_path: None,
        };

        let planned = plan_build(repo_dir.path(), &spec, out_dir.path()).unwrap();
        assert_eq!(planned.program, "cp");

        let outcome = execute_build(repo_dir.path(), &planned, &[]).unwrap();
        assert!(outcome.success, "Static build failed: {}", outcome.stderr);
        assert!(out_dir.path().join("index.html").exists());
        let content = fs::read_to_string(out_dir.path().join("index.html")).unwrap();
        assert_eq!(content, "<h1>Static</h1>");
    }
}
