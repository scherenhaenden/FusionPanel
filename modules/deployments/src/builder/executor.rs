use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;
use tokio::io::{AsyncReadExt, BufReader};
use tokio::process::Command;
use tokio::time::timeout;

#[derive(Debug, thiserror::Error)]
pub enum ExecutorError {
    #[error("Command execution failed: {0}")]
    ExecutionFailed(String),
    #[error("Command timed out after {0} seconds. Logs: {1}")]
    Timeout(u64, String),
    #[error("IO Error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StepConfig {
    pub command: String,
    pub args: Vec<String>,
    pub env: HashMap<String, String>,
    pub working_dir: PathBuf,
    pub timeout_seconds: u64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StepResult {
    pub exit_code: i32,
    pub logs: String,
}

pub struct StepExecutor;

impl StepExecutor {
    pub async fn execute(config: &StepConfig) -> Result<StepResult, ExecutorError> {
        let mut cmd = Command::new(&config.command);

        // Retain PATH for basic command resolution if env_clear is used,
        // or just let env clear apply and expect users to pass PATH, but
        // for simplicity we will preserve PATH by fetching it from the system env.
        let system_path = std::env::var("PATH").unwrap_or_default();

        cmd.args(&config.args)
            .current_dir(&config.working_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            // Ensure no inherited env vars leak in (basic isolation)
            .env_clear()
            .env("PATH", system_path)
            .envs(&config.env);

        let mut child = cmd.spawn()?;

        let stdout = child.stdout.take().expect("Failed to open stdout");
        let stderr = child.stderr.take().expect("Failed to open stderr");

        let mut stdout_reader = BufReader::new(stdout);
        let mut stderr_reader = BufReader::new(stderr);

        let mut output_buffer = String::new();
        let mut stdout_buf = Vec::new();
        let mut stderr_buf = Vec::new();

        let timeout_duration = Duration::from_secs(config.timeout_seconds);

        let wait_result = timeout(timeout_duration, async {
            // Read stdout and stderr concurrently
            let stdout_fut = stdout_reader.read_to_end(&mut stdout_buf);
            let stderr_fut = stderr_reader.read_to_end(&mut stderr_buf);

            let (stdout_res, stderr_res) = tokio::join!(stdout_fut, stderr_fut);
            stdout_res?;
            stderr_res?;

            child.wait().await
        })
        .await;

        output_buffer.push_str(&String::from_utf8_lossy(&stdout_buf));
        output_buffer.push_str(&String::from_utf8_lossy(&stderr_buf));

        match wait_result {
            Ok(Ok(status)) => {
                let exit_code = status.code().unwrap_or(1);

                Ok(StepResult {
                    exit_code,
                    logs: output_buffer,
                })
            }
            Ok(Err(e)) => {
                let _ = child.kill().await;
                Err(ExecutorError::Io(e))
            }
            Err(_) => {
                // Ignore kill errors if it's already dead
                let _ = child.kill().await;
                Err(ExecutorError::Timeout(
                    config.timeout_seconds,
                    output_buffer,
                ))
            }
        }
    }
}
