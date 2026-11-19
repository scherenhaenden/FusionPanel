#![forbid(unsafe_code)]

use axum::{
    extract::{DefaultBodyLimit, Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::{delete, get},
    Json, Router,
};
use chrono::Utc;
use observability::{
    alert_store::{AlertStore, AlertStoreError, NewAlertRule},
    webhook_signer::WebhookSigner,
};
use serde::Deserialize;
use serde_json::json;
use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::Arc,
    time::Duration,
};
use tokio::{sync::Mutex, time::sleep};
use uuid::Uuid;

use super::{metrics::NodeMetricSample, node_jobs, AppState, ErrorDetail, ErrorResponse};

const MAX_RULES_BODY_BYTES: usize = 8 * 1024;
const MAX_WEBHOOK_SECRET_BYTES: usize = 1024;
const MAX_DELIVERY_ATTEMPTS: u32 = 3;
const WEBHOOK_TIMEOUT: Duration = Duration::from_secs(2);

pub(super) fn test_store() -> Arc<Mutex<AlertStore>> {
    let encryption_key = [0xB7; 32];
    Arc::new(Mutex::new(
        AlertStore::in_memory(Some(&encryption_key)).expect("in-memory alert store"),
    ))
}

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route("/v1/alerts/rules", get(list_rules).post(create_rule))
        .route("/v1/alerts/rules/{id}", delete(delete_rule))
        .route("/v1/alerts/active", get(active_alerts))
        .route("/v1/alerts/deliveries", get(list_deliveries))
        .layer(DefaultBodyLimit::max(MAX_RULES_BODY_BYTES))
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateAlertRuleRequest {
    name: String,
    metric: String,
    operator: String,
    threshold: f64,
    severity: String,
    webhook_url: String,
    webhook_secret: String,
    #[serde(default = "default_enabled")]
    enabled: bool,
}

