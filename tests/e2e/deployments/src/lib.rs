#![forbid(unsafe_code)]

#[cfg(test)]
mod tests {
    use std::env;
    use std::fs;
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::ops::{Deref, DerefMut};
    use std::path::PathBuf;
    use std::process::{Child, Command};
    use std::sync::mpsc::Sender;
    use std::thread;
    use std::thread::JoinHandle;
    use std::time::Duration;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
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
            Self(path)
        }
    }

    impl Deref for TempDir {
        type Target = PathBuf;

        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }

    impl AsRef<std::path::Path> for TempDir {
        fn as_ref(&self) -> &std::path::Path {
            &self.0
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    struct ChildGuard(Child);

    impl ChildGuard {
        fn new(child: Child) -> Self {
            Self(child)
        }
    }

    impl Deref for ChildGuard {
        type Target = Child;

        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }

    impl DerefMut for ChildGuard {
        fn deref_mut(&mut self) -> &mut Self::Target {
            &mut self.0
        }
    }

    impl Drop for ChildGuard {
        fn drop(&mut self) {
            if matches!(self.0.try_wait(), Ok(None)) {
                let _ = self.0.kill();
                let _ = self.0.wait();
            }
        }
    }

    struct QueryGuard<T> {
        stop: Sender<()>,
        thread: Option<JoinHandle<T>>,
    }

    impl<T> QueryGuard<T> {
        fn new(stop: Sender<()>, thread: JoinHandle<T>) -> Self {
            Self {
                stop,
                thread: Some(thread),
            }
        }

        fn finish(mut self) -> thread::Result<T> {
            let _ = self.stop.send(());
            self.thread
                .take()
                .expect("query thread already joined")
                .join()
        }
    }

    impl<T> Drop for QueryGuard<T> {
        fn drop(&mut self) {
            let _ = self.stop.send(());
            if let Some(thread) = self.thread.take() {
                let _ = thread.join();
            }
        }
    }

    fn unused_loopback_port() -> u16 {
        TcpListener::bind("127.0.0.1:0")
            .and_then(|listener| listener.local_addr())
            .expect("BLOCKED: could not allocate an isolated test port")
            .port()
    }

    fn wait_for_listener(child: &mut ChildGuard, address: &str) {
        for _ in 0..600 {
            match child.try_wait() {
                Ok(Some(status)) => {
                    panic!("BLOCKED: isolated fixture exited before listening with status {status}")
                }
                Err(error) => panic!("BLOCKED: could not inspect isolated fixture: {error}"),
                Ok(None) => {}
            }
            if TcpStream::connect(address).is_ok() {
                return;
            }
            thread::sleep(Duration::from_millis(100));
        }
        panic!("BLOCKED: isolated fixture did not listen on its assigned loopback port");
    }

    fn spawn_http_server(root: &std::path::Path) -> (ChildGuard, String) {
        let port = unused_loopback_port();
        let mut child = ChildGuard::new(
            Command::new("python3")
                .arg("-m")
                .arg("http.server")
                .arg(port.to_string())
                .arg("--bind")
                .arg("127.0.0.1")
                .current_dir(root)
                .spawn()
                .expect("BLOCKED: python3 is required for the local HTTP fixture"),
        );
        let address = format!("127.0.0.1:{port}");
        wait_for_listener(&mut child, &address);
        (child, format!("http://{address}"))
    }

    fn fetch_http_body(server_url: &str, path: &str) -> std::io::Result<String> {
        let address = server_url
            .strip_prefix("http://")
            .expect("fixture URL must be HTTP loopback");
        let mut stream = TcpStream::connect(address)?;
        stream.set_read_timeout(Some(Duration::from_secs(2)))?;
        write!(
            stream,
            "GET {path} HTTP/1.1\r\nHost: {address}\r\nConnection: close\r\n\r\n"
        )?;
        let mut response = String::new();
        stream.read_to_string(&mut response)?;
        Ok(response
            .split_once("\r\n\r\n")
            .map(|(_, body)| body.to_string())
            .unwrap_or_default())
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

    fn spawn_api() -> (ChildGuard, String) {
        let root_manifest = env::current_dir()
            .unwrap()
            .join("../../..")
            .join("Cargo.toml");
        let port = unused_loopback_port();
        let listen = format!("127.0.0.1:{port}");
        let mut child = ChildGuard::new(
            Command::new("cargo")
                .arg("run")
                .arg("--manifest-path")
                .arg(root_manifest)
                .arg("--bin")
                .arg("fusionpanel-api")
                .env("FUSIONPANEL_ADMIN_TOKEN", "test-token-123")
                .env("FUSIONPANEL_LISTEN", &listen)
                .spawn()
                .expect("BLOCKED: could not spawn isolated API fixture"),
        );
        wait_for_listener(&mut child, &listen);
        (child, format!("http://{listen}"))
    }

    #[cfg(unix)]
    #[test]
    fn local_server_resolves_current_symlink_for_each_request() {
        use std::os::unix::fs::symlink;

        let base_dir = TempDir::new();
        let target_dir = base_dir.join("app");
        let release_v1 = base_dir.join("releases/v1");
        let release_v2 = base_dir.join("releases/v2");
        fs::create_dir_all(&target_dir).unwrap();
        fs::create_dir_all(&release_v1).unwrap();
        fs::create_dir_all(&release_v2).unwrap();
        fs::write(release_v1.join("index.html"), "v1").unwrap();
        fs::write(release_v2.join("index.html"), "v2").unwrap();
        symlink(&release_v1, target_dir.join("current")).unwrap();

        let (_server, server_url) = spawn_http_server(&target_dir);
        assert_eq!(
            fetch_http_body(&server_url, "/current/index.html").unwrap(),
            "v1"
        );

        symlink(&release_v2, target_dir.join("next")).unwrap();
        fs::rename(target_dir.join("next"), target_dir.join("current")).unwrap();
        assert_eq!(
            fetch_http_body(&server_url, "/current/index.html").unwrap(),
            "v2"
        );
    }

    #[test]
    fn test_atomic_symlink_switching() {
        let base_dir = TempDir::new();
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
        let (mut server, server_url) = spawn_http_server(&target_dir);

        thread::sleep(Duration::from_secs(1));

        // Start continuous querying in a thread
        let (tx, rx) = std::sync::mpsc::channel();
        let query_thread = thread::spawn(move || {
            let mut seen_v1 = false;
            let mut seen_v2 = false;
            let mut no_downtime = true;
            loop {
                match rx.try_recv() {
                    Ok(()) | Err(std::sync::mpsc::TryRecvError::Disconnected) => break,
                    Err(std::sync::mpsc::TryRecvError::Empty) => {}
                }
                let curl = Command::new("curl")
                    .arg("--fail")
                    .arg("--silent")
                    .arg("--show-error")
                    .arg("--max-time")
                    .arg("2")
                    .arg(format!("{server_url}/current/index.html"))
                    .output();
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
        let query = QueryGuard::new(tx, query_thread);

        // Try deploying v2 via CLI
        let out_v2 = run_cli(&[
            "deploy",
            "--target",
            target_dir.to_str().unwrap(),
            "--source",
            release_v2.to_str().unwrap(),
        ]);

        let (seen_v1, seen_v2, no_downtime) = query.finish().unwrap();

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
        assert!(
            no_downtime,
            "Experienced downtime or missing version during switch"
        );

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
        let base_dir = TempDir::new();
        let target_dir = base_dir.join("app");
        let release_v1 = base_dir.join("releases").join("v1");
        let release_v2 = base_dir.join("releases").join("v2");
        fs::create_dir_all(&release_v1).unwrap();
        fs::create_dir_all(&release_v2).unwrap();
        fs::write(release_v1.join("index.html"), "v1").unwrap();
        fs::write(release_v2.join("index.html"), "v2").unwrap();

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
        let (mut api_child, api_url) = spawn_api();

        // Wait for API to start
        thread::sleep(Duration::from_secs(2));

        let payload = r#"{
            "ref": "refs/heads/main",
            "commits": [{"id": "123", "message": "test commit"}]
        }"#;
        let signature = env::var("FUSIONPANEL_TEST_WEBHOOK_SIGNATURE").unwrap_or_else(|_| {
            panic!("BLOCKED: a valid test signature for this exact payload is not configured")
        });

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
            .arg(format!("x-hmac-signature: {signature}"))
            .arg("-d")
            .arg(payload)
            .arg(format!("{api_url}/api/v1/webhooks/github"))
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

        if stdout.ends_with("200") || stdout.ends_with("202") {
            panic!("BLOCKED: webhook acceptance was returned without a deployment ID or durable status assertion");
        }
        panic!("BLOCKED: Unexpected webhook response: {}", stdout);
    }

    #[test]
    fn test_health_failure_rollback() {
        let base_dir = TempDir::new();
        let target_dir = base_dir.join("app");
        let release_v1 = base_dir.join("releases").join("v1");
        let release_bad = base_dir.join("releases").join("bad");
        fs::create_dir_all(&release_v1).unwrap();
        fs::create_dir_all(&release_bad).unwrap();
        fs::write(release_v1.join("index.html"), "v1").unwrap();
        fs::write(release_bad.join("index.html"), "bad").unwrap();

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
                "BLOCKED: CLI deploy capability unavailable for health check setup. Output: {}",
                stderr
            );
        }

        // Start python http server to serve the app
        let (mut server, server_url) = spawn_http_server(&target_dir);

        thread::sleep(Duration::from_secs(1));

        // Start continuous querying in a thread
        let (tx, rx) = std::sync::mpsc::channel();
        let query_thread = thread::spawn(move || {
            let mut seen_v1 = false;
            loop {
                match rx.try_recv() {
                    Ok(()) | Err(std::sync::mpsc::TryRecvError::Disconnected) => break,
                    Err(std::sync::mpsc::TryRecvError::Empty) => {}
                }
                let curl = Command::new("curl")
                    .arg("--fail")
                    .arg("--silent")
                    .arg("--show-error")
                    .arg("--max-time")
                    .arg("2")
                    .arg(format!("{server_url}/current/index.html"))
                    .output();
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
        let query = QueryGuard::new(tx, query_thread);

        // Deploy bad with health check enabled
        let out_bad = run_cli(&[
            "deploy",
            "--target",
            target_dir.to_str().unwrap(),
            "--source",
            release_bad.to_str().unwrap(),
            "--health-check",
        ]);
        if !out_bad.status.success() {
            let stderr = String::from_utf8_lossy(&out_bad.stderr);
            panic!(
                "BLOCKED: CLI deploy with health-check capability unavailable. Output: {}",
                stderr
            );
        }

        let seen_v1 = query.finish().unwrap();
        assert!(seen_v1, "Did not see v1 served during rollback");

        // Assert health-failure rollback: the symlink must point back to v1
        let current_symlink = target_dir.join("current");
        assert_eq!(
            fs::read_link(&current_symlink).unwrap(),
            release_v1,
            "Symlink did not rollback to v1 after health failure"
        );
        assert_eq!(
            fs::read_to_string(current_symlink.join("index.html")).unwrap(),
            "v1"
        );

        server.kill().unwrap();
        server.wait().unwrap();
        fs::remove_dir_all(&base_dir).unwrap();
    }

    #[test]
    fn test_git_webhook_bad_signature_denial() {
        let (mut api_child, api_url) = spawn_api();
        thread::sleep(Duration::from_secs(2));

        let payload = r#"{"ref": "refs/heads/main"}"#;

        // 1. Invalid signature
        let curl_invalid = Command::new("curl")
            .arg("-s")
            .arg("-w")
            .arg("%{http_code}")
            .arg("-X")
            .arg("POST")
            .arg("-H")
            .arg("Content-Type: application/json")
            .arg("-H")
            .arg("x-hmac-signature: invalid-sig")
            .arg("-d")
            .arg(payload)
            .arg(format!("{api_url}/api/v1/webhooks/github"))
            .output()
            .expect("Failed to execute curl");

        let stdout_invalid = String::from_utf8_lossy(&curl_invalid.stdout);

        if stdout_invalid.ends_with("404")
            || stdout_invalid.ends_with("000")
            || stdout_invalid.ends_with("501")
        {
            api_child.kill().unwrap();
            api_child.wait().unwrap();
            panic!("BLOCKED: Webhook endpoint not implemented.");
        }

        assert!(
            stdout_invalid.ends_with("401") || stdout_invalid.ends_with("403"),
            "Bad signature was not denied. Output: {}",
            stdout_invalid
        );

        // A syntactically plausible signature is not proof of a valid signature.
        // Require the isolated test fixture to provide one for this exact payload.
        let valid_signature = env::var("FUSIONPANEL_TEST_WEBHOOK_SIGNATURE").unwrap_or_else(|_| {
            panic!(
                "BLOCKED: no valid signature fixture is configured; replay behavior was not tested"
            )
        });
        let _curl_valid1 = Command::new("curl")
            .arg("-s")
            .arg("-w")
            .arg("%{http_code}")
            .arg("-X")
            .arg("POST")
            .arg("-H")
            .arg("Content-Type: application/json")
            .arg("-H")
            .arg(format!("x-hmac-signature: {valid_signature}"))
            .arg("-d")
            .arg(payload)
            .arg(format!("{api_url}/api/v1/webhooks/github"))
            .output()
            .expect("Failed to execute curl");
        let valid1 = String::from_utf8_lossy(&_curl_valid1.stdout);
        assert!(
            valid1.ends_with("200") || valid1.ends_with("202"),
            "The configured valid signature was not accepted; replay was not tested: {valid1}"
        );

        // Send exactly the same payload and signature again
        let curl_valid2 = Command::new("curl")
            .arg("-s")
            .arg("-w")
            .arg("%{http_code}")
            .arg("-X")
            .arg("POST")
            .arg("-H")
            .arg("Content-Type: application/json")
            .arg("-H")
            .arg(format!("x-hmac-signature: {valid_signature}"))
            .arg("-d")
            .arg(payload)
            .arg(format!("{api_url}/api/v1/webhooks/github"))
            .output()
            .expect("Failed to execute curl");

        let out2 = String::from_utf8_lossy(&curl_valid2.stdout);
        api_child.kill().unwrap();
        api_child.wait().unwrap();

        assert!(
            out2.ends_with("401") || out2.ends_with("403") || out2.ends_with("409"),
            "Replayed signature was not denied"
        );
    }

    #[test]
    fn test_deployment_second_person_approval() {
        panic!("BLOCKED: the running API has no deployment approval route or provisioned second test identity");
    }

    #[test]
    fn test_worker_interruption_recovery() {
        panic!("BLOCKED: this package can start the API binary but has no separate durable worker process to interrupt and restart");
    }

    #[test]
    fn test_stable_history_after_restart() {
        panic!("BLOCKED: the API package has no deployment history route or durable deployment store to verify across restart");
    }
}
