use super::error::WpCliError;
use super::executor::WpCliExecutor;
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Deserialize)]
pub struct WpCoreVersionInfo {
    pub version: String,
}

pub struct WpCli {
    executor: WpCliExecutor,
}

impl WpCli {
    pub fn new(executor: WpCliExecutor) -> Self {
        Self { executor }
    }

    /// Run `wp core version`
    pub async fn core_version(&self, site_path: &Path) -> Result<String, WpCliError> {
        let args = ["core", "version"];
        let output = self.executor.run_command(&args, Some(site_path)).await?;
        Ok(output.trim().to_string())
    }

    /// Run `wp core is-installed`
    pub async fn is_installed(&self, site_path: &Path) -> Result<bool, WpCliError> {
        let args = ["core", "is-installed"];
        match self.executor.run_command(&args, Some(site_path)).await {
            Ok(_) => Ok(true),
            Err(WpCliError::CommandFailed(1, _)) => Ok(false),
            Err(e) => Err(e),
        }
    }

    /// Run `wp plugin list --format=json`
    pub async fn plugin_list(&self, site_path: &Path) -> Result<Vec<WpPlugin>, WpCliError> {
        let args = ["plugin", "list", "--format=json"];
        let output = self.executor.run_command(&args, Some(site_path)).await?;
        let plugins: Vec<WpPlugin> = serde_json::from_str(&output)?;
        Ok(plugins)
    }

    /// Run any command with JSON output
    pub async fn run_json<T: for<'de> Deserialize<'de>>(
        &self,
        args: &[&str],
        site_path: Option<&Path>,
    ) -> Result<T, WpCliError> {
        let mut final_args = args.to_vec();
        final_args.push("--format=json");

        let output = self.executor.run_command(&final_args, site_path).await?;
        let result: T = serde_json::from_str(&output)?;
        Ok(result)
    }
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
pub struct WpPlugin {
    pub name: String,
    pub status: String,
    pub update: String,
    pub version: String,
}
