#![forbid(unsafe_code)]

use crate::metrics::{MetricsCollector, SystemMetrics};
use reqwest::{Client, Method, Response, StatusCode, Url};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{net::IpAddr, path::Path, time::Duration};
use tokio::time::{sleep, Instant};

const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
const POLL_INTERVAL: Duration = Duration::from_secs(2);
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(30);
const MAX_LOG_BYTES: usize = 4096;

#[derive(Debug, PartialEq, Eq)]
pub enum WorkerError {
    Unauthorized,
    ApiStatus(u16),
    Transport,
    InvalidResponse,
    InvalidConfiguration,
}

impl std::fmt::Display for WorkerError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.safe_message())
    }
}

impl std::error::Error for WorkerError {}

impl WorkerError {
    pub fn safe_message(&self) -> String {
        match self {
            Self::Unauthorized => "Agent authentication was rejected".to_string(),
            Self::ApiStatus(status) => format!("API returned HTTP status {status}"),
            Self::Transport => "API transport failed; retrying is safe".to_string(),
            Self::InvalidResponse => "API returned an invalid response".to_string(),
            Self::InvalidConfiguration => {
                "Agent configuration is invalid; remote endpoints require TLS".to_string()
            }
        }
    }
}

/// Plaintext WebSocket is limited to loopback; remote endpoints must use WSS.
pub fn validate_websocket_url(raw_url: &str) -> Result<(), WorkerError> {
    let url = Url::parse(raw_url).map_err(|_| WorkerError::InvalidConfiguration)?;
    if !matches!(url.scheme(), "ws" | "wss") || url.host_str().is_none() {
        return Err(WorkerError::InvalidConfiguration);
    }
    if url.scheme() == "ws" && !is_loopback_or_localhost(&url) {
        return Err(WorkerError::InvalidConfiguration);
    }

    Ok(())
}

/// Validates both legacy WebSocket endpoints before the agent opens either connection.
pub fn validate_legacy_websocket_urls(
    agent_url: &str,
    heartbeat_url: &str,
) -> Result<(), WorkerError> {
    validate_websocket_url(agent_url)?;
    validate_websocket_url(heartbeat_url)
}

#[derive(Clone)]
struct ApiClient {
    base_url: Url,
    client: Client,
}

impl ApiClient {
    fn new(base_url: &str) -> Result<Self, WorkerError> {
        let mut base_url = Url::parse(base_url).map_err(|_| WorkerError::InvalidConfiguration)?;
        if !matches!(base_url.scheme(), "http" | "https") || base_url.host_str().is_none() {
            return Err(WorkerError::InvalidConfiguration);
        }
        let is_local_http = base_url.scheme() == "http" && is_loopback_or_localhost(&base_url);
        if base_url.scheme() == "http" && !is_local_http {
            return Err(WorkerError::InvalidConfiguration);
        }
        if !base_url.path().ends_with('/') {
            let path = format!("{}/", base_url.path());
            base_url.set_path(&path);
        }
        let mut client_builder = Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .redirect(reqwest::redirect::Policy::none());
        if is_local_http {
            // Never send local bearer credentials through a system HTTP proxy.
            client_builder = client_builder.no_proxy();
        }
        let client = client_builder
            .build()
            .map_err(|_| WorkerError::InvalidConfiguration)?;
        Ok(Self { base_url, client })
    }

    fn endpoint(&self, path: &str) -> Result<Url, WorkerError> {
        self.base_url
            .join(path)
            .map_err(|_| WorkerError::InvalidConfiguration)
    }

    async fn send_json<T: Serialize>(
        &self,
        method: Method,
        path: &str,
        token: &str,
        body: &T,
    ) -> Result<Response, WorkerError> {
        let request = self.json_request(method, path, token, body)?;
        self.client
            .execute(request)
            .await
            .map_err(|_| WorkerError::Transport)
    }

