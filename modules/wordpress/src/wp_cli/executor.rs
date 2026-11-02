use super::error::WpCliError;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::process::Command;
use tokio::time::timeout;

#[derive(Debug, Clone)]
pub struct WpCliExecutor {
    wp_path: PathBuf,
    default_timeout: Duration,
    allow_root: bool,
}

impl Default for WpCliExecutor {
    fn default() -> Self {
        Self {
            wp_path: PathBuf::from("wp"),
            default_timeout: Duration::from_secs(30),
            allow_root: false,
        }
    }
}

impl WpCliExecutor {
    pub fn new<P: AsRef<Path>>(wp_path: P) -> Self {
        Self {
            wp_path: wp_path.as_ref().to_path_buf(),
            default_timeout: Duration::from_secs(30),
            allow_root: false,
        }
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.default_timeout = timeout;
        self
    }

    pub fn allow_root(mut self, allow: bool) -> Self {
        self.allow_root = allow;
        self
    }

    pub async fn run_command(
        &self,
        args: &[&str],
        site_path: Option<&Path>,
    ) -> Result<String, WpCliError> {
        let mut cmd = Command::new(&self.wp_path);

        if let Some(path) = site_path {
            cmd.arg(format!("--path={}", path.display()));
        }

        if self.allow_root {
            cmd.arg("--allow-root");
        }

        for arg in args {
            cmd.arg(arg);
        }

        cmd.kill_on_drop(true);

        let child = cmd.output();

        let output = timeout(self.default_timeout, child)
            .await
            .map_err(|_| WpCliError::Timeout(self.default_timeout.as_secs()))??;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            if let Some(code) = output.status.code() {
                return Err(WpCliError::CommandFailed(code, stderr));
            } else {
                return Err(WpCliError::TerminatedBySignal);
            }
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}
