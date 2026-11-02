use deployments::builder::executor::{ExecutorError, StepConfig, StepExecutor};
use std::collections::HashMap;
use std::env;

#[tokio::test]
async fn test_successful_execution() {
    let config = StepConfig {
        command: "echo".to_string(),
        args: vec!["hello world".to_string()],
        env: HashMap::new(),
        working_dir: env::current_dir().unwrap(),
        timeout_seconds: 5,
    };

    let result = StepExecutor::execute(&config).await.unwrap();
    assert_eq!(result.exit_code, 0);
    assert!(result.logs.contains("hello world"));
}

#[tokio::test]
async fn test_timeout_enforcement() {
    let config = StepConfig {
        command: "sleep".to_string(),
        args: vec!["5".to_string()],
        env: HashMap::new(),
        working_dir: env::current_dir().unwrap(),
        timeout_seconds: 1, // Will timeout before sleep finishes
    };

    let result = StepExecutor::execute(&config).await;
    match result {
        Err(ExecutorError::Timeout(1, _logs)) => (), // Expected
        _ => panic!("Expected timeout error, got {:?}", result),
    }
}

#[tokio::test]
async fn test_environment_variable_injection() {
    let mut env = HashMap::new();
    env.insert("FUSION_TEST_VAR".to_string(), "fusion_value".to_string());

    // We use bash to echo the variable since printenv isnt cross-platform guaranteed
    // though printenv would work in linux. bash -c "echo $FUSION_TEST_VAR" is reliable.
    let config = StepConfig {
        command: "bash".to_string(),
        args: vec!["-c".to_string(), "echo $FUSION_TEST_VAR".to_string()],
        env,
        working_dir: env::current_dir().unwrap(),
        timeout_seconds: 5,
    };

    let result = StepExecutor::execute(&config).await.unwrap();
    assert_eq!(result.exit_code, 0);
    assert!(result.logs.contains("fusion_value"));
}

#[tokio::test]
async fn test_working_directory_isolation() {
    let tmp_dir = tempfile::tempdir().unwrap();
    let tmp_path = tmp_dir.path().to_path_buf();

    // Create a file in the temp dir
    let file_path = tmp_path.join("marker.txt");
    std::fs::write(&file_path, "isolated").unwrap();

    let config = StepConfig {
        command: "cat".to_string(),
        args: vec!["marker.txt".to_string()],
        env: HashMap::new(),
        working_dir: tmp_path.clone(),
        timeout_seconds: 5,
    };

    let result = StepExecutor::execute(&config).await.unwrap();
    assert_eq!(result.exit_code, 0);
    assert!(result.logs.contains("isolated"));
}

#[tokio::test]
async fn test_negative_non_zero_exit() {
    let config = StepConfig {
        command: "bash".to_string(),
        args: vec!["-c".to_string(), "echo failing; exit 42".to_string()],
        env: HashMap::new(),
        working_dir: env::current_dir().unwrap(),
        timeout_seconds: 5,
    };

    let result = StepExecutor::execute(&config).await;
    match result {
        Ok(result) => {
            assert_eq!(result.exit_code, 42);
            assert!(result.logs.contains("failing"));
        } // Expected
        _ => panic!("Expected ok with 42 exit, got {:?}", result),
    }
}
