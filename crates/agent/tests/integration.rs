use agent::executor::{CommandExecutor, CommandRequest};
use agent::files::FileManager;
use agent::protocol::{AgentError, AgentRequest, AgentResponse};
use serde_json::json;
use std::fs;
use std::process::Stdio;
use std::time::Duration;
use tempfile::NamedTempFile;
use tokio::time::sleep;

#[tokio::test]
async fn test_agent_protocol_integration() {
    let req = AgentRequest {
        version: 1,
        request_id: "test-id".to_string(),
        operation: "system.info".to_string(),
        payload: json!({}),
    };
    assert_eq!(req.version, 1);

    let resp = AgentResponse {
        version: 1,
        request_id: req.request_id.clone(),
        result: Some(json!({"ok": true})),
        error: None,
    };

    assert!(resp.result.is_some());

    let err = AgentError {
        code: "test".to_string(),
        message: "msg".to_string(),
    };

    assert_eq!(err.code, "test");
}

#[tokio::test]
async fn test_agent_executor_real_ops() {
    let req = CommandRequest {
        cmd: "echo".to_string(),
        args: vec!["hello".to_string()],
        envs: std::collections::HashMap::new(),
        timeout: Duration::from_secs(2),
    };
    let res = CommandExecutor::execute(&req).await.unwrap();
    assert_eq!(res.exit_code, Some(0));
    assert_eq!(String::from_utf8_lossy(&res.stdout).trim(), "hello");
}

#[tokio::test]
async fn test_agent_files_real_ops() {
    let fm = FileManager::new();
    let temp = NamedTempFile::new().unwrap();
    let path = temp.path();
    fm.write_file_atomic(path, b"test-data", None, None, None)
        .unwrap();
    let content = fs::read_to_string(path).unwrap();
    assert_eq!(content, "test-data");
}

#[tokio::test]
async fn test_agent_child_process_integration() {
    let mut cmd = tokio::process::Command::new("cargo");
    cmd.arg("run")
        .arg("--bin")
        .arg("fusionpanel-agent")
        .env("FUSIONPANEL_AGENT_URL", "ws://127.0.0.1:0") // Port 0 to intentionally fail connection quickly, testing reconnect backoff
        .env("FUSIONPANEL_AGENT_HEARTBEAT_URL", "ws://127.0.0.1:0")
        .env("FUSIONPANEL_AGENT_SECRET", "test-secret")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    let mut child = cmd.spawn().expect("failed to start agent process");

    sleep(Duration::from_millis(1500)).await;

    child.kill().await.expect("failed to kill agent");
    let output = child.wait_with_output().await.expect("failed to wait");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Protocol loop error"),
        "Agent should attempt to connect and log an error"
    );
}

// Dummy server to verify handshake
#[tokio::test]
async fn test_agent_child_process_handshake() {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    use tokio::net::TcpListener;
    use tokio_tungstenite::accept_hdr_async;
    use tokio_tungstenite::tungstenite::handshake::server::{Request, Response};

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();

    let url = format!("ws://127.0.0.1:{}", port);

    let mut cmd = tokio::process::Command::new("cargo");
    cmd.arg("run")
        .arg("--bin")
        .arg("fusionpanel-agent")
        .env("FUSIONPANEL_AGENT_URL", url.clone())
        .env("FUSIONPANEL_AGENT_HEARTBEAT_URL", "ws://127.0.0.1:0")
        .env("FUSIONPANEL_AGENT_SECRET", "test-secret")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    let mut child = cmd.spawn().expect("failed to start agent process");

    let (stream, _) = listener.accept().await.unwrap();

    let callback = |req: &Request, mut response: Response| {
        let _sig = req.headers().get("x-hmac-signature").unwrap();
        let ts = req.headers().get("x-timestamp").unwrap();

        let mut mac = Hmac::<Sha256>::new_from_slice(b"test-secret").unwrap();
        mac.update(b"fusionpanel-control-plane-accept:");
        mac.update(ts.as_bytes());
        let response_sig = hex::encode(mac.finalize().into_bytes());

        response
            .headers_mut()
            .insert("x-hmac-signature", response_sig.parse().unwrap());
        response.headers_mut().insert("x-timestamp", ts.clone());

        Ok(response)
    };

    let mut ws = accept_hdr_async(stream, callback).await.unwrap();

    // We send an executor request that is ALLOWED
    let req = AgentRequest {
        version: 1,
        request_id: "test-req".to_string(),
        operation: "executor.run".to_string(),
        payload: json!({"cmd": "echo", "args": ["hello-executor"]}),
    };

    use futures_util::SinkExt;
    ws.send(tokio_tungstenite::tungstenite::Message::Text(
        serde_json::to_string(&req).unwrap(),
    ))
    .await
    .unwrap();

    use futures_util::StreamExt;
    let resp = ws.next().await.unwrap().unwrap();
    if let tokio_tungstenite::tungstenite::Message::Text(t) = resp {
        let resp: AgentResponse = serde_json::from_str(&t).unwrap();
        assert!(resp.result.is_some());
        let res_json = resp.result.unwrap();
        let out = res_json.get("stdout").unwrap().as_str().unwrap();
        assert!(out.contains("hello-executor"));
    }

    // We send an executor request that is BLOCKED
    let req_blocked = AgentRequest {
        version: 1,
        request_id: "test-req-blocked".to_string(),
        operation: "executor.run".to_string(),
        payload: json!({"cmd": "cat", "args": ["/etc/shadow"]}),
    };

    ws.send(tokio_tungstenite::tungstenite::Message::Text(
        serde_json::to_string(&req_blocked).unwrap(),
    ))
    .await
    .unwrap();

    let resp = ws.next().await.unwrap().unwrap();
    if let tokio_tungstenite::tungstenite::Message::Text(t) = resp {
        let resp: AgentResponse = serde_json::from_str(&t).unwrap();
        assert!(resp.error.is_some());
        let err = resp.error.unwrap();
        assert_eq!(err.code, "operation_blocked");
    }

    child.kill().await.unwrap();
}
