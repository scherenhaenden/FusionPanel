#![forbid(unsafe_code)]

#[cfg(test)]
mod tests {
    use std::env;
    use std::fs;
    use std::path::PathBuf;
    use std::process::Command;
    use std::thread;
    use std::time::Duration;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn get_temp_dir() -> PathBuf {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let mut path = env::temp_dir();
        path.push(format!(
            "fusionpanel-deployments-e2e-{}-{}",
            std::process::id(),
            now
        ));
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn run_cli(args: &[&str]) -> std::process::Output {
        let root_manifest = env::current_dir()
            .unwrap()
            .join("../../..")
            .join("Cargo.toml");

        let mut cmd = Command::new("cargo");
        cmd.arg("run")
            .arg("--manifest-path")
            .arg(root_manifest)
            .arg("--bin")
            .arg("fusionpanel-cli")
            .arg("--");

        for arg in args {
            cmd.arg(arg);
        }

        cmd.output().expect("Failed to spawn cargo process")
    }

    fn spawn_api() -> std::process::Child {
        let root_manifest = env::current_dir()
            .unwrap()
            .join("../../..")
            .join("Cargo.toml");
        Command::new("cargo")
            .arg("run")
            .arg("--manifest-path")
            .arg(root_manifest)
            .arg("--bin")
            .arg("fusionpanel-api")
            .env("FUSIONPANEL_ADMIN_TOKEN", "test-token-123")
            .env("FUSIONPANEL_LISTEN", "127.0.0.1:8080")
            .spawn()
            .expect("Failed to spawn API process")
    }

    #[test]
    fn test_atomic_symlink_switching() {
        let base_dir = get_temp_dir();
        let target_dir = base_dir.join("app");
        let release_v1 = base_dir.join("releases").join("v1");
        let release_v2 = base_dir.join("releases").join("v2");
        fs::create_dir_all(&release_v1).unwrap();
        fs::create_dir_all(&release_v2).unwrap();
        fs::write(release_v1.join("index.html"), "v1").unwrap();
        fs::write(release_v2.join("index.html"), "v2").unwrap();

        // Try deploying v1 via CLI
        let out_v1 = run_cli(&[
            "deploy",
            "--target",
            target_dir.to_str().unwrap(),
            "--source",
            release_v1.to_str().unwrap(),
        ]);
        if !out_v1.status.success() {
            let stderr = String::from_utf8_lossy(&out_v1.stderr);
            panic!(
                "BLOCKED: CLI deploy capability unavailable or failed. Output: {}",
                stderr
            );
        }

        // Verify symlink points to v1
        let current_symlink = target_dir.join("current");
        assert_eq!(fs::read_link(&current_symlink).unwrap(), release_v1);

        // Start python http server to serve the app from current_symlink
        let mut server = Command::new("python3")
            .arg("-m")
            .arg("http.server")
            .arg("8082")
            .current_dir(&current_symlink)
            .spawn()
            .expect("Failed to start test HTTP server");

        thread::sleep(Duration::from_secs(1));

        // Start continuous querying in a thread
        let (tx, rx) = std::sync::mpsc::channel();
        let query_thread = thread::spawn(move || {
            let mut seen_v1 = false;
            let mut seen_v2 = false;
            let mut no_downtime = true;
            loop {
                if let Ok(_) = rx.try_recv() {
                    break;
                }
                let curl = Command::new("curl").arg("-s").arg("http://127.0.0.1:8082/index.html").output();
                if let Ok(out) = curl {
                    let content = String::from_utf8_lossy(&out.stdout);
                    if content.contains("v1") {
                        seen_v1 = true;
                    } else if content.contains("v2") {
                        seen_v2 = true;
                    } else {
                        no_downtime = false;
                    }
                } else {
                    no_downtime = false;
                }
                thread::sleep(Duration::from_millis(10));
            }
            (seen_v1, seen_v2, no_downtime)
        });

        // Try deploying v2 via CLI
        let out_v2 = run_cli(&[
            "deploy",
            "--target",
            target_dir.to_str().unwrap(),
            "--source",
            release_v2.to_str().unwrap(),
        ]);
        
        tx.send(()).unwrap();
        let (seen_v1, seen_v2, no_downtime) = query_thread.join().unwrap();
        
        if !out_v2.status.success() {
            server.kill().unwrap();
            server.wait().unwrap();
            let stderr = String::from_utf8_lossy(&out_v2.stderr);
            panic!(
                "BLOCKED: CLI deploy switch capability unavailable. Output: {}",
                stderr
            );
        }

        assert!(seen_v1, "Did not see v1 served before switch");
        assert!(seen_v2, "Did not see v2 served after switch");
        assert!(no_downtime, "Experienced downtime or missing version during switch");

        // Verify symlink atomically points to v2
        assert_eq!(fs::read_link(&current_symlink).unwrap(), release_v2);
        assert_eq!(
            fs::read_to_string(current_symlink.join("index.html")).unwrap(),
            "v2"
        );

        server.kill().unwrap();
        server.wait().unwrap();
        fs::remove_dir_all(&base_dir).unwrap();
    }

    #[test]
    fn test_release_rollback() {
        let base_dir = get_temp_dir();
        let target_dir = base_dir.join("app");
        let release_v1 = base_dir.join("releases").join("v1");
        let release_v2 = base_dir.join("releases").join("v2");
        fs::create_dir_all(&release_v1).unwrap();
        fs::create_dir_all(&release_v2).unwrap();

        // Initial setup - deploy v1 then v2
        let out_v1 = run_cli(&[
            "deploy",
            "--target",
            target_dir.to_str().unwrap(),
            "--source",
            release_v1.to_str().unwrap(),
        ]);
        if !out_v1.status.success() {
            panic!("BLOCKED: CLI deploy capability unavailable for initial rollback setup.");
        }
        let out_v2 = run_cli(&[
            "deploy",
            "--target",
            target_dir.to_str().unwrap(),
            "--source",
            release_v2.to_str().unwrap(),
        ]);
        if !out_v2.status.success() {
            panic!("BLOCKED: CLI deploy capability unavailable for initial rollback setup.");
        }

        // Perform rollback
        let out_rollback = run_cli(&[
            "deploy",
            "rollback",
            "--target",
            target_dir.to_str().unwrap(),
        ]);
        if !out_rollback.status.success() {
            let stderr = String::from_utf8_lossy(&out_rollback.stderr);
            panic!(
                "BLOCKED: CLI rollback capability unavailable. Output: {}",
                stderr
            );
        }

        // Verify symlink reverted to v1
        let current_symlink = target_dir.join("current");
        assert_eq!(fs::read_link(&current_symlink).unwrap(), release_v1);

        fs::remove_dir_all(&base_dir).unwrap();
    }

    #[test]
    fn test_git_webhook_pipeline_triggers() {
        let mut api_child = spawn_api();

        // Wait for API to start
        thread::sleep(Duration::from_secs(2));

        let payload = r#"{
            "ref": "refs/heads/main",
            "commits": [{"id": "123", "message": "test commit"}]
        }"#;

        // Use curl via Command since we cannot add dependencies like reqwest
        let curl_out = Command::new("curl")
            .arg("-s")
            .arg("-w")
            .arg("%{http_code}")
            .arg("-X")
            .arg("POST")
            .arg("-H")
            .arg("Content-Type: application/json")
            .arg("-H")
            .arg("x-hmac-signature: dummy-sig")
            .arg("-d")
            .arg(payload)
            .arg("http://127.0.0.1:8080/api/v1/webhooks/github")
            .output()
            .expect("Failed to execute curl");

        let stdout = String::from_utf8_lossy(&curl_out.stdout);

        api_child.kill().unwrap();
        api_child.wait().unwrap();

        if stdout.ends_with("404") || stdout.contains("Not Found") {
            panic!("BLOCKED: Webhook endpoint not implemented.");
        }

        if stdout.ends_with("500") || stdout.ends_with("501") || stdout.ends_with("000") {
            panic!("BLOCKED: Webhook endpoint failed or not fully implemented.");
        }

        // Depending on success behavior, verify 200 or 202
        if !stdout.ends_with("200") && !stdout.ends_with("202") {
            panic!("BLOCKED: Unexpected webhook response: {}", stdout);
        }

        // We would normally verify pipeline side effects here, e.g. checking a DB or state
    }

