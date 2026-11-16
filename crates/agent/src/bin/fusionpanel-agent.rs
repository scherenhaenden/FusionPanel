#![forbid(unsafe_code)]

use agent::{
    daemon::{AgentDaemon, HmacWsTransport},
    executor::{CommandExecutor, CommandRequest},
    metrics::MetricsCollector,
    mode::{select_mode, AgentMode},
    protocol::{AgentError, AgentRequest, AgentResponse},
    worker::{sanitize_bounded, validate_legacy_websocket_urls, AgentWorker, AgentWorkerConfig},
};
use futures_util::{SinkExt, StreamExt};
use hmac::{Hmac, Mac};
use serde_json::json;
use sha2::Sha256;
use std::{collections::HashMap, env, path::Path, process, time::Duration};
use tokio::{signal, time::sleep};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::{client::IntoClientRequest, Message};

#[tokio::main]
async fn main() {
    let mode_text = env::var("FUSIONPANEL_AGENT_MODE").ok();
    let legacy_environment_present = [
        "FUSIONPANEL_AGENT_URL",
        "FUSIONPANEL_AGENT_HEARTBEAT_URL",
        "FUSIONPANEL_AGENT_SECRET",
    ]
    .iter()
    .any(|key| env::var_os(key).is_some());
    let mode = match select_mode(mode_text.as_deref(), legacy_environment_present) {
        Ok(mode) => mode,
        Err(_) => {
            eprintln!("Set FUSIONPANEL_AGENT_MODE to http or legacy-ws");
            process::exit(1);
        }
    };

    let result = match mode {
        AgentMode::Http => run_http_worker().await,
        AgentMode::LegacyWebSocket => run_legacy_websocket().await,
    };
    if let Err(message) = result {
        eprintln!("Agent stopped: {message}");
        process::exit(1);
    }
}

async fn run_http_worker() -> Result<(), String> {
    let api_url =
        env::var("FUSIONPANEL_API_URL").unwrap_or_else(|_| "http://127.0.0.1:8080".to_string());
    let worker_config = AgentWorkerConfig {
        api_url,
        admin_token: env::var("FUSIONPANEL_ADMIN_TOKEN")
            .ok()
            .filter(|token| !token.trim().is_empty()),
        node_name: env::var("FUSIONPANEL_NODE_NAME").ok(),
        node_id: env::var("FUSIONPANEL_NODE_ID").ok(),
        node_token: env::var("FUSIONPANEL_NODE_TOKEN")
            .ok()
            .filter(|token| !token.trim().is_empty()),
    };
    let mut worker =
        AgentWorker::from_config(worker_config).map_err(|error| error.safe_message())?;
    worker.run().await.map_err(|error| error.safe_message())
}

async fn run_legacy_websocket() -> Result<(), String> {
    let url = env::var("FUSIONPANEL_AGENT_URL")
        .map_err(|_| "FUSIONPANEL_AGENT_URL is required in legacy-ws mode".to_string())?;
    let secret = env::var("FUSIONPANEL_AGENT_SECRET")
        .ok()
        .filter(|secret| !secret.trim().is_empty())
        .ok_or_else(|| "FUSIONPANEL_AGENT_SECRET is required in legacy-ws mode".to_string())?;
    let heartbeat_url = env::var("FUSIONPANEL_AGENT_HEARTBEAT_URL")
        .unwrap_or_else(|_| "ws://127.0.0.1:8080/heartbeat".to_string());
    validate_legacy_websocket_urls(&url, &heartbeat_url).map_err(|_| {
        "legacy-ws remote endpoints must use WSS; WS is allowed only for loopback".to_string()
    })?;
    let transport = HmacWsTransport::new(heartbeat_url, secret.as_bytes().to_vec());
    let daemon = AgentDaemon::new(
        transport,
        option_env!("FUSIONPANEL_BUILD_VERSION")
            .unwrap_or("development")
            .to_string(),
    );
    tokio::spawn(async move { daemon.run().await });

    let protocol_loop = async move {
        loop {
            if run_legacy_protocol_loop(&url, secret.as_bytes())
                .await
                .is_err()
            {
                eprintln!("Legacy WebSocket request failed; reconnecting");
                sleep(Duration::from_secs(5)).await;
            }
        }
    };
    tokio::select! {
        _ = protocol_loop => Ok(()),
        signal_result = signal::ctrl_c() => {
            signal_result.map_err(|_| "Could not install shutdown handler".to_string())?;
            Ok(())
        }
    }
}

fn is_allowed_command(command: &str) -> bool {
    matches!(command, "echo" | "uptime" | "free" | "df")
}

async fn run_legacy_protocol_loop(
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

    let (mut websocket, response) = connect_async(request).await?;
    let server_signature = response
        .headers()
        .get("x-hmac-signature")
        .ok_or("Missing server signature")?;
    let server_timestamp = response
        .headers()
        .get("x-timestamp")
        .ok_or("Missing server timestamp")?;
    let mut verify_mac = HmacSha256::new_from_slice(secret)?;
    verify_mac.update(b"fusionpanel-control-plane-accept:");
    verify_mac.update(server_timestamp.as_bytes());
    let expected = hex::encode(verify_mac.finalize().into_bytes());
    if server_signature.to_str()? != expected {
        return Err("Server authentication failed".into());
    }
    let signed_at: u64 = server_timestamp.to_str()?.parse()?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs();
    if now.abs_diff(signed_at) > 300 {
        return Err("Server authentication failed".into());
    }

    let mut metrics_collector = MetricsCollector::default();
    while let Some(message) = websocket.next().await {
        let Message::Text(text) = message? else {
            continue;
        };
        let request: AgentRequest = match serde_json::from_str(&text) {
            Ok(request) => request,
            Err(_) => continue,
        };
        let response = handle_legacy_request(&request, &mut metrics_collector, secret).await;
        websocket
            .send(Message::Text(serde_json::to_string(&response)?))
            .await?;
    }
    Ok(())
}

