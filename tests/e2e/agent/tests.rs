use agent::daemon::{AgentDaemon, HmacWsTransport};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::net::TcpListener;
use tokio::sync::Mutex;
use tokio::time::sleep;

#[tokio::test]
async fn test_agent_reconnect_loop_with_hmac_and_command_streaming() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let attempts = Arc::new(Mutex::new(Vec::new()));
    let attempts_clone = attempts.clone();

    tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            let mut attempts = attempts_clone.lock().await;
            attempts.push(Instant::now());
            // Intentionally drop the stream to simulate transport failure
            drop(stream);
        }
    });

    let url = format!("ws://{}/ws", addr);
    let transport = HmacWsTransport::new(url, vec![1, 2, 3, 4]);

    let initial_delay = Duration::from_millis(50);
    let max_delay = Duration::from_millis(400);

    let daemon =
        AgentDaemon::new(transport, "1.0.0".to_string()).with_delays(initial_delay, max_delay);

    let handle = tokio::spawn(async move {
        daemon.run().await;
    });

    // Let it try a few times: 50 + 100 + 200 + 400 = 750ms total
    sleep(Duration::from_millis(850)).await;
    handle.abort();

    let attempts = attempts.lock().await;
    assert!(
        attempts.len() >= 4,
        "Expected at least 4 connection attempts"
    );

    let mut delays = Vec::new();
    for i in 1..attempts.len() {
        delays.push(attempts[i].duration_since(attempts[i - 1]).as_millis());
    }

    // Checking if backoff doubles roughly
    assert!(
        delays[0] >= 40 && delays[0] <= 90,
        "Expected first delay around 50ms, got {}",
        delays[0]
    );
    assert!(
        delays[1] >= 90 && delays[1] <= 140,
        "Expected second delay around 100ms, got {}",
        delays[1]
    );
    assert!(
        delays[2] >= 180 && delays[2] <= 260,
        "Expected third delay around 200ms, got {}",
        delays[2]
    );
    if delays.len() > 3 {
        assert!(
            delays[3] >= 380 && delays[3] <= 460,
            "Expected fourth delay around 400ms, got {}",
            delays[3]
        );
    }
}

#[tokio::test]
async fn test_hmac_authentication_challenge() {
    // Tests MUST explicitly record 'BLOCKED' instead of passing if required dependencies or prerequisites are unavailable.
    // The current Agent daemon does not implement challenge-response authentication.

    // Example intended API (pseudo-code)
    // let mut client = ws_connect("ws://localhost/auth").await;
    // let challenge = client.recv().await;
    // let response = sign_challenge(&challenge, "secret");
    // client.send(response).await;
    // assert_eq!(client.recv().await, Ok("Authenticated"));

    panic!(
        "BLOCKED: Agent daemon currently lacks interactive HMAC challenge-response capabilities."
    );
}

#[tokio::test]
async fn test_command_execution_streaming() {
    // Tests MUST explicitly record 'BLOCKED' instead of passing if required dependencies or prerequisites are unavailable.
    // The current Agent daemon does not implement `system.exec` and execution output streaming.

    // Example intended API (pseudo-code)
    // let mut client = ws_connect("ws://localhost/exec").await;
    // client.send(SystemExec { command: "echo test" }).await;
    // let mut output = String::new();
    // while let Some(chunk) = client.recv_stream().await { output.push_str(&chunk); }
    // assert_eq!(output, "test\n");

    panic!("BLOCKED: Agent daemon currently lacks system.exec capabilities and execution stream upgrades.");
}