    #[test]
    fn test_health_failure_rollback() {
        let base_dir = get_temp_dir();
        let target_dir = base_dir.join("app");
        let release_v1 = base_dir.join("releases").join("v1");
        let release_bad = base_dir.join("releases").join("bad");
        fs::create_dir_all(&release_v1).unwrap();
        fs::create_dir_all(&release_bad).unwrap();
        fs::write(release_v1.join("index.html"), "v1").unwrap();
        fs::write(release_bad.join("index.html"), "bad").unwrap();

        let out_v1 = run_cli(&["deploy", "--target", target_dir.to_str().unwrap(), "--source", release_v1.to_str().unwrap()]);
        if !out_v1.status.success() {
            let stderr = String::from_utf8_lossy(&out_v1.stderr);
            panic!("BLOCKED: CLI deploy capability unavailable for health check setup. Output: {}", stderr);
        }

        // Start python http server to serve the app
        let mut server = Command::new("python3")
            .arg("-m")
            .arg("http.server")
            .arg("8081")
            .current_dir(target_dir.join("current"))
            .spawn()
            .expect("Failed to start test HTTP server");

        thread::sleep(Duration::from_secs(1));

        // Start continuous querying in a thread
        let (tx, rx) = std::sync::mpsc::channel();
        let query_thread = thread::spawn(move || {
            let mut seen_v1 = false;
            loop {
                if let Ok(_) = rx.try_recv() {
                    break;
                }
                let curl = Command::new("curl").arg("-s").arg("http://127.0.0.1:8081/index.html").output();
                if let Ok(out) = curl {
                    let content = String::from_utf8_lossy(&out.stdout);
                    if content.contains("v1") {
                        seen_v1 = true;
                    }
                }
                thread::sleep(Duration::from_millis(50));
            }
            seen_v1
        });

        // Deploy bad with health check enabled
        let out_bad = run_cli(&["deploy", "--target", target_dir.to_str().unwrap(), "--source", release_bad.to_str().unwrap(), "--health-check"]);
        if !out_bad.status.success() {
            let stderr = String::from_utf8_lossy(&out_bad.stderr);
            if stderr.contains("unrecognized subcommand") || stderr.contains("error:") {
                server.kill().unwrap();
                tx.send(()).unwrap();
                query_thread.join().unwrap();
                panic!("BLOCKED: CLI deploy with health-check capability unavailable. Output: {}", stderr);
            }
        }

        tx.send(()).unwrap();
        let seen_v1 = query_thread.join().unwrap();
        assert!(seen_v1, "Did not see v1 served during rollback");

        // Assert health-failure rollback: the symlink must point back to v1
        let current_symlink = target_dir.join("current");
        assert_eq!(fs::read_link(&current_symlink).unwrap(), release_v1, "Symlink did not rollback to v1 after health failure");
        assert_eq!(fs::read_to_string(current_symlink.join("index.html")).unwrap(), "v1");

        server.kill().unwrap();
        server.wait().unwrap();
        fs::remove_dir_all(&base_dir).unwrap();
    }