    fn json_request<T: Serialize>(
        &self,
        method: Method,
        path: &str,
        token: &str,
        body: &T,
    ) -> Result<reqwest::Request, WorkerError> {
        self.client
            .request(method, self.endpoint(path)?)
            .bearer_auth(token)
            .json(body)
            .build()
            .map_err(|_| WorkerError::InvalidConfiguration)
    }

    async fn register(
        &self,
        admin_token: &str,
        name: &str,
    ) -> Result<NodeCredentials, WorkerError> {
        let response = self
            .send_json(
                Method::POST,
                "v1/nodes/register",
                admin_token,
                &RegisterRequest { name },
            )
            .await?;
        ensure_success(&response)?;
        let registration: RegisterResponse = response
            .json()
            .await
            .map_err(|_| WorkerError::InvalidResponse)?;
        if !is_uuid(&registration.node_id) || registration.token.is_empty() {
            return Err(WorkerError::InvalidResponse);
        }
        Ok(NodeCredentials {
            node_id: registration.node_id,
            token: registration.token,
        })
    }

    async fn heartbeat(&self, node: &NodeCredentials) -> Result<(), WorkerError> {
        let path = format!("v1/nodes/{}/heartbeat", node.node_id);
        let response = self
            .send_json(Method::POST, &path, &node.token, &EmptyRequest {})
            .await?;
        ensure_success(&response)
    }

    async fn claim(&self, node: &NodeCredentials) -> Result<Option<ClaimedJob>, WorkerError> {
        let response = self
            .send_json(
                Method::POST,
                "v1/jobs/claim",
                &node.token,
                &ClaimRequest {
                    node_id: &node.node_id,
                },
            )
            .await?;
        ensure_success(&response)?;
        let response: ClaimResponse = response
            .json()
            .await
            .map_err(|_| WorkerError::InvalidResponse)?;
        if let Some(job) = response.job.as_ref() {
            if !is_uuid(&job.id) {
                return Err(WorkerError::InvalidResponse);
            }
        }
        Ok(response.job)
    }

    async fn append_log(
        &self,
        node: &NodeCredentials,
        job_id: &str,
        stream: &str,
        text: &str,
    ) -> Result<(), WorkerError> {
        if !is_uuid(job_id) {
            return Err(WorkerError::InvalidResponse);
        }
        let path = format!("v1/jobs/{job_id}/logs");
        let response = self
            .send_json(
                Method::POST,
                &path,
                &node.token,
                &AppendLogRequest {
                    node_id: &node.node_id,
                    stream,
                    text,
                },
            )
            .await?;
        ensure_success(&response)
    }

    async fn complete(
        &self,
        node: &NodeCredentials,
        job_id: &str,
        outcome: &OperationOutcome,
    ) -> Result<(), WorkerError> {
        if !is_uuid(job_id) {
            return Err(WorkerError::InvalidResponse);
        }
        let path = format!("v1/jobs/{job_id}/complete");
        let response = self
            .send_json(
                Method::POST,
                &path,
                &node.token,
                &CompleteRequest {
                    node_id: &node.node_id,
                    state: if outcome.succeeded {
                        "succeeded"
                    } else {
                        "failed"
                    },
                    error_text: outcome.error_text.as_deref(),
                },
            )
            .await?;
        ensure_success(&response)
    }
}

fn is_loopback_or_localhost(url: &Url) -> bool {
    let Some(host) = url.host_str() else {
        return false;
    };
    let host = host
        .strip_prefix('[')
        .and_then(|host| host.strip_suffix(']'))
        .unwrap_or(host);
    host.eq_ignore_ascii_case("localhost")
        || host
            .parse::<IpAddr>()
            .is_ok_and(|address| address.is_loopback())
}

#[derive(Debug)]
struct NodeCredentials {
    node_id: String,
    token: String,
}

#[derive(Serialize)]
struct RegisterRequest<'a> {
    name: &'a str,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RegisterResponse {
    node_id: String,
    token: String,
}

