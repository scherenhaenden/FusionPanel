use std::process::Stdio;
use async_trait::async_trait;
use tokio::process::Command;
use tokio::io::AsyncWriteExt;

use fusionpanel_agent_protocol::{Request, Response};
use crate::worker::AgentClient;

pub struct SubprocessAgentClient {
    pub executable_path: String,
}

#[async_trait]
impl AgentClient for SubprocessAgentClient {
    async fn execute(&self, request: Request) -> Result<Response, String> {
        let payload = serde_json::to_string(&request).map_err(|e| e.to_string())?;

        let mut child = Command::new(&self.executable_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| e.to_string())?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(payload.as_bytes()).await.map_err(|e| e.to_string())?;
            // explicitly drop stdin to signal EOF to the child
            drop(stdin);
        }

        let output = child.wait_with_output().await.map_err(|e| e.to_string())?;

        if !output.status.success() {
            let stderr_str = String::from_utf8_lossy(&output.stderr);
            return Err(format!("Process exited with status {}: {}", output.status, stderr_str));
        }

        let response: Response = serde_json::from_slice(&output.stdout).map_err(|e| format!("Failed to parse output: {}", e))?;
        Ok(response)
    }
}