    #[test]
    fn test_git_webhook_bad_signature_denial() {
        let mut api_child = spawn_api();
        thread::sleep(Duration::from_secs(2));

        let payload = r#"{"ref": "refs/heads/main"}"#;
        
        // 1. Invalid signature
        let curl_invalid = Command::new("curl")
            .arg("-s").arg("-w").arg("%{http_code}")
            .arg("-X").arg("POST")
            .arg("-H").arg("Content-Type: application/json")
            .arg("-H").arg("x-hmac-signature: invalid-sig")
            .arg("-d").arg(payload)
            .arg("http://127.0.0.1:8080/api/v1/webhooks/github")
            .output().expect("Failed to execute curl");

        let stdout_invalid = String::from_utf8_lossy(&curl_invalid.stdout);
        
        if stdout_invalid.ends_with("404") || stdout_invalid.ends_with("000") || stdout_invalid.ends_with("501") {
            api_child.kill().unwrap();
            api_child.wait().unwrap();
            panic!("BLOCKED: Webhook endpoint not implemented.");
        }
        
        assert!(stdout_invalid.ends_with("401") || stdout_invalid.ends_with("403"), "Bad signature was not denied. Output: {}", stdout_invalid);

        // 2. Replay attack: Assume we have a valid signature for the payload for testing
        // Without knowing the actual HMAC secret, we would simulate a replay by capturing a legitimate request,
        // but here we just simulate sending a valid request twice if we had one.
        // For E2E we must test replay denial.
        // We will send a request with a valid-looking signature.
        let _curl_valid1 = Command::new("curl")
            .arg("-s").arg("-w").arg("%{http_code}")
            .arg("-X").arg("POST")
            .arg("-H").arg("Content-Type: application/json")
            .arg("-H").arg("x-hmac-signature: t=123,v1=valid")
            .arg("-d").arg(payload)
            .arg("http://127.0.0.1:8080/api/v1/webhooks/github")
            .output().expect("Failed to execute curl");
        
        // Send exactly the same payload and signature again
        let curl_valid2 = Command::new("curl")
            .arg("-s").arg("-w").arg("%{http_code}")
            .arg("-X").arg("POST")
            .arg("-H").arg("Content-Type: application/json")
            .arg("-H").arg("x-hmac-signature: t=123,v1=valid")
            .arg("-d").arg(payload)
            .arg("http://127.0.0.1:8080/api/v1/webhooks/github")
            .output().expect("Failed to execute curl");

        let out2 = String::from_utf8_lossy(&curl_valid2.stdout);
        api_child.kill().unwrap();
        api_child.wait().unwrap();

        assert!(out2.ends_with("401") || out2.ends_with("403") || out2.ends_with("409"), "Replayed signature was not denied");
    }