async fn handle_legacy_request(
    request: &AgentRequest,
    metrics: &mut MetricsCollector,
    secret: &[u8],
) -> AgentResponse {
    let mut response = AgentResponse {
        version: request.version,
        request_id: request.request_id.clone(),
        result: None,
        error: None,
    };
    if request.version != 1 {
        response.error = Some(AgentError {
            code: "unsupported_version".to_string(),
            message: "Unsupported protocol version".to_string(),
        });
        return response;
    }

    match request.operation.as_str() {
        "system.info" => match metrics.collect() {
            Ok(metrics) => {
                response.result = Some(json!({
                    "cpu_usage_percent": metrics.cpu_usage_percent,
                    "memory_used_kb": metrics.memory_used_kb,
                    "memory_available_kb": metrics.memory_available_kb,
                    "disk_io_ops": metrics.disk_io_ops,
                }));
            }
            Err(_) => {
                response.error = Some(AgentError {
                    code: "metrics_failed".to_string(),
                    message: "Failed to collect system metrics".to_string(),
                })
            }
        },
        "site.validate" => {
            let domain = request.payload.get("domain").and_then(ValueExt::string);
            let root = request
                .payload
                .get("document_root")
                .and_then(ValueExt::string);
            if domain.is_some_and(valid_domain) && root.is_some_and(valid_document_root) {
                response.result = Some(json!({ "valid": true }));
            } else {
                response.error = Some(AgentError {
                    code: "invalid_input".to_string(),
                    message: "Invalid domain or document root".to_string(),
                });
            }
        }
        "executor.run" => {
            let command = request.payload.get("cmd").and_then(ValueExt::string);
            let Some(command) = command.filter(|command| is_allowed_command(command)) else {
                response.error = Some(AgentError {
                    code: "operation_blocked".to_string(),
                    message: "Command is not in the legacy allowlist".to_string(),
                });
                return response;
            };
            let args = match request.payload.get("args") {
                None => Vec::new(),
                Some(value) => match value.as_array() {
                    Some(values) if values.len() <= 64 => match values
                        .iter()
                        .map(|value| value.as_str().map(str::to_owned))
                        .collect::<Option<Vec<_>>>()
                    {
                        Some(args) => args,
                        None => {
                            response.error = Some(AgentError {
                                code: "invalid_input".to_string(),
                                message: "Command arguments must be strings".to_string(),
                            });
                            return response;
                        }
                    },
                    _ => {
                        response.error = Some(AgentError {
                            code: "invalid_input".to_string(),
                            message: "Command arguments are invalid".to_string(),
                        });
                        return response;
                    }
                },
            };
            let timeout_seconds = request
                .payload
                .get("timeout_secs")
                .and_then(|value| value.as_u64())
                .unwrap_or(30)
                .clamp(1, 60);
            let command_request = CommandRequest {
                cmd: command.to_string(),
                args,
                envs: HashMap::new(),
                timeout: Duration::from_secs(timeout_seconds),
            };
            match CommandExecutor::execute(&command_request).await {
                Ok(result) => {
                    let stdout = sanitize_bounded(
                        &String::from_utf8_lossy(&result.stdout),
                        &[std::str::from_utf8(secret).unwrap_or("")],
                    );
                    let stderr = sanitize_bounded(
                        &String::from_utf8_lossy(&result.stderr),
                        &[std::str::from_utf8(secret).unwrap_or("")],
                    );
                    response.result = Some(json!({
                        "exit_code": result.exit_code,
                        "stdout": stdout,
                        "stderr": stderr,
                        "timeout": result.timeout
                    }));
                }
                Err(_) => {
                    response.error = Some(AgentError {
                        code: "execution_failed".to_string(),
                        message: "Allowlisted command execution failed".to_string(),
                    })
                }
            }
        }
        _ => {
            response.error = Some(AgentError {
                code: "unsupported_operation".to_string(),
                message: "Unsupported operation".to_string(),
            })
        }
    }
    response
}

trait ValueExt {
    fn string(&self) -> Option<&str>;
}

impl ValueExt for serde_json::Value {
    fn string(&self) -> Option<&str> {
        self.as_str()
    }
}

fn valid_domain(domain: &str) -> bool {
    !domain.is_empty()
        && domain.len() <= 253
        && domain.is_ascii()
        && domain.parse::<std::net::IpAddr>().is_err()
        && domain.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        })
}

fn valid_document_root(root: &str) -> bool {
    root.starts_with("/srv/fusionpanel/sites/")
        && !root.contains('\0')
        && Path::new(root).components().all(|component| {
            matches!(
                component,
                std::path::Component::RootDir | std::path::Component::Normal(_)
            )
        })
        && Path::new(root).components().count() > 4
}