fn default_enabled() -> bool {
    true
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DeliveryQuery {
    limit: Option<u32>,
}

async fn list_rules(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ErrorResponse> {
    require_operator(&state, &headers).await?;
    let rules = state
        .alerts_store
        .lock()
        .await
        .list_rules()
        .map_err(map_store_error)?;
    Ok(Json(json!({ "rules": rules })))
}

async fn create_rule(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<CreateAlertRuleRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>), ErrorResponse> {
    require_operator(&state, &headers).await?;
    validate_rule(&request)?;
    validate_webhook_url(&request.webhook_url)?;

    let now = Utc::now().to_rfc3339();
    let rule = NewAlertRule {
        name: request.name.trim().to_string(),
        metric: request.metric,
        operator: request.operator,
        threshold: request.threshold,
        severity: request.severity,
        webhook_url: request.webhook_url,
        enabled: request.enabled,
    };
    let stored = state
        .alerts_store
        .lock()
        .await
        .create_rule(&rule, &request.webhook_secret, &now)
        .map_err(map_store_error)?;
    Ok((StatusCode::CREATED, Json(json!({ "rule": stored }))))
}

async fn delete_rule(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<StatusCode, ErrorResponse> {
    require_operator(&state, &headers).await?;
    Uuid::parse_str(&id).map_err(|_| node_jobs::bad_request())?;
    let deleted = state
        .alerts_store
        .lock()
        .await
        .delete_rule(&id)
        .map_err(map_store_error)?;
    if deleted {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ErrorResponse {
            error: ErrorDetail {
                code: "not_found",
                message: "Alert rule not found",
            },
        })
    }
}

async fn active_alerts(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ErrorResponse> {
    require_operator(&state, &headers).await?;
    let active = state
        .alerts_store
        .lock()
        .await
        .active_states()
        .map_err(map_store_error)?;
    Ok(Json(json!({ "active": active })))
}

async fn list_deliveries(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<DeliveryQuery>,
) -> Result<Json<serde_json::Value>, ErrorResponse> {
    require_operator(&state, &headers).await?;
    let deliveries = state
        .alerts_store
        .lock()
        .await
        .deliveries(query.limit.unwrap_or(50))
        .map_err(map_store_error)?;
    Ok(Json(json!({ "deliveries": deliveries })))
}

async fn require_operator(state: &AppState, headers: &HeaderMap) -> Result<(), ErrorResponse> {
    if node_jobs::authorized_operator(state, headers).await {
        Ok(())
    } else {
        Err(node_jobs::unauthorized())
    }
}

fn validate_rule(request: &CreateAlertRuleRequest) -> Result<(), ErrorResponse> {
    let name = request.name.trim();
    if name.is_empty() || name.len() > 128 || name.chars().any(char::is_control) {
        return Err(node_jobs::bad_request());
    }
    if !matches!(
        request.metric.as_str(),
        "cpu_usage_percent" | "memory_used_percent" | "disk_io_ops"
    ) || !matches!(request.operator.as_str(), "above" | "below")
        || !matches!(request.severity.as_str(), "info" | "warning" | "critical")
        || !request.threshold.is_finite()
    {
        return Err(node_jobs::bad_request());
    }
    if matches!(
        request.metric.as_str(),
        "cpu_usage_percent" | "memory_used_percent"
    ) && !(0.0..=100.0).contains(&request.threshold)
    {
        return Err(node_jobs::bad_request());
    }
    if request.metric == "disk_io_ops" && !(0.0..=1.0e15).contains(&request.threshold) {
        return Err(node_jobs::bad_request());
    }
    if request.webhook_secret.trim().is_empty()
        || request.webhook_secret.len() < 32
        || request.webhook_secret.len() > MAX_WEBHOOK_SECRET_BYTES
    {
        return Err(node_jobs::bad_request());
    }
    Ok(())
}

fn validate_webhook_url(raw_url: &str) -> Result<reqwest::Url, ErrorResponse> {
    if raw_url.len() > 2048 || raw_url.chars().any(char::is_control) {
        return Err(node_jobs::bad_request());
    }
    let url = reqwest::Url::parse(raw_url).map_err(|_| node_jobs::bad_request())?;
    if !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(node_jobs::bad_request());
    }
    let host = url.host_str().ok_or_else(node_jobs::bad_request)?;
    match url.scheme() {
        "https" => Ok(url),
        "http" if is_loopback_host(host) => Ok(url),
        _ => Err(node_jobs::bad_request()),
    }
}

fn is_loopback_host(host: &str) -> bool {
    host.eq_ignore_ascii_case("localhost")
        || host
            .parse::<IpAddr>()
            .is_ok_and(|address| address.is_loopback())
}

fn compare(value: f64, operator: &str, threshold: f64) -> bool {
    match operator {
        "above" => value > threshold,
        "below" => value < threshold,
        _ => false,
    }
}

fn metric_value(sample: &NodeMetricSample, metric: &str) -> Option<f64> {
    match metric {
        "cpu_usage_percent" => Some(sample.cpu_usage_percent),
        "memory_used_percent" => {
            let total = sample
                .memory_used_kb
                .checked_add(sample.memory_available_kb)?;
            (total > 0).then(|| sample.memory_used_kb as f64 / total as f64 * 100.0)
        }
        "disk_io_ops" => Some(sample.disk_io_ops as f64),
        _ => None,
    }
}

pub(super) async fn evaluate_sample(
    state: &AppState,
    sample: &NodeMetricSample,
) -> Result<(), ErrorResponse> {
    let rules = match state.alerts_store.lock().await.rules_for_evaluation() {
        Ok(rules) => rules,
        Err(AlertStoreError::SecretKeyUnavailable) => return Ok(()),
        Err(error) => return Err(map_store_error(error)),
    };

    for rule in rules {
        let Some(value) = metric_value(sample, &rule.rule.metric) else {
            continue;
        };
        let triggered = compare(value, &rule.rule.operator, rule.rule.threshold);
        let transition_at = Utc::now().to_rfc3339();
        let transition = state
            .alerts_store
            .lock()
            .await
            .transition(
                &rule.rule.id,
                &sample.node_id,
                triggered,
                value,
                &transition_at,
            )
            .map_err(map_store_error)?;
        let Some(transition) = transition else {
            continue;
        };

        let delivery_id = state
            .alerts_store
            .lock()
            .await
            .create_delivery(
                &rule.rule.id,
                &sample.node_id,
                transition.as_str(),
                &transition_at,
            )
            .map_err(map_store_error)?;
        let payload = json!({
            "event": transition.as_str(),
            "rule_id": rule.rule.id,
            "rule_name": rule.rule.name,
            "metric": rule.rule.metric,
            "operator": rule.rule.operator,
            "threshold": rule.rule.threshold,
            "severity": rule.rule.severity,
            "node_id": sample.node_id,
            "value": value,
            "recorded_at": sample.recorded_at,
        });
        let payload = serde_json::to_string(&payload).map_err(|_| node_jobs::internal_error())?;
        let signed_at = Utc::now().timestamp().max(0) as u64;
        let signer = WebhookSigner::new(rule.secret.as_bytes().to_vec(), 300);
        let signature = signer.sign(&payload, signed_at);
        let outcome = deliver_webhook(&rule.rule.webhook_url, &payload, &signature).await;
        state
            .alerts_store
            .lock()
            .await
            .finish_delivery(
                &delivery_id,
                outcome.status,
                outcome.attempt_count,
                outcome.http_status,
                outcome.error_code,
                &Utc::now().to_rfc3339(),
            )
            .map_err(map_store_error)?;
    }
    Ok(())
}

struct DeliveryOutcome {
    status: &'static str,
    attempt_count: u32,
    http_status: Option<u16>,
    error_code: Option<&'static str>,
}

async fn deliver_webhook(raw_url: &str, payload: &str, signature: &str) -> DeliveryOutcome {
    let url = match validate_webhook_url(raw_url) {
        Ok(url) => url,
        Err(_) => {
            return DeliveryOutcome {
                status: "failed",
                attempt_count: 0,
                http_status: None,
                error_code: Some("invalid_target"),
            };
        }
    };
    let Some(addresses) = safe_target_addresses(&url).await else {
        return DeliveryOutcome {
            status: "failed",
            attempt_count: 0,
            http_status: None,
            error_code: Some("blocked_target"),
        };
    };
    let host = url.host_str().unwrap_or_default();
    let mut builder = reqwest::Client::builder()
        .timeout(WEBHOOK_TIMEOUT)
        .redirect(reqwest::redirect::Policy::none())
        .no_proxy();
    if !host.parse::<IpAddr>().is_ok() {
        if let Some(address) = addresses.first() {
            builder = builder.resolve(host, *address);
        }
    }
    let client = match builder.build() {
        Ok(client) => client,
        Err(_) => {
            return DeliveryOutcome {
                status: "failed",
                attempt_count: 0,
                http_status: None,
                error_code: Some("client_config"),
            };
        }
    };

    let mut last_status = None;
    let mut last_error = None;
    for attempt in 1..=MAX_DELIVERY_ATTEMPTS {
        let response = client
            .post(url.clone())
            .header("content-type", "application/json")
            .header("x-fusionpanel-signature", signature)
            .body(payload.to_string())
            .send()
            .await;
        match response {
            Ok(response) if response.status().is_success() => {
                return DeliveryOutcome {
                    status: "delivered",
                    attempt_count: attempt,
                    http_status: Some(response.status().as_u16()),
                    error_code: None,
                };
            }
            Ok(response) => {
                let status = response.status();
                last_status = Some(status.as_u16());
                last_error = Some("http_status");
                if status.is_client_error() && status != reqwest::StatusCode::TOO_MANY_REQUESTS {
                    return DeliveryOutcome {
                        status: "failed",
                        attempt_count: attempt,
                        http_status: last_status,
                        error_code: last_error,
                    };
                }
            }
            Err(error) => {
                last_error = Some(if error.is_timeout() {
                    "timeout"
                } else {
                    "transport_error"
                });
            }
        }
        if attempt < MAX_DELIVERY_ATTEMPTS {
            sleep(Duration::from_millis(100 * (1 << (attempt - 1)))).await;
        }
    }
    DeliveryOutcome {
        status: "failed",
        attempt_count: MAX_DELIVERY_ATTEMPTS,
        http_status: last_status,
        error_code: last_error,
    }
}

async fn safe_target_addresses(url: &reqwest::Url) -> Option<Vec<SocketAddr>> {
    let host = url.host_str()?;
    let port = url.port_or_known_default()?;
    let loopback_target = is_loopback_host(host);
    let addresses = if let Ok(ip) = host.parse::<IpAddr>() {
        vec![SocketAddr::new(ip, port)]
    } else {
        tokio::net::lookup_host((host, port))
            .await
            .ok()?
            .collect::<Vec<_>>()
    };
    if addresses.is_empty()
        || addresses.iter().any(|address| {
            if loopback_target {
                !address.ip().is_loopback()
            } else {
                !is_public_address(address.ip())
            }
        })
    {
        return None;
    }
    Some(addresses)
}

fn is_public_address(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(address) => is_public_ipv4(address),
        IpAddr::V6(address) => {
            if let Some(mapped) = address.to_ipv4_mapped() {
                is_public_ipv4(mapped)
            } else {
                !address.is_loopback()
                    && !address.is_unspecified()
                    && !address.is_multicast()
                    && !address.is_unique_local()
                    && !address.is_unicast_link_local()
            }
        }
    }
}

fn is_public_ipv4(address: Ipv4Addr) -> bool {
    let octets = address.octets();
    !(address.is_private()
        || address.is_loopback()
        || address.is_link_local()
        || address.is_unspecified()
        || address.is_multicast()
        || address.is_broadcast()
        || octets[0] == 0
        || (octets[0] == 100 && (64..=127).contains(&octets[1]))
        || (octets[0] == 192 && octets[1] == 0)
        || (octets[0] == 198 && (18..=19).contains(&octets[1]))
        || (octets[0] >= 240))
}

fn map_store_error(error: AlertStoreError) -> ErrorResponse {
    match error {
        AlertStoreError::SecretKeyUnavailable => ErrorResponse {
            error: ErrorDetail {
                code: "service_unavailable",
                message: "Webhook encryption key is not configured",
            },
        },
        AlertStoreError::RuleLimitReached => ErrorResponse {
            error: ErrorDetail {
                code: "conflict",
                message: "Alert rule limit reached",
            },
        },
        _ => node_jobs::internal_error(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::Request};
    use http_body_util::BodyExt;
    use std::sync::Arc;
    use tokio::sync::Mutex;
    use tokio::{
        io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
        net::TcpListener,
    };
    use tower::ServiceExt;

    fn test_state() -> AppState {
        AppState {
            admin_token: Arc::new("test-admin-token".to_string()),
            identity_store: Arc::new(Mutex::new(
                identity::SqliteIdentityStore::in_memory().unwrap(),
            )),
            control_store: Arc::new(Mutex::new(
                fusionpanel_control_store::ControlStore::new_in_memory().unwrap(),
            )),
            jobs_store: Arc::new(Mutex::new(fusionpanel_jobs::JobStore::in_memory().unwrap())),
            metrics_store: crate::metrics::shared_store(),
            alerts_store: test_store(),
            frontend_dir: None,
        }
    }

    fn request_body(secret: &str) -> String {
        json!({
            "name": "High CPU",
            "metric": "cpu_usage_percent",
            "operator": "above",
            "threshold": 85.0,
            "severity": "critical",
            "webhook_url": "https://alerts.example.test/hooks/fusionpanel",
            "webhook_secret": secret,
        })
        .to_string()
    }

    #[tokio::test]
    async fn alert_rules_require_operator_auth_and_never_return_the_secret() {
        let router = super::super::app(test_state());
        let unauthorized = router
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/v1/alerts/rules")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);

        let secret = format!("{}{}", Uuid::new_v4(), Uuid::new_v4());
        let created = router
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/alerts/rules")
                    .header("authorization", "Bearer test-admin-token")
                    .header("content-type", "application/json")
                    .body(Body::from(request_body(&secret)))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(created.status(), StatusCode::CREATED);
        let response_text = String::from_utf8(
            created
                .into_body()
                .collect()
                .await
                .unwrap()
                .to_bytes()
                .to_vec(),
        )
        .unwrap();
        assert!(!response_text.contains(&secret));
        let response: serde_json::Value = serde_json::from_str(&response_text).unwrap();
        assert_eq!(response["rule"]["has_secret"], true);

        let listed = router
            .oneshot(
                Request::builder()
                    .uri("/v1/alerts/rules")
                    .header("authorization", "Bearer test-admin-token")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(listed.status(), StatusCode::OK);
        let list_text = String::from_utf8(
            listed
                .into_body()
                .collect()
                .await
                .unwrap()
                .to_bytes()
                .to_vec(),
        )
        .unwrap();
        assert!(!list_text.contains(&secret));
        let rules: serde_json::Value = serde_json::from_str(&list_text).unwrap();
        assert_eq!(rules["rules"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn rules_reject_unbounded_thresholds_and_unsafe_webhook_urls() {
        let invalid = CreateAlertRuleRequest {
            name: "CPU".to_string(),
            metric: "cpu_usage_percent".to_string(),
            operator: "above".to_string(),
            threshold: 101.0,
            severity: "critical".to_string(),
            webhook_url: "https://alerts.example.test/hook".to_string(),
            webhook_secret: "x".repeat(40),
            enabled: true,
        };
        assert!(validate_rule(&invalid).is_err());
        assert!(validate_webhook_url("http://203.0.113.10/hook").is_err());
        assert!(validate_webhook_url("https://user:pass@alerts.example.test/hook").is_err());
        assert!(validate_webhook_url("https://alerts.example.test/hook?token=hidden").is_err());
        assert!(validate_webhook_url("http://127.0.0.1:9000/hook").is_ok());
        assert!(validate_webhook_url("https://alerts.example.test/hook").is_ok());
    }

    #[test]
    fn alert_metric_values_are_derived_from_live_samples() {
        let sample = NodeMetricSample {
            node_id: "node-1".to_string(),
            cpu_usage_percent: 25.0,
            memory_used_kb: 300,
            memory_available_kb: 700,
            disk_io_ops: 77,
            recorded_at: "2026-11-19T10:00:00Z".to_string(),
        };
        assert_eq!(metric_value(&sample, "cpu_usage_percent"), Some(25.0));
        assert_eq!(metric_value(&sample, "memory_used_percent"), Some(30.0));
        assert_eq!(metric_value(&sample, "disk_io_ops"), Some(77.0));
        assert!(compare(91.0, "above", 85.0));
        assert!(!compare(50.0, "above", 85.0));
    }

    #[tokio::test]
    async fn webhook_delivery_retries_transient_failures_three_times_and_sends_signature() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let mut requests = Vec::new();
            for _ in 0..MAX_DELIVERY_ATTEMPTS {
                let (stream, _) = listener.accept().await.unwrap();
                let mut reader = BufReader::new(stream);
                let mut headers = String::new();
                loop {
                    let mut line = String::new();
                    reader.read_line(&mut line).await.unwrap();
                    if line == "\r\n" {
                        break;
                    }
                    headers.push_str(&line);
                }
                let body_length = headers
                    .lines()
                    .find_map(|line| {
                        let (name, value) = line.split_once(':')?;
                        name.eq_ignore_ascii_case("content-length")
                            .then(|| value.trim().parse::<usize>().ok())
                            .flatten()
                    })
                    .unwrap_or_default();
                let mut body = vec![0; body_length];
                reader.read_exact(&mut body).await.unwrap();
                requests.push((headers, String::from_utf8(body).unwrap()));
                reader
                    .get_mut()
                    .write_all(
                        b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                    )
                    .await
                    .unwrap();
            }
            requests
        });

        let secret = b"test webhook secret for signing";
        let payload = r#"{"event":"triggered","node_id":"node-1"}"#;
        let signed_at = Utc::now().timestamp().max(0) as u64;
        let signer = WebhookSigner::new(secret.to_vec(), 300);
        let signature = signer.sign(payload, signed_at);
        let outcome = deliver_webhook(&format!("http://{address}/hook"), payload, &signature).await;
        let requests = server.await.unwrap();

        assert_eq!(outcome.status, "failed");
        assert_eq!(outcome.attempt_count, MAX_DELIVERY_ATTEMPTS);
        assert_eq!(outcome.http_status, Some(503));
        assert_eq!(outcome.error_code, Some("http_status"));
        assert_eq!(requests.len(), MAX_DELIVERY_ATTEMPTS as usize);
        for (headers, body) in requests {
            assert!(headers
                .to_ascii_lowercase()
                .contains(&format!("x-fusionpanel-signature: {signature}")));
            assert_eq!(body, payload);
            signer.verify(&body, &signature, signed_at).unwrap();
        }
    }

    #[tokio::test]
    async fn live_threshold_crossing_persists_alert_and_delivers_verifiable_event() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let receiver = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let mut reader = BufReader::new(stream);
            let mut headers = String::new();
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).await.unwrap();
                if line == "\r\n" {
                    break;
                }
                headers.push_str(&line);
            }
            let body_length = headers
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().ok())
                        .flatten()
                })
                .unwrap_or_default();
            let mut body = vec![0; body_length];
            reader.read_exact(&mut body).await.unwrap();
            reader
                .get_mut()
                .write_all(
                    b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                )
                .await
                .unwrap();
            (headers, String::from_utf8(body).unwrap())
        });

        let state = test_state();
        let webhook_secret = "test webhook secret for a threshold event";
        let rule = observability::alert_store::NewAlertRule {
            name: "High CPU".to_string(),
            metric: "cpu_usage_percent".to_string(),
            operator: "above".to_string(),
            threshold: 85.0,
            severity: "critical".to_string(),
            webhook_url: format!("http://{address}/alerts"),
            enabled: true,
        };
        let stored_rule = state
            .alerts_store
            .lock()
            .await
            .create_rule(&rule, webhook_secret, &Utc::now().to_rfc3339())
            .unwrap();
        let sample = NodeMetricSample {
            node_id: "node-1".to_string(),
            cpu_usage_percent: 92.0,
            memory_used_kb: 3_000,
            memory_available_kb: 7_000,
            disk_io_ops: 77,
            recorded_at: Utc::now().to_rfc3339(),
        };

        assert!(evaluate_sample(&state, &sample).await.is_ok());
        let (headers, body) = receiver.await.unwrap();
        let signature = headers
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                name.eq_ignore_ascii_case("x-fusionpanel-signature")
                    .then(|| value.trim())
            })
            .unwrap();
        let event: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(event["event"], "triggered");
        assert_eq!(event["rule_id"], stored_rule.id);
        assert_eq!(event["node_id"], sample.node_id);
        assert_eq!(event["value"], 92.0);
        WebhookSigner::new(webhook_secret, 300)
            .verify(&body, signature, Utc::now().timestamp().max(0) as u64)
            .unwrap();

        let store = state.alerts_store.lock().await;
        let active = store.active_states().unwrap();
        assert_eq!(active.len(), 1);
        assert!(active[0].active);
        let deliveries = store.deliveries(10).unwrap();
        assert_eq!(deliveries.len(), 1);
        assert_eq!(deliveries[0].status, "delivered");
        assert_eq!(deliveries[0].attempt_count, 1);
    }
}