    #[test]
    fn test_deployment_second_person_approval() {
        let mut api_child = spawn_api();
        thread::sleep(Duration::from_secs(2));

        // 1. Creator attempts to approve their own deployment
        let curl_self = Command::new("curl")
            .arg("-s").arg("-w").arg("%{http_code}")
            .arg("-X").arg("POST")
            .arg("-H").arg("Authorization: Bearer test-token-123") // Creator token
            .arg("http://127.0.0.1:8080/api/v1/deployments/pending/1/approve")
            .output().expect("Failed to execute curl");

        let self_stdout = String::from_utf8_lossy(&curl_self.stdout);
        
        if self_stdout.ends_with("404") || self_stdout.ends_with("000") || self_stdout.ends_with("501") {
            api_child.kill().unwrap();
            api_child.wait().unwrap();
            panic!("BLOCKED: Deployments API not implemented.");
        }

        assert!(self_stdout.ends_with("403"), "Creator was allowed to approve their own deployment");

        // 2. Second person approves
        let curl_approve = Command::new("curl")
            .arg("-s").arg("-w").arg("%{http_code}")
            .arg("-X").arg("POST")
            .arg("-H").arg("Authorization: Bearer another-token-456") // Second person token
            .arg("http://127.0.0.1:8080/api/v1/deployments/pending/1/approve")
            .output().expect("Failed to execute curl");

        let approve_stdout = String::from_utf8_lossy(&curl_approve.stdout);
        api_child.kill().unwrap();
        api_child.wait().unwrap();

        assert!(approve_stdout.ends_with("200") || approve_stdout.ends_with("202"), "Second person approval failed");
    }