#[derive(Serialize)]
struct EmptyRequest {}

#[derive(Serialize)]
struct ClaimRequest<'a> {
    node_id: &'a str,
}

#[derive(Deserialize)]
struct ClaimResponse {
    job: Option<ClaimedJob>,
}

#[derive(Deserialize)]
struct ClaimedJob {
    id: String,
    payload: Value,
}

#[derive(Serialize)]
struct AppendLogRequest<'a> {
    node_id: &'a str,
    stream: &'a str,
    text: &'a str,
}

#[derive(Serialize)]
struct CompleteRequest<'a> {
    node_id: &'a str,
    state: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    error_text: Option<&'a str>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct JobRequest {
    version: u32,
    request_id: String,
    operation: String,
    payload: Value,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SystemInfoPayload {}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SiteValidatePayload {
    domain: String,
    document_root: String,
}

#[derive(Debug)]
enum TypedOperation {
    SystemInfo,
    SiteValidate {
        domain: String,
        document_root: String,
    },
}

#[derive(Debug)]
struct OperationOutcome {
    succeeded: bool,
    log_text: String,
    error_text: Option<String>,
}

impl OperationOutcome {
    fn success(text: String) -> Self {
        Self {
            succeeded: true,
            log_text: text,
            error_text: None,
        }
    }

    fn failure(message: &'static str) -> Self {
        Self {
            succeeded: false,
            log_text: message.to_string(),
            error_text: Some(message.to_string()),
        }
    }
}

pub struct AgentWorker {
    api: ApiClient,
    admin_token: String,
    node_name: String,
    node: Option<NodeCredentials>,
    metrics: MetricsCollector,
    poll_interval: Duration,
    heartbeat_interval: Duration,
}

impl AgentWorker {
    pub fn new(api_url: &str, admin_token: String, node_name: String) -> Result<Self, WorkerError> {
        if admin_token.trim().is_empty() || node_name.trim().is_empty() {
            return Err(WorkerError::InvalidConfiguration);
        }
        Ok(Self {
            api: ApiClient::new(api_url)?,
            admin_token,
            node_name,
            node: None,
            metrics: MetricsCollector::default(),
            poll_interval: POLL_INTERVAL,
            heartbeat_interval: HEARTBEAT_INTERVAL,
        })
    }

    async fn register(&mut self) -> Result<(), WorkerError> {
        let node = self
            .api
            .register(&self.admin_token, &self.node_name)
            .await?;
        self.node = Some(node);
        Ok(())
    }

    async fn heartbeat(&self) -> Result<(), WorkerError> {
        let node = self
            .node
            .as_ref()
            .ok_or(WorkerError::InvalidConfiguration)?;
        self.api.heartbeat(node).await
    }

    pub async fn run_once(&mut self) -> Result<bool, WorkerError> {
        if self.node.is_none() {
            self.register().await?;
        }
        let node = self
            .node
            .as_ref()
            .ok_or(WorkerError::InvalidConfiguration)?;
        let Some(job) = self.api.claim(node).await? else {
            return Ok(false);
        };
        self.process_job(job).await?;
        Ok(true)
    }

    async fn process_job(&mut self, job: ClaimedJob) -> Result<(), WorkerError> {
        let node = self
            .node
            .as_ref()
            .ok_or(WorkerError::InvalidConfiguration)?;
        let outcome = match parse_operation(&job.payload) {
            Ok(operation) => execute_operation(operation, &mut self.metrics),
            Err(outcome) => outcome,
        };
        let log_text = sanitize_bounded(
            &outcome.log_text,
            &[self.admin_token.as_str(), node.token.as_str()],
        );
        if !log_text.is_empty() {
            if let Err(error) = self
                .api
                .append_log(node, &job.id, "system", &log_text)
                .await
            {
                // A claimed job whose output cannot be persisted must never be reported as
                // successful. The reconciliation API remains available if completion also fails.
                let failed = OperationOutcome::failure("Failed to persist operation output");
                let _ = self.api.complete(node, &job.id, &failed).await;
                return Err(error);
            }
        }
        self.api.complete(node, &job.id, &outcome).await
    }

