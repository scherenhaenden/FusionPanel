#![forbid(unsafe_code)]

use crate::backup_driver::{execute_backup_create, execute_backup_restore};
use crate::cron_driver::{execute_cron_apply, execute_cron_delete, execute_cron_run_now};
use crate::database_driver::{DatabaseDriver, RealDatabaseDriver};
use crate::dns_driver::{execute_dns_probe, execute_dns_zone_apply, execute_dns_zone_delete};
use crate::metrics::{MetricsCollector, SystemMetrics};
use crate::nginx_helper_rpc::NginxHelperClient;
use fusionpanel_agent_protocol::{
    BackupCreatePayload, BackupDestination, BackupRestorePayload, DatabaseEngineKind,
    Operation as ProtocolOperation, Request as ProtocolRequest,
};
use hosting::{ManagedSiteError, ManagedSiteOperations, ManagedSiteSpec};
use reqwest::{Client, Method, Response, StatusCode, Url};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{net::IpAddr, path::Path, time::Duration};
use tokio::time::{sleep, Instant};
use uuid::Uuid;

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
                "Agent configuration is invalid; check endpoint and bootstrap credentials"
                    .to_string()
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
    is_local: bool,
}

impl ApiClient {
    fn new(base_url: &str) -> Result<Self, WorkerError> {
        let mut base_url = Url::parse(base_url).map_err(|_| WorkerError::InvalidConfiguration)?;
        if !matches!(base_url.scheme(), "http" | "https") || base_url.host_str().is_none() {
            return Err(WorkerError::InvalidConfiguration);
        }
        let is_local = is_loopback_or_localhost(&base_url);
        let is_local_http = base_url.scheme() == "http" && is_local;
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
        Ok(Self {
            base_url,
            client,
            is_local,
        })
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

    fn is_local(&self) -> bool {
        self.is_local
    }

    async fn heartbeat(&self, node: &NodeCredentials) -> Result<(), WorkerError> {
        let path = format!("v1/nodes/{}/heartbeat", node.node_id);
        let response = self
            .send_json(Method::POST, &path, &node.token, &EmptyRequest {})
            .await?;
        ensure_success(&response)
    }

    async fn publish_metrics(
        &self,
        node: &NodeCredentials,
        metrics: &SystemMetrics,
    ) -> Result<(), WorkerError> {
        let path = format!("v1/nodes/{}/metrics", node.node_id);
        let response = self
            .send_json(Method::POST, &path, &node.token, metrics)
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

/// Startup configuration. Remote agents use a pre-provisioned, node-scoped
/// token; only loopback agents may self-register with the admin token.
pub struct AgentWorkerConfig {
    pub api_url: String,
    pub admin_token: Option<String>,
    pub node_name: Option<String>,
    pub node_id: Option<String>,
    pub node_token: Option<String>,
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

#[derive(Debug)]
enum TypedOperation {
    SystemInfo,
    SiteValidate {
        domain: String,
        document_root: String,
    },
    SiteApply {
        site_id: Uuid,
        domain: String,
        spec: ManagedSiteSpec,
    },
    SiteDelete {
        site_id: Uuid,
        domain: String,
    },
    TlsApply {
        site_id: Uuid,
        domain: String,
        spec: ManagedSiteSpec,
        source: fusionpanel_agent_protocol::TlsCertificateSource,
    },
    TlsDelete {
        site_id: Uuid,
        domain: String,
        spec: ManagedSiteSpec,
    },
    DatabaseApply {
        name: String,
        engine: DatabaseEngineKind,
    },
    DatabaseDelete {
        name: String,
        engine: DatabaseEngineKind,
    },
    DatabaseUserCreate {
        database_name: String,
        engine: DatabaseEngineKind,
        username: String,
        password: String,
    },
    BackupCreate(Box<BackupCreatePayload>),
    BackupRestore(Box<BackupRestorePayload>),
    DnsZoneApply(Box<fusionpanel_agent_protocol::DnsZoneApplyPayload>),
    DnsZoneDelete(fusionpanel_agent_protocol::DnsZoneDeletePayload),
    DnsProbe(fusionpanel_agent_protocol::DnsProbePayload),
    CronApply(fusionpanel_agent_protocol::CronApplyPayload),
    CronDelete(fusionpanel_agent_protocol::CronDeletePayload),
    CronRunNow(fusionpanel_agent_protocol::CronRunNowPayload),
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
    admin_token: Option<String>,
    node_name: Option<String>,
    node: Option<NodeCredentials>,
    metrics: MetricsCollector,
    telemetry_metrics: MetricsCollector,
    site_operations: Box<dyn ManagedSiteOperations>,
    poll_interval: Duration,
    heartbeat_interval: Duration,
}

impl AgentWorker {
    pub fn new(api_url: &str, admin_token: String, node_name: String) -> Result<Self, WorkerError> {
        Self::from_config(AgentWorkerConfig {
            api_url: api_url.to_string(),
            admin_token: Some(admin_token),
            node_name: Some(node_name),
            node_id: None,
            node_token: None,
        })
    }

    pub fn from_config(config: AgentWorkerConfig) -> Result<Self, WorkerError> {
        Self::from_config_with_site_operations(config, Box::new(NginxHelperClient::default()))
    }

    /// Build a worker with an explicit site runtime. Production defaults use a
    /// bounded local RPC to the privileged Nginx helper; fixtures can inject an
    /// isolated Nginx adapter or a unit fake through the same boundary.
    pub fn from_config_with_site_operations(
        config: AgentWorkerConfig,
        site_operations: Box<dyn ManagedSiteOperations>,
    ) -> Result<Self, WorkerError> {
        let api = ApiClient::new(&config.api_url)?;
        let node = match (config.node_id, config.node_token) {
            (Some(node_id), Some(token)) if is_uuid(&node_id) && !token.trim().is_empty() => {
                Some(NodeCredentials { node_id, token })
            }
            (None, None) => None,
            _ => return Err(WorkerError::InvalidConfiguration),
        };

        let (admin_token, node_name) = match node {
            Some(_) if config.admin_token.is_some() => {
                // Do not retain a global credential in a remote agent process.
                return Err(WorkerError::InvalidConfiguration);
            }
            Some(_) => (None, None),
            None if !api.is_local() => {
                // Remote agents may not bootstrap with the control-plane admin token.
                return Err(WorkerError::InvalidConfiguration);
            }
            None => {
                let admin_token = config
                    .admin_token
                    .filter(|token| !token.trim().is_empty())
                    .ok_or(WorkerError::InvalidConfiguration)?;
                let node_name = config
                    .node_name
                    .filter(|name| !name.trim().is_empty())
                    .ok_or(WorkerError::InvalidConfiguration)?;
                (Some(admin_token), Some(node_name))
            }
        };

        Ok(Self {
            api,
            admin_token,
            node_name,
            node,
            metrics: MetricsCollector::default(),
            telemetry_metrics: MetricsCollector::default(),
            site_operations,
            poll_interval: POLL_INTERVAL,
            heartbeat_interval: HEARTBEAT_INTERVAL,
        })
    }

    async fn register(&mut self) -> Result<(), WorkerError> {
        if self.node.is_some() {
            return Ok(());
        }
        let admin_token = self
            .admin_token
            .as_deref()
            .ok_or(WorkerError::InvalidConfiguration)?;
        let node_name = self
            .node_name
            .as_deref()
            .ok_or(WorkerError::InvalidConfiguration)?;
        let node = self.api.register(admin_token, node_name).await?;
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

    async fn report_metrics(&mut self) -> Result<(), WorkerError> {
        let sample = match self.telemetry_metrics.collect() {
            Ok(sample) => sample,
            Err(_) => {
                eprintln!("Agent metrics collection failed; retrying");
                return Ok(());
            }
        };
        let node = self
            .node
            .as_ref()
            .ok_or(WorkerError::InvalidConfiguration)?;
        self.api.publish_metrics(node, &sample).await
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
        let mut extra_secret: Option<String> = None;
        let outcome = match parse_operation(&job.payload) {
            Ok(TypedOperation::TlsApply {
                site_id,
                domain,
                spec,
                source:
                    fusionpanel_agent_protocol::TlsCertificateSource::AcmeHttp01 {
                        acme_directory_url,
                        contact_email,
                    },
            }) => {
                match crate::acme::issue_http01_certificate(
                    &acme_directory_url,
                    &domain,
                    contact_email.as_deref(),
                )
                .await
                {
                    Ok((cert_pem, key_pem)) => {
                        extra_secret = Some(key_pem.clone());
                        match self
                            .site_operations
                            .apply_site_tls(site_id, &domain, &spec, &cert_pem, &key_pem)
                        {
                            Ok(()) => OperationOutcome::success(format!(
                                "tls.apply completed for {domain}"
                            )),
                            Err(error) => {
                                OperationOutcome::failure(site_apply_failure_message(&error))
                            }
                        }
                    }
                    Err(err) => {
                        eprintln!("ACME HTTP-01 issuance failed for {domain}: {err}");
                        OperationOutcome::failure("tls.apply failed during ACME HTTP-01 issuance")
                    }
                }
            }
            Ok(TypedOperation::DatabaseApply { name, engine }) => {
                match RealDatabaseDriver.apply_database(engine, &name).await {
                    Ok(()) => {
                        OperationOutcome::success(format!("database.apply completed for {name}"))
                    }
                    Err(err) => {
                        eprintln!("database.apply failed for {name}: {err}");
                        OperationOutcome::failure("database.apply failed")
                    }
                }
            }
            Ok(TypedOperation::DatabaseDelete { name, engine }) => {
                match RealDatabaseDriver.delete_database(engine, &name).await {
                    Ok(()) => {
                        OperationOutcome::success(format!("database.delete completed for {name}"))
                    }
                    Err(err) => {
                        eprintln!("database.delete failed for {name}: {err}");
                        OperationOutcome::failure("database.delete failed")
                    }
                }
            }
            Ok(TypedOperation::DatabaseUserCreate {
                database_name,
                engine,
                username,
                password,
            }) => {
                extra_secret = Some(password.clone());
                match RealDatabaseDriver
                    .create_user(engine, &database_name, &username, &password)
                    .await
                {
                    Ok(()) => OperationOutcome::success(format!(
                        "database.user.create completed for {username}@{database_name}"
                    )),
                    Err(err) => {
                        let safe_err = sanitize_bounded(&err, &[&password]);
                        eprintln!(
                            "database.user.create failed for {username}@{database_name}: {safe_err}"
                        );
                        OperationOutcome::failure("database.user.create failed")
                    }
                }
            }
            Ok(TypedOperation::BackupCreate(payload)) => {
                if let BackupDestination::S3 { ref secret_key, .. } = payload.destination {
                    extra_secret = Some(secret_key.clone());
                }
                match execute_backup_create(&payload).await {
                    Ok(sha256) => OperationOutcome::success(format!(
                        "backup.create completed for {} sha256={sha256}",
                        payload.backup_id
                    )),
                    Err(err) => {
                        let safe_err = match &extra_secret {
                            Some(sec) => sanitize_bounded(&err, &[sec.as_str()]),
                            None => err,
                        };
                        eprintln!("backup.create failed for {}: {safe_err}", payload.backup_id);
                        OperationOutcome::failure("backup.create failed")
                    }
                }
            }
            Ok(TypedOperation::BackupRestore(payload)) => {
                if let BackupDestination::S3 { ref secret_key, .. } = payload.destination {
                    extra_secret = Some(secret_key.clone());
                }
                match execute_backup_restore(&payload).await {
                    Ok(()) => OperationOutcome::success(format!(
                        "backup.restore completed for {}",
                        payload.backup_id
                    )),
                    Err(err) => {
                        let safe_err = match &extra_secret {
                            Some(sec) => sanitize_bounded(&err, &[sec.as_str()]),
                            None => err,
                        };
                        eprintln!(
                            "backup.restore failed for {}: {safe_err}",
                            payload.backup_id
                        );
                        OperationOutcome::failure("backup.restore failed")
                    }
                }
            }
            Ok(TypedOperation::DnsZoneApply(payload)) => {
                match execute_dns_zone_apply(&payload).await {
                    Ok(()) => OperationOutcome::success(format!(
                        "dns.zone.apply completed for {} (serial {})",
                        payload.domain, payload.serial
                    )),
                    Err(err) => {
                        eprintln!("dns.zone.apply failed for {}: {err}", payload.domain);
                        OperationOutcome::failure("dns.zone.apply failed")
                    }
                }
            }
            Ok(TypedOperation::DnsZoneDelete(payload)) => {
                match execute_dns_zone_delete(&payload).await {
                    Ok(()) => OperationOutcome::success(format!(
                        "dns.zone.delete completed for {}",
                        payload.domain
                    )),
                    Err(err) => {
                        eprintln!("dns.zone.delete failed for {}: {err}", payload.domain);
                        OperationOutcome::failure("dns.zone.delete failed")
                    }
                }
            }
            Ok(TypedOperation::DnsProbe(payload)) => match execute_dns_probe(&payload).await {
                Ok(summary) => OperationOutcome::success(summary),
                Err(err) => {
                    eprintln!("dns.probe failed for {}: {err}", payload.domain);
                    OperationOutcome::failure("dns.probe failed")
                }
            },
            Ok(TypedOperation::CronApply(payload)) => match execute_cron_apply(&payload).await {
                Ok(()) => OperationOutcome::success(format!(
                    "cron.apply completed for {} ({} jobs)",
                    payload.cron_id,
                    payload.jobs.len()
                )),
                Err(err) => {
                    eprintln!("cron.apply failed for {}: {err}", payload.cron_id);
                    OperationOutcome::failure("cron.apply failed")
                }
            },
            Ok(TypedOperation::CronDelete(payload)) => match execute_cron_delete(&payload).await {
                Ok(()) => OperationOutcome::success(format!(
                    "cron.delete completed for {} ({} remaining)",
                    payload.cron_id,
                    payload.jobs.len()
                )),
                Err(err) => {
                    eprintln!("cron.delete failed for {}: {err}", payload.cron_id);
                    OperationOutcome::failure("cron.delete failed")
                }
            },
            Ok(TypedOperation::CronRunNow(payload)) => match execute_cron_run_now(&payload).await {
                Ok(summary) => OperationOutcome::success(summary),
                Err(err) => {
                    eprintln!("cron.run_now failed for {}: {err}", payload.cron_id);
                    OperationOutcome::failure("cron.run_now failed")
                }
            },
            Ok(operation) => {
                if let TypedOperation::TlsApply {
                    source:
                        fusionpanel_agent_protocol::TlsCertificateSource::CustomPem {
                            ref private_key_pem,
                            ..
                        },
                    ..
                } = operation
                {
                    extra_secret = Some(private_key_pem.clone());
                }
                execute_operation(operation, &mut self.metrics, self.site_operations.as_ref())
            }
            Err(outcome) => outcome,
        };
        let mut secrets = vec![node.token.as_str()];
        if let Some(admin_token) = self.admin_token.as_deref() {
            secrets.push(admin_token);
        }
        if let Some(ref key_secret) = extra_secret {
            secrets.push(key_secret.as_str());
        }
        let log_text = sanitize_bounded(&outcome.log_text, &secrets);
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
        // Prime the CPU counters so the first published value is measured over
        // an interval instead of reporting the collector's initial zero.
        let _ = self.telemetry_metrics.collect();
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
                        match self.report_metrics().await {
                            Ok(()) => {}
                            Err(WorkerError::Unauthorized) => {
                                return Err(WorkerError::Unauthorized)
                            }
                            Err(_) => eprintln!("Agent metrics report failed; retrying"),
                        }
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
    let request: ProtocolRequest = serde_json::from_value(payload.clone())
        .map_err(|_| OperationOutcome::failure("Malformed agent operation request"))?;
    if request.version != 1 {
        return Err(OperationOutcome::failure(
            "Unsupported agent protocol version",
        ));
    }
    request
        .validate()
        .map_err(|_| OperationOutcome::failure("Agent operation input is invalid"))?;
    match request.operation {
        ProtocolOperation::SystemInfo(_) => Ok(TypedOperation::SystemInfo),
        ProtocolOperation::SiteValidate(payload) => Ok(TypedOperation::SiteValidate {
            domain: payload.domain,
            document_root: payload.document_root,
        }),
        ProtocolOperation::SiteApply(payload) => {
            let spec = match payload.spec {
                fusionpanel_agent_protocol::SiteSpec::Static { document_root } => {
                    ManagedSiteSpec::Static { document_root }
                }
                fusionpanel_agent_protocol::SiteSpec::ReverseProxy { upstream_url } => {
                    ManagedSiteSpec::ReverseProxy { upstream_url }
                }
            };
            Ok(TypedOperation::SiteApply {
                site_id: payload.site_id,
                domain: payload.domain,
                spec,
            })
        }
        ProtocolOperation::SiteDelete(payload) => Ok(TypedOperation::SiteDelete {
            site_id: payload.site_id,
            domain: payload.domain,
        }),
        ProtocolOperation::TlsApply(payload) => {
            let spec = match payload.spec {
                fusionpanel_agent_protocol::SiteSpec::Static { document_root } => {
                    ManagedSiteSpec::Static { document_root }
                }
                fusionpanel_agent_protocol::SiteSpec::ReverseProxy { upstream_url } => {
                    ManagedSiteSpec::ReverseProxy { upstream_url }
                }
            };
            Ok(TypedOperation::TlsApply {
                site_id: payload.site_id,
                domain: payload.domain,
                spec,
                source: payload.source,
            })
        }
        ProtocolOperation::TlsDelete(payload) => {
            let spec = match payload.spec {
                fusionpanel_agent_protocol::SiteSpec::Static { document_root } => {
                    ManagedSiteSpec::Static { document_root }
                }
                fusionpanel_agent_protocol::SiteSpec::ReverseProxy { upstream_url } => {
                    ManagedSiteSpec::ReverseProxy { upstream_url }
                }
            };
            Ok(TypedOperation::TlsDelete {
                site_id: payload.site_id,
                domain: payload.domain,
                spec,
            })
        }
        ProtocolOperation::DatabaseApply(payload) => Ok(TypedOperation::DatabaseApply {
            name: payload.name,
            engine: payload.engine,
        }),
        ProtocolOperation::DatabaseDelete(payload) => Ok(TypedOperation::DatabaseDelete {
            name: payload.name,
            engine: payload.engine,
        }),
        ProtocolOperation::DatabaseUserCreate(payload) => Ok(TypedOperation::DatabaseUserCreate {
            database_name: payload.database_name,
            engine: payload.engine,
            username: payload.username,
            password: payload.password,
        }),
        ProtocolOperation::BackupCreate(payload) => {
            Ok(TypedOperation::BackupCreate(Box::new(payload)))
        }
        ProtocolOperation::BackupRestore(payload) => {
            Ok(TypedOperation::BackupRestore(Box::new(payload)))
        }
        ProtocolOperation::DnsZoneApply(payload) => {
            Ok(TypedOperation::DnsZoneApply(Box::new(payload)))
        }
        ProtocolOperation::DnsZoneDelete(payload) => Ok(TypedOperation::DnsZoneDelete(payload)),
        ProtocolOperation::DnsProbe(payload) => Ok(TypedOperation::DnsProbe(payload)),
        ProtocolOperation::CronApply(payload) => Ok(TypedOperation::CronApply(payload)),
        ProtocolOperation::CronDelete(payload) => Ok(TypedOperation::CronDelete(payload)),
        ProtocolOperation::CronRunNow(payload) => Ok(TypedOperation::CronRunNow(payload)),
    }
}

fn execute_operation(
    operation: TypedOperation,
    metrics: &mut MetricsCollector,
    site_operations: &dyn ManagedSiteOperations,
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
        TypedOperation::SiteApply {
            site_id,
            domain,
            spec,
        } => match site_operations.apply_site(site_id, &domain, &spec) {
            Ok(()) => OperationOutcome::success(format!("site.apply completed for {domain}")),
            Err(error) => OperationOutcome::failure(site_apply_failure_message(&error)),
        },
        TypedOperation::SiteDelete { site_id, domain } => {
            match site_operations.delete_site(site_id, &domain) {
                Ok(()) => OperationOutcome::success(format!("site.delete completed for {domain}")),
                Err(error) => OperationOutcome::failure(site_delete_failure_message(&error)),
            }
        }
        TypedOperation::TlsApply {
            site_id,
            domain,
            spec,
            source,
        } => match source {
            fusionpanel_agent_protocol::TlsCertificateSource::CustomPem {
                certificate_pem,
                private_key_pem,
            } => match site_operations.apply_site_tls(
                site_id,
                &domain,
                &spec,
                &certificate_pem,
                &private_key_pem,
            ) {
                Ok(()) => OperationOutcome::success(format!("tls.apply completed for {domain}")),
                Err(error) => OperationOutcome::failure(site_apply_failure_message(&error)),
            },
            fusionpanel_agent_protocol::TlsCertificateSource::AcmeHttp01 { .. } => {
                OperationOutcome::failure("tls.apply ACME HTTP-01 requires async worker execution")
            }
        },
        TypedOperation::TlsDelete {
            site_id,
            domain,
            spec,
        } => match site_operations.delete_site_tls(site_id, &domain, &spec) {
            Ok(()) => OperationOutcome::success(format!("tls.delete completed for {domain}")),
            Err(error) => OperationOutcome::failure(site_delete_failure_message(&error)),
        },
        TypedOperation::DatabaseApply { .. }
        | TypedOperation::DatabaseDelete { .. }
        | TypedOperation::DatabaseUserCreate { .. }
        | TypedOperation::BackupCreate(_)
        | TypedOperation::BackupRestore(_)
        | TypedOperation::DnsZoneApply(_)
        | TypedOperation::DnsZoneDelete(_)
        | TypedOperation::DnsProbe(_)
        | TypedOperation::CronApply(_)
        | TypedOperation::CronDelete(_)
        | TypedOperation::CronRunNow(_) => {
            OperationOutcome::failure("async operation requires async worker execution")
        }
    }
}

fn site_apply_failure_message(error: &ManagedSiteError) -> &'static str {
    if matches!(
        error,
        ManagedSiteError::RollbackFailed | ManagedSiteError::OutcomeUncertain
    ) {
        "site.apply failed; managed config state is uncertain and manual Nginx recovery may be required"
    } else {
        "site.apply failed; managed config was preserved or rolled back"
    }
}

fn site_delete_failure_message(error: &ManagedSiteError) -> &'static str {
    if matches!(
        error,
        ManagedSiteError::RollbackFailed | ManagedSiteError::OutcomeUncertain
    ) {
        "site.delete failed; managed config state is uncertain and manual Nginx recovery may be required"
    } else {
        "site.delete failed; managed config was preserved or rolled back"
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
    use std::sync::Mutex;

    #[derive(Default)]
    struct RecordingSiteOperations {
        calls: Mutex<Vec<String>>,
        fail: bool,
    }

    impl ManagedSiteOperations for RecordingSiteOperations {
        fn apply_site(
            &self,
            site_id: Uuid,
            domain: &str,
            spec: &ManagedSiteSpec,
        ) -> Result<(), hosting::ManagedSiteError> {
            self.calls
                .lock()
                .unwrap()
                .push(format!("apply:{site_id}:{domain}:{spec:?}"));
            if self.fail {
                Err(hosting::ManagedSiteError::ReloadFailed)
            } else {
                Ok(())
            }
        }

        fn delete_site(
            &self,
            site_id: Uuid,
            domain: &str,
        ) -> Result<(), hosting::ManagedSiteError> {
            self.calls
                .lock()
                .unwrap()
                .push(format!("delete:{site_id}:{domain}"));
            if self.fail {
                Err(hosting::ManagedSiteError::ReloadFailed)
            } else {
                Ok(())
            }
        }
    }

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
    fn site_apply_and_delete_are_dispatched_to_the_hosting_adapter() {
        let site_id = "00000000-0000-4000-8000-000000000001";
        let apply = parse_operation(&valid_job(
            "site.apply",
            json!({
                "site_id": site_id,
                "domain": "static.example.test",
                "spec": {
                    "kind": "static",
                    "document_root": "/srv/fusionpanel/sites/static.example.test"
                }
            }),
        ))
        .unwrap();
        let delete = parse_operation(&valid_job(
            "site.delete",
            json!({ "site_id": site_id, "domain": "static.example.test" }),
        ))
        .unwrap();
        let adapter = RecordingSiteOperations::default();
        let mut metrics = MetricsCollector::default();

        assert!(execute_operation(apply, &mut metrics, &adapter).succeeded);
        assert!(execute_operation(delete, &mut metrics, &adapter).succeeded);
        let calls = adapter.calls.lock().unwrap();
        assert_eq!(calls.len(), 2);
        assert!(
            calls[0].starts_with("apply:00000000-0000-4000-8000-000000000001:static.example.test:")
        );
        assert_eq!(
            calls[1],
            "delete:00000000-0000-4000-8000-000000000001:static.example.test"
        );
    }

    #[test]
    fn site_apply_rejects_malformed_and_unsupported_payloads_before_dispatch() {
        let adapter = RecordingSiteOperations::default();
        for payload in [
            valid_job(
                "site.apply",
                json!({
                    "site_id": "00000000-0000-4000-8000-000000000001",
                    "domain": "example.test",
                    "spec": { "kind": "php", "version": "8.2" }
                }),
            ),
            valid_job(
                "site.apply",
                json!({
                    "site_id": "00000000-0000-4000-8000-000000000001",
                    "domain": "example.test",
                    "spec": {
                        "kind": "static",
                        "document_root": "/srv/fusionpanel/sites/example.test",
                        "unexpected": true
                    }
                }),
            ),
            valid_job("executor.run", json!({ "cmd": "echo unsafe" })),
        ] {
            assert!(parse_operation(&payload).is_err());
        }
        assert!(adapter.calls.lock().unwrap().is_empty());
    }

    #[test]
    fn site_adapter_failures_are_reported_without_raw_backend_errors() {
        let operation = parse_operation(&valid_job(
            "site.apply",
            json!({
                "site_id": "00000000-0000-4000-8000-000000000001",
                "domain": "example.test",
                "spec": {
                    "kind": "reverse_proxy",
                    "upstream_url": "http://127.0.0.1:8080"
                }
            }),
        ))
        .unwrap();
        let adapter = RecordingSiteOperations {
            fail: true,
            ..RecordingSiteOperations::default()
        };
        let mut metrics = MetricsCollector::default();
        let outcome = execute_operation(operation, &mut metrics, &adapter);
        assert!(!outcome.succeeded);
        assert_eq!(
            outcome.error_text.as_deref(),
            Some("site.apply failed; managed config was preserved or rolled back")
        );
        assert!(!outcome.log_text.contains("ReloadFailed"));
    }

    #[test]
    fn rollback_and_rpc_uncertainty_are_not_reported_as_successful_restoration() {
        assert!(
            site_apply_failure_message(&ManagedSiteError::RollbackFailed)
                .contains("state is uncertain")
        );
        assert!(
            site_delete_failure_message(&ManagedSiteError::OutcomeUncertain)
                .contains("manual Nginx recovery")
        );
        assert!(site_apply_failure_message(&ManagedSiteError::ReloadFailed)
            .contains("preserved or rolled back"));
    }

    #[test]
    fn site_validation_rejects_unicode_ip_and_path_traversal() {
        let valid_root = "/srv/fusionpanel/sites/example.test";
        for domain in ["127.0.0.1", "éxample.test"] {
            assert!(parse_operation(&valid_job(
                "site.validate",
                json!({ "domain": domain, "document_root": valid_root })
            ))
            .is_err());
        }
        assert!(parse_operation(&valid_job(
            "site.validate",
            json!({
                "domain": "example.test",
                "document_root": "/srv/fusionpanel/sites/../etc"
            })
        ))
        .is_err());
        assert!(parse_operation(&valid_job(
            "site.validate",
            json!({ "domain": "example.test", "document_root": valid_root })
        ))
        .is_ok());
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
    fn metrics_report_uses_node_scoped_endpoint_and_bounded_sample_fields() {
        let api = ApiClient::new("https://control.example.test").unwrap();
        let node = NodeCredentials {
            node_id: "00000000-0000-4000-8000-000000000001".to_string(),
            token: "node-secret".to_string(),
        };
        let metrics = SystemMetrics {
            cpu_usage_percent: 42.5,
            memory_used_kb: 2048,
            memory_available_kb: 4096,
            disk_io_ops: 12,
        };
        let request = api
            .json_request(
                Method::POST,
                &format!("v1/nodes/{}/metrics", node.node_id),
                &node.token,
                &metrics,
            )
            .unwrap();

        assert_eq!(request.method(), Method::POST);
        assert_eq!(
            request.url().path(),
            "/v1/nodes/00000000-0000-4000-8000-000000000001/metrics"
        );
        assert_eq!(
            request
                .headers()
                .get(reqwest::header::AUTHORIZATION)
                .unwrap(),
            "Bearer node-secret"
        );
        let body: Value =
            serde_json::from_slice(request.body().and_then(reqwest::Body::as_bytes).unwrap())
                .unwrap();
        assert_eq!(body["cpu_usage_percent"], 42.5);
        assert_eq!(body["memory_used_kb"], 2048);
        assert_eq!(body["memory_available_kb"], 4096);
        assert_eq!(body["disk_io_ops"], 12);
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
    fn bootstrap_policy_keeps_admin_registration_on_loopback_only() {
        let local = AgentWorker::from_config(AgentWorkerConfig {
            api_url: "http://127.0.0.1:8080".to_string(),
            admin_token: Some("local-admin-secret".to_string()),
            node_name: Some("local-node".to_string()),
            node_id: None,
            node_token: None,
        })
        .unwrap();
        assert_eq!(local.admin_token.as_deref(), Some("local-admin-secret"));
        assert!(local.node.is_none());

        for (admin_token, node_id, node_token) in [
            (None, None, None),
            (Some("admin-secret".to_string()), None, None),
            (
                Some("admin-secret".to_string()),
                Some("00000000-0000-4000-8000-000000000001".to_string()),
                Some("node-secret".to_string()),
            ),
            (
                None,
                Some("not-a-uuid".to_string()),
                Some("node-secret".to_string()),
            ),
            (
                None,
                Some("00000000-0000-4000-8000-000000000001".to_string()),
                None,
            ),
        ] {
            assert_eq!(
                AgentWorker::from_config(AgentWorkerConfig {
                    api_url: "https://agent-control.example.test".to_string(),
                    admin_token,
                    node_name: Some("remote-node".to_string()),
                    node_id,
                    node_token,
                })
                .err(),
                Some(WorkerError::InvalidConfiguration)
            );
        }
    }

    #[test]
    fn remote_bootstrap_uses_only_the_preprovisioned_node_token() {
        let node_id = "00000000-0000-4000-8000-000000000001";
        let worker = AgentWorker::from_config(AgentWorkerConfig {
            api_url: "https://agent-control.example.test".to_string(),
            admin_token: None,
            node_name: None,
            node_id: Some(node_id.to_string()),
            node_token: Some("scoped-node-secret".to_string()),
        })
        .unwrap();

        assert!(worker.admin_token.is_none());
        assert_eq!(worker.node.as_ref().unwrap().node_id, node_id);
        assert_eq!(worker.node.as_ref().unwrap().token, "scoped-node-secret");

        let request = worker
            .api
            .json_request(
                Method::POST,
                "v1/jobs/claim",
                &worker.node.as_ref().unwrap().token,
                &ClaimRequest { node_id },
            )
            .unwrap();
        assert_eq!(request.url().path(), "/v1/jobs/claim");
        assert_eq!(
            request
                .headers()
                .get(reqwest::header::AUTHORIZATION)
                .unwrap(),
            "Bearer scoped-node-secret"
        );

        // Pre-provisioned remote credentials bypass self-registration after startup.
        assert!(worker.node.is_some());
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
