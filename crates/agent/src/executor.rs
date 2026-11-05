#![forbid(unsafe_code)]

use std::collections::HashMap;
use std::process::Stdio;
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::process::Command;
use tokio::time::timeout;

#[derive(Debug, Clone)]
pub struct CommandRequest {
    pub cmd: String,
    pub args: Vec<String>,
    pub envs: HashMap<String, String>,
    pub timeout: Duration,
}

#[derive(Debug, Clone)]
pub struct CommandResult {
    pub stdout: Vec<u8>,
    #[allow(dead_code)]
    pub stderr: Vec<u8>,
    pub exit_code: Option<i32>,
    pub timeout: bool,
}

#[derive(Debug)]
pub struct CommandExecutor;

const MAX_OUTPUT_SIZE: u64 = 1024 * 1024; // 1 MB cap

impl CommandExecutor {
    pub async fn execute(req: &CommandRequest) -> std::io::Result<CommandResult> {
        let mut command = Command::new(&req.cmd);
        command.args(&req.args);
        command.envs(&req.envs);
        command.stdout(Stdio::piped());
        command.stderr(Stdio::piped());
        command.kill_on_drop(true);

        let mut child = command.spawn()?;

        let stdout = child.stdout.take().expect("Failed to open stdout");
        let stderr = child.stderr.take().expect("Failed to open stderr");

        let mut stdout_buf = Vec::new();
        let mut stderr_buf = Vec::new();

        let stdout_handle = tokio::spawn(async move {
            let mut handle = stdout.take(MAX_OUTPUT_SIZE);
            handle.read_to_end(&mut stdout_buf).await?;
            Ok::<_, std::io::Error>(stdout_buf)
        });

        let stderr_handle = tokio::spawn(async move {
            let mut handle = stderr.take(MAX_OUTPUT_SIZE);
            handle.read_to_end(&mut stderr_buf).await?;
            Ok::<_, std::io::Error>(stderr_buf)
        });

        let wait_future = child.wait();
        let result = match timeout(req.timeout, wait_future).await {
            Ok(Ok(status)) => {
                let stdout_res = stdout_handle.await.unwrap_or_else(|_| Ok(Vec::new()))?;
                let stderr_res = stderr_handle.await.unwrap_or_else(|_| Ok(Vec::new()))?;
                CommandResult {
                    stdout: stdout_res,
                    stderr: stderr_res,
                    exit_code: status.code(),
                    timeout: false,
                }
            }
            Ok(Err(e)) => return Err(e),
            Err(_) => {
                let _ = child.kill().await;

                let stdout_res = stdout_handle.await.unwrap_or_else(|_| Ok(Vec::new()))?;
                let stderr_res = stderr_handle.await.unwrap_or_else(|_| Ok(Vec::new()))?;

                CommandResult {
                    stdout: stdout_res,
                    stderr: stderr_res,
                    exit_code: None,
                    timeout: true,
                }
            }
        };

        Ok(result)
    }

    pub fn redact_env(
        envs: &HashMap<String, String>,
        sensitive_keys: &[&str],
    ) -> HashMap<String, String> {
        let mut redacted = envs.clone();
        for key in sensitive_keys {
            if redacted.contains_key(*key) {
                redacted.insert(key.to_string(), "[REDACTED]".to_string());
            }
        }
        redacted
    }
}