    pub async fn run(&mut self) -> Result<(), WorkerError> {
        self.register().await?;
        let mut last_heartbeat = Instant::now();
        let mut heartbeat_due = true;
        let mut shutdown = Box::pin(tokio::signal::ctrl_c());
        loop {
            tokio::select! {
                signal_result = &mut shutdown => {
                    signal_result.map_err(|_| WorkerError::Transport)?;
                    return Ok(());
                }
                _ = sleep(self.poll_interval) => {}
            }

            if heartbeat_due || last_heartbeat.elapsed() >= self.heartbeat_interval {
                match self.heartbeat().await {
                    Ok(()) => {
                        last_heartbeat = Instant::now();
                        heartbeat_due = false;
                    }
                    Err(WorkerError::Unauthorized) => return Err(WorkerError::Unauthorized),
                    Err(_) => eprintln!("Agent heartbeat failed; retrying"),
                }
            }

            match self.run_once().await {
                Ok(_) => {}
                Err(WorkerError::Unauthorized) => return Err(WorkerError::Unauthorized),
                Err(_) => eprintln!("Agent job poll or report failed; retrying"),
            }
        }
    }
}

fn ensure_success(response: &Response) -> Result<(), WorkerError> {
    classify_status(response.status())
}

fn classify_status(status: StatusCode) -> Result<(), WorkerError> {
    if status.is_success() {
        Ok(())
    } else if matches!(status, StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN) {
        Err(WorkerError::Unauthorized)
    } else {
        Err(WorkerError::ApiStatus(status.as_u16()))
    }
}

fn parse_operation(payload: &Value) -> Result<TypedOperation, OperationOutcome> {
    let request: JobRequest = serde_json::from_value(payload.clone())
        .map_err(|_| OperationOutcome::failure("Malformed agent operation request"))?;
    if request.version != 1 || !is_uuid(&request.request_id) {
        return Err(OperationOutcome::failure(
            "Unsupported agent protocol version or request ID",
        ));
    }
    match request.operation.as_str() {
        "system.info" => {
            serde_json::from_value::<SystemInfoPayload>(request.payload)
                .map_err(|_| OperationOutcome::failure("Malformed system.info payload"))?;
            Ok(TypedOperation::SystemInfo)
        }
        "site.validate" => {
            let payload: SiteValidatePayload = serde_json::from_value(request.payload)
                .map_err(|_| OperationOutcome::failure("Malformed site.validate payload"))?;
            if !valid_domain(&payload.domain) || !valid_document_root(&payload.document_root) {
                return Err(OperationOutcome::failure("site.validate input is invalid"));
            }
            Ok(TypedOperation::SiteValidate {
                domain: payload.domain,
                document_root: payload.document_root,
            })
        }
        _ => Err(OperationOutcome::failure("Unsupported agent operation")),
    }
}

fn execute_operation(
    operation: TypedOperation,
    metrics: &mut MetricsCollector,
) -> OperationOutcome {
    match operation {
        TypedOperation::SystemInfo => {
            if !["/proc/stat", "/proc/meminfo", "/proc/diskstats"]
                .iter()
                .all(|path| Path::new(path).is_file())
            {
                return OperationOutcome::failure("system.info is unavailable on this host");
            }
            match metrics.collect() {
                Ok(metrics) => OperationOutcome::success(system_info_summary(&metrics)),
                Err(_) => OperationOutcome::failure("system.info metrics collection failed"),
            }
        }
        TypedOperation::SiteValidate {
            domain,
            document_root,
        } => OperationOutcome::success(format!(
            "site.validate passed for {domain} at {document_root}; no provisioning was performed"
        )),
    }
}

