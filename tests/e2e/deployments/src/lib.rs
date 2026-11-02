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
        assert_eq!(
            fs::read_to_string(current_symlink.join("index.html")).unwrap(),
            "v1"
        );

        // Try deploying v2 via CLI
        let out_v2 = run_cli(&[
            "deploy",
            "--target",
            target_dir.to_str().unwrap(),
            "--source",
            release_v2.to_str().unwrap(),
        ]);
        if !out_v2.status.success() {
            let stderr = String::from_utf8_lossy(&out_v2.stderr);
            panic!(
                "BLOCKED: CLI deploy switch capability unavailable. Output: {}",
                stderr
            );
        }

        // Verify symlink atomically points to v2
        assert_eq!(fs::read_link(&current_symlink).unwrap(), release_v2);
        assert_eq!(
            fs::read_to_string(current_symlink.join("index.html")).unwrap(),
            "v2"
        );

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
}
