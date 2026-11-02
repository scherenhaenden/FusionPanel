use performance_tests::runner::run_load_test;
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[tokio::test]
async fn test_basic_load() {
    let result = run_load_test(10, 100, || async {
        // Mock task simulating some IO bounds or wait
        tokio::time::sleep(Duration::from_millis(1)).await;
        Ok(())
    })
    .await;

    assert_eq!(result.success_count, 1000);
    assert_eq!(result.error_count, 0);
    assert!(result.duration < Duration::from_secs(2));
}

#[tokio::test]
async fn test_failure_budget() {
    // A test that simulates partial failure to ensure we can budget error_count.
    // If the prerequisite API is not available, we explicitly log a BLOCKED requirement.
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
        .unwrap();

    let res = client.get("http://127.0.0.1:8080/health").send().await;

    if let Err(e) = res {
        panic!(
            "BLOCKED: Prerequisites not available for E2E. Could not reach FusionPanel API: {}",
            e
        );
    }

    let result = run_load_test(5, 20, || async {
        let n = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .subsec_nanos();
        if n % 5 == 0 {
            Err(()) // partial failure
        } else {
            Ok(())
        }
    })
    .await;

    assert_eq!(result.success_count + result.error_count, 100);
}

#[test]
fn test_system_dependency_presence() {
    // Real system dependency probe.
    let status = Command::new("docker").arg("info").output();
    if status.is_err() || !status.unwrap().status.success() {
        panic!("BLOCKED: Prerequisites not available for E2E. Docker not available.");
    }
}