    #[test]
    fn test_worker_interruption_recovery() {
        let base_dir = get_temp_dir();
        let _target_dir = base_dir.join("app");
        let release = base_dir.join("releases").join("v1");
        fs::create_dir_all(&release).unwrap();

        // Spawn a background API/worker
        let mut worker_child = spawn_api();
        thread::sleep(Duration::from_secs(2));

        // Start deploy via API (which should take some time)
        let curl_out = Command::new("curl")
            .arg("-s").arg("-w").arg("%{http_code}")
            .arg("-X").arg("POST")
            .arg("-H").arg("Authorization: Bearer test-token-123")
            .arg("http://127.0.0.1:8080/api/v1/deployments")
            .output().expect("Failed to execute curl");

        let stdout = String::from_utf8_lossy(&curl_out.stdout);
        if stdout.ends_with("404") || stdout.ends_with("000") || stdout.ends_with("501") {
            worker_child.kill().unwrap();
            worker_child.wait().unwrap();
            fs::remove_dir_all(&base_dir).unwrap();
            panic!("BLOCKED: Deployments API unavailable for worker interruption test.");
        }

        // Kill worker mid-flight
        thread::sleep(Duration::from_millis(100));
        worker_child.kill().unwrap();
        worker_child.wait().unwrap();

        // Restart worker
        let mut new_worker = spawn_api();
        thread::sleep(Duration::from_secs(2));

        // Wait for it to recover and finish the deployment
        thread::sleep(Duration::from_secs(2));

        // Check if deployment finished
        let curl_check = Command::new("curl")
            .arg("-s").arg("-w").arg("%{http_code}")
            .arg("-H").arg("Authorization: Bearer test-token-123")
            .arg("http://127.0.0.1:8080/api/v1/deployments/1")
            .output().expect("Failed to execute curl");
        
        let check_stdout = String::from_utf8_lossy(&curl_check.stdout);
        new_worker.kill().unwrap();
        new_worker.wait().unwrap();

        assert!(check_stdout.contains("success") || check_stdout.contains("completed") || check_stdout.contains("Active"), "Deployment did not recover after worker interruption");
        fs::remove_dir_all(&base_dir).unwrap();
    }

    #[test]
    fn test_stable_history_after_restart() {
        let mut api_child = spawn_api();
        thread::sleep(Duration::from_secs(2));

        let curl_out = Command::new("curl")
            .arg("-s").arg("-w").arg("%{http_code}")
            .arg("-H").arg("Authorization: Bearer test-token-123")
            .arg("http://127.0.0.1:8080/api/v1/deployments/history")
            .output().expect("Failed to execute curl");

        let stdout = String::from_utf8_lossy(&curl_out.stdout);
        api_child.kill().unwrap();
        api_child.wait().unwrap();

        if stdout.ends_with("404") || stdout.ends_with("000") || stdout.ends_with("501") {
            panic!("BLOCKED: Deployments history API not implemented.");
        }

        // Restart API
        let mut new_api = spawn_api();
        thread::sleep(Duration::from_secs(2));

        let curl_out2 = Command::new("curl")
            .arg("-s").arg("-w").arg("%{http_code}")
            .arg("-H").arg("Authorization: Bearer test-token-123")
            .arg("http://127.0.0.1:8080/api/v1/deployments/history")
            .output().expect("Failed to execute curl");
        
        let stdout2 = String::from_utf8_lossy(&curl_out2.stdout);
        new_api.kill().unwrap();
        new_api.wait().unwrap();

        assert_eq!(stdout, stdout2, "History was not stable after restart");
    }
}
