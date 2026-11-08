#![forbid(unsafe_code)]

use std::cell::RefCell;
use wordpress::plugins::{validate_slug, PluginManager, ThemeManager, WpCliError, WpCliExecutor};

struct MockExecutor {
    executed_commands: RefCell<Vec<(Vec<String>, String)>>,
    mock_result: Result<String, WpCliError>,
}

impl MockExecutor {
    fn new(mock_result: Result<String, WpCliError>) -> Self {
        Self {
            executed_commands: RefCell::new(Vec::new()),
            mock_result,
        }
    }
}

impl WpCliExecutor for MockExecutor {
    fn execute(&self, args: &[&str], path: &str) -> Result<String, WpCliError> {
        self.executed_commands.borrow_mut().push((
            args.iter().map(|s| s.to_string()).collect(),
            path.to_string(),
        ));

        match &self.mock_result {
            Ok(s) => Ok(s.clone()),
            Err(e) => match e {
                WpCliError::InvalidSlug(m) => Err(WpCliError::InvalidSlug(m.clone())),
                WpCliError::CommandFailed(m) => Err(WpCliError::CommandFailed(m.clone())),
            },
        }
    }
}

#[test]
fn test_validate_slug_valid() {
    assert!(validate_slug("akismet").is_ok());
    assert!(validate_slug("contact-form-7").is_ok());
    assert!(validate_slug("wp-super-cache").is_ok());
    assert!(validate_slug("my-plugin-123").is_ok());
}

#[test]
fn test_validate_slug_invalid() {
    // Empty slug
    assert!(matches!(validate_slug(""), Err(WpCliError::InvalidSlug(_))));

    // Invalid characters
    assert!(matches!(
        validate_slug("Akismet"),
        Err(WpCliError::InvalidSlug(_))
    ));
    assert!(matches!(
        validate_slug("plugin_name"),
        Err(WpCliError::InvalidSlug(_))
    ));
    assert!(matches!(
        validate_slug("plugin name"),
        Err(WpCliError::InvalidSlug(_))
    ));
    assert!(matches!(
        validate_slug("plugin/name"),
        Err(WpCliError::InvalidSlug(_))
    ));

    // Starting/ending with hyphen
    assert!(matches!(
        validate_slug("-plugin"),
        Err(WpCliError::InvalidSlug(_))
    ));
    assert!(matches!(
        validate_slug("plugin-"),
        Err(WpCliError::InvalidSlug(_))
    ));

    // Too long
    let long_slug = "a".repeat(101);
    assert!(matches!(
        validate_slug(&long_slug),
        Err(WpCliError::InvalidSlug(_))
    ));
}

#[test]
fn test_plugin_install() {
    let mock = MockExecutor::new(Ok("Success".to_string()));
    let manager = PluginManager::new(mock, "/var/www/site");

    manager.install("akismet").unwrap();

    let commands = manager.executor.executed_commands.borrow();
    assert_eq!(commands.len(), 1);
    assert_eq!(commands[0].0, vec!["plugin", "install", "akismet"]);
    assert_eq!(commands[0].1, "/var/www/site");
}

#[test]
fn test_plugin_activate() {
    let mock = MockExecutor::new(Ok("Success".to_string()));
    let manager = PluginManager::new(mock, "/var/www/site");

    manager.activate("akismet").unwrap();

    let commands = manager.executor.executed_commands.borrow();
    assert_eq!(commands.len(), 1);
    assert_eq!(commands[0].0, vec!["plugin", "activate", "akismet"]);
}

#[test]
fn test_plugin_deactivate() {
    let mock = MockExecutor::new(Ok("Success".to_string()));
    let manager = PluginManager::new(mock, "/var/www/site");

    manager.deactivate("akismet").unwrap();

    let commands = manager.executor.executed_commands.borrow();
    assert_eq!(commands.len(), 1);
    assert_eq!(commands[0].0, vec!["plugin", "deactivate", "akismet"]);
}

#[test]
fn test_plugin_update() {
    let mock = MockExecutor::new(Ok("Success".to_string()));
    let manager = PluginManager::new(mock, "/var/www/site");

    manager.update("akismet").unwrap();

    let commands = manager.executor.executed_commands.borrow();
    assert_eq!(commands.len(), 1);
    assert_eq!(commands[0].0, vec!["plugin", "update", "akismet"]);
}

#[test]
fn test_plugin_remove() {
    let mock = MockExecutor::new(Ok("Success".to_string()));
    let manager = PluginManager::new(mock, "/var/www/site");

    manager.remove("akismet").unwrap();

    let commands = manager.executor.executed_commands.borrow();
    assert_eq!(commands.len(), 1);
    assert_eq!(commands[0].0, vec!["plugin", "uninstall", "akismet"]);
}

#[test]
fn test_theme_install() {
    let mock = MockExecutor::new(Ok("Success".to_string()));
    let manager = ThemeManager::new(mock, "/var/www/site");

    manager.install("twentytwenty").unwrap();

    let commands = manager.executor.executed_commands.borrow();
    assert_eq!(commands.len(), 1);
    assert_eq!(commands[0].0, vec!["theme", "install", "twentytwenty"]);
}

#[test]
fn test_theme_activate() {
    let mock = MockExecutor::new(Ok("Success".to_string()));
    let manager = ThemeManager::new(mock, "/var/www/site");

    manager.activate("twentytwenty").unwrap();

    let commands = manager.executor.executed_commands.borrow();
    assert_eq!(commands.len(), 1);
    assert_eq!(commands[0].0, vec!["theme", "activate", "twentytwenty"]);
}

#[test]
fn test_theme_update() {
    let mock = MockExecutor::new(Ok("Success".to_string()));
    let manager = ThemeManager::new(mock, "/var/www/site");

    manager.update("twentytwenty").unwrap();

    let commands = manager.executor.executed_commands.borrow();
    assert_eq!(commands.len(), 1);
    assert_eq!(commands[0].0, vec!["theme", "update", "twentytwenty"]);
}

#[test]
fn test_theme_remove() {
    let mock = MockExecutor::new(Ok("Success".to_string()));
    let manager = ThemeManager::new(mock, "/var/www/site");

    manager.remove("twentytwenty").unwrap();

    let commands = manager.executor.executed_commands.borrow();
    assert_eq!(commands.len(), 1);
    assert_eq!(commands[0].0, vec!["theme", "uninstall", "twentytwenty"]);
}

#[test]
fn test_invalid_slug_does_not_execute() {
    let mock = MockExecutor::new(Ok("Success".to_string()));
    let manager = PluginManager::new(mock, "/var/www/site");

    let result = manager.install("invalid slug");

    assert!(result.is_err());
    let commands = manager.executor.executed_commands.borrow();
    assert_eq!(commands.len(), 0);
}

#[test]
fn test_command_failure_is_propagated() {
    let mock = MockExecutor::new(Err(WpCliError::CommandFailed(
        "Plugin not found".to_string(),
    )));
    let manager = PluginManager::new(mock, "/var/www/site");

    let result = manager.install("akismet");

    assert!(matches!(result, Err(WpCliError::CommandFailed(_))));
}
