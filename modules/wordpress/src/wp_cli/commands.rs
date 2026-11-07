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



    pub fn sanitize_arg(arg: &str) -> Result<&str, WpCliError> {
        // Since we are using std::process::Command and NOT invoking a shell (like sh -c),
        // we don't need to block all shell metacharacters like &, ;, |. 
        // We do want to prevent someone injecting flags like --allow-root when we just expect a value.
        if arg.starts_with("--") {
            return Err(WpCliError::InvalidArgument(format!("Invalid argument (starts with --): {}", arg)));
        }
        Ok(arg)
    }
    
    pub fn sanitize_url(url: &str) -> Result<&str, WpCliError> {
        if !url.starts_with("http://") && !url.starts_with("https://") {
            return Err(WpCliError::InvalidArgument(format!("Invalid URL: {}", url)));
        }
        Self::sanitize_arg(url)
    }

    pub fn sanitize_email(email: &str) -> Result<&str, WpCliError> {
        if !email.contains('@') {
            return Err(WpCliError::InvalidArgument(format!("Invalid email: {}", email)));
        }
        Self::sanitize_arg(email)
    }

    /// Run `wp core download`
    pub async fn core_download(&self, site_path: &Path, locale: Option<&str>, version: Option<&str>) -> Result<(), WpCliError> {
        let mut args = vec!["core", "download"];
        
        let locale_arg;
        if let Some(l) = locale {
            Self::sanitize_arg(l)?;
            locale_arg = format!("--locale={}", l);
            args.push(&locale_arg);
        }
        
        let version_arg;
        if let Some(v) = version {
            Self::sanitize_arg(v)?;
            version_arg = format!("--version={}", v);
            args.push(&version_arg);
        }
        
        self.executor.run_command(&args, Some(site_path)).await?;
        Ok(())
    }

    /// Run `wp core config`
    pub async fn core_config(&self, site_path: &Path, dbname: &str, dbuser: &str, dbpass: &str, dbhost: &str, dbprefix: &str) -> Result<(), WpCliError> {
        Self::sanitize_arg(dbname)?;
        Self::sanitize_arg(dbuser)?;
        // Passwords can be complex, but we shouldn't allow shell injection
        if dbpass.contains('\'') || dbpass.contains('\\') {
            return Err(WpCliError::InvalidArgument("Password contains invalid characters".to_string()));
        }
        Self::sanitize_arg(dbhost)?;
        Self::sanitize_arg(dbprefix)?;
        
        let dbname_arg = format!("--dbname={}", dbname);
        let dbuser_arg = format!("--dbuser={}", dbuser);
        let dbpass_arg = format!("--dbpass={}", dbpass);
        let dbhost_arg = format!("--dbhost={}", dbhost);
        let dbprefix_arg = format!("--dbprefix={}", dbprefix);
        
        let args = vec![
            "core", "config",
            &dbname_arg,
            &dbuser_arg,
            &dbpass_arg,
            &dbhost_arg,
            &dbprefix_arg,
        ];
        
        self.executor.run_command(&args, Some(site_path)).await?;
        Ok(())
    }

    /// Run `wp core install`
    pub async fn core_install(&self, site_path: &Path, url: &str, title: &str, admin_user: &str, admin_password: &str, admin_email: &str) -> Result<(), WpCliError> {
        Self::sanitize_url(url)?;
        Self::sanitize_arg(title)?;
        Self::sanitize_arg(admin_user)?;
        if admin_password.contains('\'') || admin_password.contains('\\') {
             return Err(WpCliError::InvalidArgument("Password contains invalid characters".to_string()));
        }
        Self::sanitize_email(admin_email)?;
        
        let url_arg = format!("--url={}", url);
        let title_arg = format!("--title={}", title);
        let admin_user_arg = format!("--admin_user={}", admin_user);
        let admin_password_arg = format!("--admin_password={}", admin_password);
        let admin_email_arg = format!("--admin_email={}", admin_email);
        
        let args = vec![
            "core", "install",
            &url_arg,
            &title_arg,
            &admin_user_arg,
            &admin_password_arg,
            &admin_email_arg,
        ];
        
        self.executor.run_command(&args, Some(site_path)).await?;
        Ok(())
    }

    /// Run `wp plugin install`
    pub async fn plugin_install(&self, site_path: &Path, slug: &str, activate: bool) -> Result<(), WpCliError> {
        Self::sanitize_arg(slug)?;
        let mut args = vec!["plugin", "install", slug];
        if activate {
            args.push("--activate");
        }
        self.executor.run_command(&args, Some(site_path)).await?;
        Ok(())
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
