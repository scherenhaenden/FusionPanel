#[path = "../src/executor.rs"]
mod executor;

use executor::{CommandExecutor, CommandRequest};
use std::collections::HashMap;
use std::time::Duration;

#[tokio::test]
async fn test_execute_success() {
    let req = CommandRequest {
        cmd: "echo".to_string(),
        args: vec!["hello".to_string()],
        envs: HashMap::new(),
        timeout: Duration::from_secs(5),
    };

    let result = CommandExecutor::execute(&req).await.unwrap();
    assert_eq!(result.exit_code, Some(0));
    assert_eq!(String::from_utf8_lossy(&result.stdout).trim(), "hello");
    assert_eq!(result.timeout, false);
}

#[tokio::test]
async fn test_execute_timeout() {
    let req = CommandRequest {
        cmd: "sleep".to_string(),
        args: vec!["5".to_string()],
        envs: HashMap::new(),
        timeout: Duration::from_millis(100),
    };

    let result = CommandExecutor::execute(&req).await.unwrap();
    assert_eq!(result.timeout, true);
    assert_eq!(result.exit_code, None);
}

#[tokio::test]
async fn test_output_capping() {
    // Generate output that exceeds 1MB.
    // 'dd' command is available on unix systems.
    let req = CommandRequest {
        cmd: "dd".to_string(),
        args: vec![
            "if=/dev/zero".to_string(),
            "bs=2M".to_string(),
            "count=1".to_string(),
        ],
        envs: HashMap::new(),
        timeout: Duration::from_secs(5),
    };

    let result = CommandExecutor::execute(&req).await.unwrap();

    // We expect it to cap at exactly 1024*1024 bytes (1MB)
    assert!(result.stdout.len() <= 1024 * 1024);
}

#[tokio::test]
async fn test_env_redaction() {
    let mut envs = HashMap::new();
    envs.insert("SECRET_KEY".to_string(), "supersecret".to_string());
    envs.insert("PUBLIC_KEY".to_string(), "publicinfo".to_string());

    let redacted = CommandExecutor::redact_env(&envs, &["SECRET_KEY"]);

    assert_eq!(redacted.get("SECRET_KEY").unwrap(), "[REDACTED]");
    assert_eq!(redacted.get("PUBLIC_KEY").unwrap(), "publicinfo");
}