fn system_info_summary(metrics: &SystemMetrics) -> String {
    serde_json::to_string(&json!({
        "operation": "system.info",
        "cpu_usage_percent": metrics.cpu_usage_percent,
        "memory_used_kb": metrics.memory_used_kb,
        "memory_available_kb": metrics.memory_available_kb,
        "disk_io_ops": metrics.disk_io_ops,
    }))
    .unwrap_or_else(|_| "system.info result could not be serialized".to_string())
}

fn valid_domain(domain: &str) -> bool {
    if domain.is_empty()
        || domain.len() > 253
        || !domain.is_ascii()
        || domain.parse::<std::net::IpAddr>().is_ok()
    {
        return false;
    }
    domain.split('.').all(|label| {
        !label.is_empty()
            && label.len() <= 63
            && !label.starts_with('-')
            && !label.ends_with('-')
            && label
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    })
}

fn valid_document_root(document_root: &str) -> bool {
    if !document_root.starts_with("/srv/fusionpanel/sites/") || document_root.contains('\0') {
        return false;
    }
    let mut components = Path::new(document_root).components();
    matches!(components.next(), Some(std::path::Component::RootDir))
        && components.next() == Some(std::path::Component::Normal("srv".as_ref()))
        && components.next() == Some(std::path::Component::Normal("fusionpanel".as_ref()))
        && components.next() == Some(std::path::Component::Normal("sites".as_ref()))
        && components.all(|component| matches!(component, std::path::Component::Normal(_)))
        && Path::new(document_root).components().count() > 4
}

fn is_uuid(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 36
        && [8, 13, 18, 23].iter().all(|index| bytes[*index] == b'-')
        && bytes
            .iter()
            .enumerate()
            .all(|(index, byte)| [8, 13, 18, 23].contains(&index) || byte.is_ascii_hexdigit())
}

