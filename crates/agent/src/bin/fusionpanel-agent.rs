#![forbid(unsafe_code)]

use agent::daemon::{AgentDaemon, HmacWsTransport};
use agent::executor::{CommandExecutor, CommandRequest};
use agent::metrics::MetricsCollector;
use agent::protocol::{AgentError, AgentRequest, AgentResponse};
use futures_util::{SinkExt, StreamExt};
use hmac::{Hmac, Mac};
use serde_json::json;
use sha2::Sha256;
use std::collections::HashMap;
use std::env;
use std::time::Duration;
use tokio::time::sleep;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::Message;

#[tokio::main]
async fn main() {
    let url = env::var("FUSIONPANEL_AGENT_URL")
        .unwrap_or_else(|_| "ws://127.0.0.1:8080/agent".to_string());
    let secret =
        env::var("FUSIONPANEL_AGENT_SECRET").unwrap_or_else(|_| "default-secret".to_string());

    // Start heartbeat loop (daemon) on the same URL for now, or assume it's separated by logical endpoints
    // For this requirement we just need the agent process to use both transport features.
    let h_url = env::var("FUSIONPANEL_AGENT_HEARTBEAT_URL")
        .unwrap_or_else(|_| "ws://127.0.0.1:8080/heartbeat".to_string());
    let transport = HmacWsTransport::new(h_url, secret.as_bytes().to_vec());
    let daemon = AgentDaemon::new(transport, env!("CARGO_PKG_VERSION").to_string());

    tokio::spawn(async move {
        daemon.run().await;
    });

    loop {
        if let Err(e) = run_protocol_loop(&url, secret.as_bytes()).await {
            eprintln!("Protocol loop error: {:?}. Reconnecting...", e);
            sleep(Duration::from_secs(5)).await;
        }
    }
}

async fn run_protocol_loop(
    url: &str,
    secret: &[u8],
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut request = url.to_string().into_client_request()?;

    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs()
        .to_string();

    type HmacSha256 = Hmac<Sha256>;
    let mut mac = HmacSha256::new_from_slice(secret)?;
    mac.update(b"fusionpanel-agent-connect:");
    mac.update(timestamp.as_bytes());
    let signature = hex::encode(mac.finalize().into_bytes());
    request
        .headers_mut()
        .insert("x-hmac-signature", signature.parse()?);
    request
        .headers_mut()
        .insert("x-timestamp", timestamp.parse()?);

    let (mut ws_stream, response) = connect_async(request).await?;

    if let (Some(server_sig), Some(server_ts)) = (
        response.headers().get("x-hmac-signature"),
        response.headers().get("x-timestamp"),
    ) {
        let mut verify_mac = HmacSha256::new_from_slice(secret)?;
        verify_mac.update(b"fusionpanel-control-plane-accept:");
        verify_mac.update(server_ts.as_bytes());
        let expected_sig = hex::encode(verify_mac.finalize().into_bytes());
        if server_sig.to_str().unwrap_or("") != expected_sig {
            return Err("Server authentication failed (invalid signature)".into());
        }

        let s_ts: u64 = server_ts.to_str().unwrap_or("0").parse()?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs();
        if now > s_ts + 300 || s_ts > now + 300 {
            return Err("Server authentication failed (replay detected)".into());
        }
    } else {
        return Err("Missing server signature or timestamp".into());
    }

    let mut metrics_collector = MetricsCollector::default();

    while let Some(msg) = ws_stream.next().await {
        let msg = msg?;
        if let Message::Text(text) = msg {
            let req: AgentRequest = match serde_json::from_str(&text) {
                Ok(r) => r,
                Err(_) => continue,
            };

            let mut response = AgentResponse {
                version: req.version,
                request_id: req.request_id.clone(),
                result: None,
                error: None,
            };

            if req.version != 1 {
                response.error = Some(AgentError {
                    code: "unsupported_version".to_string(),
                    message: "Unsupported protocol version".to_string(),
                });
            } else {
                match req.operation.as_str() {
                    "system.info" => {
                        if let Ok(metrics) = metrics_collector.collect() {
                            response.result = Some(json!({
                                "cpu_usage_percent": metrics.cpu_usage_percent,
                                "memory_used_kb": metrics.memory_used_kb,
                                "memory_available_kb": metrics.memory_available_kb,
                                "disk_io_ops": metrics.disk_io_ops,
                            }));
                        } else {
                            response.error = Some(AgentError {
                                code: "metrics_failed".to_string(),
                                message: "Failed to collect system metrics".to_string(),
                            });
                        }
                    }
                    "site.validate" => {
                        if let Some(_domain) = req.payload.get("domain").and_then(|v| v.as_str()) {
                            if let Some(doc_root) =
                                req.payload.get("document_root").and_then(|v| v.as_str())
                            {
                                if doc_root.starts_with("/srv/fusionpanel/sites/")
                                    && !doc_root.contains("..")
                                    && !doc_root.contains('\0')
                                {
                                    response.result = Some(json!({
                                        "valid": true
                                    }));
                                } else {
                                    response.error = Some(AgentError {
                                        code: "invalid_path".to_string(),
                                        message: "Invalid document root".to_string(),
                                    });
                                }
                            } else {
                                response.error = Some(AgentError {
                                    code: "invalid_input".to_string(),
                                    message: "Missing document_root".to_string(),
                                });
                            }
                        } else {
                            response.error = Some(AgentError {
                                code: "invalid_input".to_string(),
                                message: "Missing domain".to_string(),
                            });
                        }
                    }
                    "executor.run" => {
                        if let Some(cmd) = req.payload.get("cmd").and_then(|v| v.as_str()) {
                            let args = req
                                .payload
                                .get("args")
                                .and_then(|v| v.as_array())
                                .map(|arr| {
                                    arr.iter()
                                        .filter_map(|x| x.as_str().map(|s| s.to_string()))
                                        .collect()
                                })
                                .unwrap_or_default();
                            let timeout_secs = req
                                .payload
                                .get("timeout_secs")
                                .and_then(|v| v.as_u64())
                                .unwrap_or(30);

                            let cmd_req = CommandRequest {
                                cmd: cmd.to_string(),
                                args,
                                envs: HashMap::new(),
                                timeout: Duration::from_secs(timeout_secs),
                            };

                            match CommandExecutor::execute(&cmd_req).await {
                                Ok(res) => {
                                    response.result = Some(json!({
                                        "exit_code": res.exit_code,
                                        "stdout": String::from_utf8_lossy(&res.stdout).to_string(),
                                        "stderr": String::from_utf8_lossy(&res.stderr).to_string(),
                                        "timeout": res.timeout
                                    }));
                                }
                                Err(e) => {
                                    response.error = Some(AgentError {
                                        code: "execution_failed".to_string(),
                                        message: e.to_string(),
                                    });
                                }
                            }
                        } else {
                            response.error = Some(AgentError {
                                code: "invalid_input".to_string(),
                                message: "Missing cmd".to_string(),
                            });
                        }
                    }
                    _ => {
                        response.error = Some(AgentError {
                            code: "unsupported_operation".to_string(),
                            message: "Unsupported operation".to_string(),
                        });
                    }
                }
            }

            let resp_str = serde_json::to_string(&response)?;
            ws_stream.send(Message::Text(resp_str)).await?;
        }
    }

    Ok(())
}
