#![forbid(unsafe_code)]

use std::path::Path;
use tokio_test::block_on;
use wordpress::wp_cli::{commands::WpCli, WpCliExecutor};

#[test]
fn test_executor_success() {
    block_on(async {
        let executor = WpCliExecutor::new("echo");
        let output = executor.run_command(&["hello world"], None).await.unwrap();
        assert_eq!(output.trim(), "hello world");
    });
}

#[test]
fn test_executor_failure() {
    block_on(async {
        let executor = WpCliExecutor::new("false");
        let result = executor.run_command(&[], None).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            wordpress::wp_cli::WpCliError::CommandFailed(code, _) => {
                assert_eq!(code, 1);
            }
            e => panic!("Expected CommandFailed error, got {:?}", e),
        }
    });
}

#[test]
fn test_executor_timeout() {
    block_on(async {
        let executor =
            WpCliExecutor::new("sleep").with_timeout(std::time::Duration::from_millis(10));
        let result = executor.run_command(&["1"], None).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            wordpress::wp_cli::WpCliError::Timeout(_) => {}
            e => panic!("Expected Timeout error, got {:?}", e),
        }
    });
}

#[test]
fn test_wp_cli_command_builder_json_parsing() {
    block_on(async {
        let script = r#"#!/usr/bin/env sh
found=0
for arg in "$@"; do
    if [ "$arg" = "--format=json" ]; then
        found=1
    fi
done
if [ "$found" -eq 1 ]; then
    echo '[{"name":"akismet","status":"active","update":"none","version":"5.0"}]'
else
    echo "missing format"
fi
"#;

        let temp_dir = std::env::temp_dir();
        let script_path = temp_dir.join("dummy_wp_json.sh");
        std::fs::write(&script_path, script).unwrap();
        std::process::Command::new("chmod")
            .arg("+x")
            .arg(&script_path)
            .status()
            .unwrap();

        let executor = WpCliExecutor::new(&script_path);
        let cli = WpCli::new(executor);

        let site_path = Path::new("/var/www/site");
        let plugins = cli
            .run_json::<Vec<wordpress::wp_cli::commands::WpPlugin>>(&[], Some(site_path))
            .await
            .unwrap();

        assert_eq!(plugins.len(), 1);
        assert_eq!(plugins[0].name, "akismet");
        assert_eq!(plugins[0].status, "active");

        std::fs::remove_file(script_path).unwrap();
    });
}

#[test]
fn test_is_installed_parsing() {
    block_on(async {
        let executor = WpCliExecutor::new("true");
        let cli = WpCli::new(executor);

        let is_installed = cli.is_installed(Path::new("/dummy")).await.unwrap();
        assert!(is_installed);
    });
}

#[tokio::test]
async fn test_sanitize_arg_blocks_injection() {
    assert!(WpCli::sanitize_arg("--allow-root").is_err());
    
    assert!(WpCli::sanitize_arg("valid_argument123").is_ok());
}