pub fn sanitize_bounded(input: &str, secrets: &[&str]) -> String {
    let mut output = input.to_string();
    for secret in secrets.iter().filter(|secret| !secret.is_empty()) {
        output = output.replace(secret, "[REDACTED]");
    }
    let mut bounded = String::new();
    let truncation_marker = "…";
    let content_limit = MAX_LOG_BYTES - truncation_marker.len();
    for character in output.chars() {
        if character.is_control() && character != '\n' && character != '\t' {
            continue;
        }
        if bounded.len() + character.len_utf8() > content_limit {
            bounded.push_str(truncation_marker);
            break;
        }
        bounded.push(character);
    }
    bounded
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_job(operation: &str, payload: Value) -> Value {
        json!({
            "version": 1,
            "request_id": "00000000-0000-4000-8000-000000000001",
            "operation": operation,
            "payload": payload,
        })
    }

    #[test]
    fn parses_only_supported_typed_operations() {
        assert!(matches!(
            parse_operation(&valid_job("system.info", json!({}))),
            Ok(TypedOperation::SystemInfo)
        ));
        assert!(matches!(
            parse_operation(&valid_job(
                "site.validate",
                json!({
                    "domain": "example.test",
                    "document_root": "/srv/fusionpanel/sites/example.test"
                })
            )),
            Ok(TypedOperation::SiteValidate { .. })
        ));
        assert!(parse_operation(&valid_job("executor.run", json!({ "cmd": "echo" }))).is_err());
        assert!(parse_operation(&valid_job("system.info", json!({ "extra": true }))).is_err());
    }

    #[test]
    fn site_validation_rejects_unicode_ip_and_path_traversal() {
        assert!(!valid_domain("127.0.0.1"));
        assert!(!valid_domain("éxample.test"));
        assert!(!valid_document_root("/srv/fusionpanel/sites/../etc"));
        assert!(valid_document_root("/srv/fusionpanel/sites/example.test"));
    }

    #[test]
    fn log_output_is_bounded_and_redacts_tokens() {
        let output = sanitize_bounded(
            &format!("credential={};{}", "node-secret", "x".repeat(5000)),
            &["node-secret"],
        );
        assert!(output.contains("[REDACTED]"));
        assert!(!output.contains("node-secret"));
        assert!(output.len() <= MAX_LOG_BYTES + 3);
    }

    #[test]
    fn api_request_uses_bearer_and_register_contract_without_logging_it() {
        let api = ApiClient::new("http://127.0.0.1:8080/base").unwrap();
        let request = api
            .json_request(
                Method::POST,
                "v1/nodes/register",
                "admin-secret",
                &RegisterRequest { name: "node-test" },
            )
            .unwrap();
        assert_eq!(request.method(), Method::POST);
        assert_eq!(request.url().path(), "/base/v1/nodes/register");
        assert_eq!(
            request
                .headers()
                .get(reqwest::header::AUTHORIZATION)
                .unwrap(),
            "Bearer admin-secret"
        );
        let body: Value =
            serde_json::from_slice(request.body().and_then(reqwest::Body::as_bytes).unwrap())
                .unwrap();
        assert_eq!(body, json!({ "name": "node-test" }));
        assert!(!WorkerError::Unauthorized
            .safe_message()
            .contains("admin-secret"));
    }

    #[test]
    fn api_client_allows_local_http_and_remote_https_only() {
        for url in [
            "http://127.0.0.1:8080",
            "http://[::1]:8080",
            "http://localhost:8080",
            "https://api.example.test",
        ] {
            assert!(ApiClient::new(url).is_ok(), "expected {url} to be allowed");
        }
    }

    #[test]
    fn api_client_rejects_remote_http_and_lookalike_localhost_hosts() {
        for url in [
            "http://api.example.test",
            "http://192.168.1.10:8080",
            "http://localhost.example.test:8080",
        ] {
            assert_eq!(
                ApiClient::new(url).err(),
                Some(WorkerError::InvalidConfiguration),
                "expected {url} to be rejected"
            );
        }
    }

    #[test]
    fn legacy_websocket_urls_allow_loopback_ws_and_remote_wss() {
        for url in [
            "ws://127.0.0.1:8080/agent",
            "ws://[::1]:8080/heartbeat",
            "ws://localhost:8080/agent",
            "wss://agent.example.test/agent",
        ] {
            assert!(
                validate_legacy_websocket_urls(url, "ws://127.0.0.1:8080/heartbeat").is_ok(),
                "expected {url} to be allowed"
            );
        }
    }

    #[test]
    fn legacy_websocket_urls_reject_remote_ws_for_either_endpoint() {
        assert_eq!(
            validate_legacy_websocket_urls(
                "ws://agent.example.test/agent",
                "wss://agent.example.test/heartbeat"
            ),
            Err(WorkerError::InvalidConfiguration)
        );
        assert_eq!(
            validate_legacy_websocket_urls(
                "wss://agent.example.test/agent",
                "ws://agent.example.test/heartbeat"
            ),
            Err(WorkerError::InvalidConfiguration)
        );
    }

    #[test]
    fn legacy_websocket_urls_reject_missing_hosts_and_other_schemes() {
        for url in [
            "ws:///agent",
            "ftp://agent.example.test/agent",
            "http://localhost/agent",
        ] {
            assert_eq!(
                validate_legacy_websocket_urls(url, "ws://127.0.0.1:8080/heartbeat"),
                Err(WorkerError::InvalidConfiguration),
                "expected {url} to be rejected"
            );
        }
    }

    #[test]
    fn api_status_handling_rejects_auth_and_server_errors() {
        assert_eq!(
            classify_status(StatusCode::UNAUTHORIZED),
            Err(WorkerError::Unauthorized)
        );
        assert_eq!(
            classify_status(StatusCode::FORBIDDEN),
            Err(WorkerError::Unauthorized)
        );
        assert_eq!(
            classify_status(StatusCode::INTERNAL_SERVER_ERROR),
            Err(WorkerError::ApiStatus(500))
        );
        assert!(classify_status(StatusCode::NO_CONTENT).is_ok());
    }
}
