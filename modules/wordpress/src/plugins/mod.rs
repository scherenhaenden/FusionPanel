use std::process::Command;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum WpCliError {
    #[error("Invalid slug format: {0}")]
    InvalidSlug(String),
    #[error("WP-CLI command failed: {0}")]
    CommandFailed(String),
}

/// Validates a plugin or theme slug.
/// Slugs must contain only lowercase letters, digits, and hyphens.
pub fn validate_slug(slug: &str) -> Result<(), WpCliError> {
    if slug.is_empty() {
        return Err(WpCliError::InvalidSlug("Slug cannot be empty".to_string()));
    }
    if slug.len() > 100 {
        return Err(WpCliError::InvalidSlug("Slug is too long".to_string()));
    }
    if !slug
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        return Err(WpCliError::InvalidSlug(
            "Slug can only contain lowercase letters, digits, and hyphens".to_string(),
        ));
    }
    if slug.starts_with('-') || slug.ends_with('-') {
        return Err(WpCliError::InvalidSlug(
            "Slug cannot start or end with a hyphen".to_string(),
        ));
    }
    Ok(())
}

/// Trait to allow abstracting the execution of WP-CLI commands.
/// Useful for unit testing.
pub trait WpCliExecutor {
    fn execute(&self, args: &[&str], path: &str) -> Result<String, WpCliError>;
}

/// A real WP-CLI executor that runs commands via `std::process::Command`.
pub struct RealWpCliExecutor;

impl WpCliExecutor for RealWpCliExecutor {
    fn execute(&self, args: &[&str], path: &str) -> Result<String, WpCliError> {
        let output = Command::new("wp")
            .args(args)
            .arg(format!("--path={}", path))
            .arg("--allow-root") // Just for safety if run under root, though discouraged
            .output()
            .map_err(|e| WpCliError::CommandFailed(e.to_string()))?;

        if !output.status.success() {
            return Err(WpCliError::CommandFailed(
                String::from_utf8_lossy(&output.stderr).to_string(),
            ));
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}

pub struct PluginManager<E: WpCliExecutor> {
    pub executor: E,
    path: String,
}

impl<E: WpCliExecutor> PluginManager<E> {
    pub fn new(executor: E, path: impl Into<String>) -> Self {
        Self {
            executor,
            path: path.into(),
        }
    }

    pub fn install(&self, slug: &str) -> Result<(), WpCliError> {
        validate_slug(slug)?;
        self.executor
            .execute(&["plugin", "install", slug], &self.path)?;
        Ok(())
    }

    pub fn activate(&self, slug: &str) -> Result<(), WpCliError> {
        validate_slug(slug)?;
        self.executor
            .execute(&["plugin", "activate", slug], &self.path)?;
        Ok(())
    }

    pub fn deactivate(&self, slug: &str) -> Result<(), WpCliError> {
        validate_slug(slug)?;
        self.executor
            .execute(&["plugin", "deactivate", slug], &self.path)?;
        Ok(())
    }

    pub fn update(&self, slug: &str) -> Result<(), WpCliError> {
        validate_slug(slug)?;
        self.executor
            .execute(&["plugin", "update", slug], &self.path)?;
        Ok(())
    }

    pub fn remove(&self, slug: &str) -> Result<(), WpCliError> {
        validate_slug(slug)?;
        self.executor
            .execute(&["plugin", "uninstall", slug], &self.path)?;
        Ok(())
    }
}

pub struct ThemeManager<E: WpCliExecutor> {
    pub executor: E,
    path: String,
}

impl<E: WpCliExecutor> ThemeManager<E> {
    pub fn new(executor: E, path: impl Into<String>) -> Self {
        Self {
            executor,
            path: path.into(),
        }
    }

    pub fn install(&self, slug: &str) -> Result<(), WpCliError> {
        validate_slug(slug)?;
        self.executor
            .execute(&["theme", "install", slug], &self.path)?;
        Ok(())
    }

    pub fn activate(&self, slug: &str) -> Result<(), WpCliError> {
        validate_slug(slug)?;
        self.executor
            .execute(&["theme", "activate", slug], &self.path)?;
        Ok(())
    }

    pub fn update(&self, slug: &str) -> Result<(), WpCliError> {
        validate_slug(slug)?;
        self.executor
            .execute(&["theme", "update", slug], &self.path)?;
        Ok(())
    }

    pub fn remove(&self, slug: &str) -> Result<(), WpCliError> {
        validate_slug(slug)?;
        self.executor
            .execute(&["theme", "uninstall", slug], &self.path)?;
        Ok(())
    }
}
