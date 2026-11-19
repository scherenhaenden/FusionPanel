#![forbid(unsafe_code)]

use axum::{
    extract::{DefaultBodyLimit, Path, State},
    http::{HeaderMap, StatusCode},
    routing::get,
    Json, Router,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
};
use tokio::sync::Mutex;
use uuid::Uuid;

use super::{node_jobs, AppState, ErrorResponse};

const SAMPLES_PER_NODE: usize = 120;
const MAX_TRACKED_NODES: usize = 256;
const MAX_SAMPLE_MEMORY_KB: u64 = 1 << 40;

pub(super) type SharedMetricsStore = Arc<Mutex<MetricsBuffer>>;

pub(super) fn shared_store() -> SharedMetricsStore {
    Arc::new(Mutex::new(MetricsBuffer::default()))
}

#[derive(Debug, Default)]
pub(super) struct MetricsBuffer {
    nodes: HashMap<Uuid, VecDeque<NodeMetricSample>>,
}

impl MetricsBuffer {
    fn push(&mut self, node_id: Uuid, sample: NodeMetricSample) {
        if !self.nodes.contains_key(&node_id) && self.nodes.len() >= MAX_TRACKED_NODES {
            let oldest_node = self
                .nodes
                .iter()
                .filter_map(|(id, samples)| samples.back().map(|sample| (*id, &sample.recorded_at)))
                .min_by(|left, right| left.1.cmp(right.1))
                .map(|(id, _)| id);
            if let Some(oldest_node) = oldest_node {
                self.nodes.remove(&oldest_node);
            }
        }

        let samples = self.nodes.entry(node_id).or_default();
        if samples.len() == SAMPLES_PER_NODE {
            samples.pop_front();
        }
        samples.push_back(sample);
    }

    fn latest(&self) -> Vec<NodeMetricSample> {
        let mut samples = self
            .nodes
            .values()
            .filter_map(|samples| samples.back().cloned())
            .collect::<Vec<_>>();
        samples.sort_by(|left, right| left.node_id.cmp(&right.node_id));
        samples
    }

    fn recent(&self, node_id: Uuid) -> Vec<NodeMetricSample> {
        self.nodes
            .get(&node_id)
            .map(|samples| samples.iter().cloned().collect())
            .unwrap_or_default()
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct MetricIngestRequest {
    cpu_usage_percent: f64,
    memory_used_kb: u64,
    memory_available_kb: u64,
    disk_io_ops: u64,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct NodeMetricSample {
    pub(super) node_id: String,
    pub(super) cpu_usage_percent: f64,
    pub(super) memory_used_kb: u64,
    pub(super) memory_available_kb: u64,
    pub(super) disk_io_ops: u64,
    pub(super) recorded_at: String,
}

impl MetricIngestRequest {
    fn validate(self, node_id: Uuid) -> Result<NodeMetricSample, ErrorResponse> {
        let memory_total = self
            .memory_used_kb
            .checked_add(self.memory_available_kb)
            .ok_or_else(node_jobs::bad_request)?;
        if !self.cpu_usage_percent.is_finite()
            || !(0.0..=100.0).contains(&self.cpu_usage_percent)
            || memory_total == 0
            || memory_total > MAX_SAMPLE_MEMORY_KB
        {
            return Err(node_jobs::bad_request());
        }

        Ok(NodeMetricSample {
            node_id: node_id.to_string(),
            cpu_usage_percent: self.cpu_usage_percent,
            memory_used_kb: self.memory_used_kb,
            memory_available_kb: self.memory_available_kb,
            disk_io_ops: self.disk_io_ops,
            recorded_at: Utc::now().to_rfc3339(),
        })
    }
}

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route("/v1/metrics", get(latest_metrics))
        .route(
            "/v1/nodes/{id}/metrics",
            get(node_metrics).post(ingest_node_metrics),
        )
        .layer(DefaultBodyLimit::max(1024))
}

async fn ingest_node_metrics(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<MetricIngestRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>), ErrorResponse> {
    let node_id = parse_uuid(&id)?;
    let token = node_jobs::bearer_token(&headers).ok_or_else(node_jobs::unauthorized)?;
    node_jobs::authenticate_node(&state, node_id, token).await?;
    let sample = request.validate(node_id)?;
    state
        .metrics_store
        .lock()
        .await
        .push(node_id, sample.clone());
    super::alerts::evaluate_sample(&state, &sample).await?;
    Ok((StatusCode::CREATED, Json(json!({ "sample": sample }))))
}

async fn latest_metrics(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ErrorResponse> {
    if !node_jobs::authorized_operator(&state, &headers).await {
        return Err(node_jobs::unauthorized());
    }
    let samples = state.metrics_store.lock().await.latest();
    Ok(Json(json!({ "samples": samples })))
}

async fn node_metrics(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ErrorResponse> {
    if !node_jobs::authorized_operator(&state, &headers).await {
        return Err(node_jobs::unauthorized());
    }
    let node_id = parse_uuid(&id)?;
    let samples = state.metrics_store.lock().await.recent(node_id);
    Ok(Json(json!({ "samples": samples })))
}

fn parse_uuid(value: &str) -> Result<Uuid, ErrorResponse> {
    Uuid::parse_str(value).map_err(|_| node_jobs::bad_request())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::Request};
    use http_body_util::BodyExt;
    use std::sync::Arc;
    use tokio::sync::Mutex;
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
            metrics_store: shared_store(),
            alerts_store: crate::alerts::test_store(),
            frontend_dir: None,
        }
    }

    fn sample_body(cpu: f64) -> String {
        format!(
            r#"{{"cpu_usage_percent":{cpu},"memory_used_kb":2048,"memory_available_kb":4096,"disk_io_ops":12}}"#
        )
    }

    async fn register_node(router: &Router, name: &str) -> (String, String) {
        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/nodes/register")
                    .header("authorization", "Bearer test-admin-token")
                    .header("content-type", "application/json")
                    .body(Body::from(format!(r#"{{"name":"{name}"}}"#)))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
        let body: serde_json::Value =
            serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes())
                .unwrap();
        (
            body["node_id"].as_str().unwrap().to_string(),
            body["token"].as_str().unwrap().to_string(),
        )
    }

    #[tokio::test]
    async fn metrics_ingestion_is_node_scoped_and_reads_require_operator_auth() {
        let router = super::super::app(test_state());
        let (node_id, token) = register_node(&router, "metrics-worker").await;

        let ingest = router
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/v1/nodes/{node_id}/metrics"))
                    .header("authorization", format!("Bearer {token}"))
                    .header("content-type", "application/json")
                    .body(Body::from(sample_body(42.5)))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(ingest.status(), StatusCode::CREATED);

        let agent_cannot_read = router
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/v1/metrics")
                    .header("authorization", format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(agent_cannot_read.status(), StatusCode::UNAUTHORIZED);

        let latest = router
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/v1/metrics")
                    .header("authorization", "Bearer test-admin-token")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(latest.status(), StatusCode::OK);
        let body: serde_json::Value =
            serde_json::from_slice(&latest.into_body().collect().await.unwrap().to_bytes())
                .unwrap();
        assert_eq!(body["samples"].as_array().unwrap().len(), 1);
        assert_eq!(body["samples"][0]["node_id"], node_id);
        assert_eq!(body["samples"][0]["cpu_usage_percent"], 42.5);
        assert!(body["samples"][0]["recorded_at"].as_str().is_some());

        let history = router
            .oneshot(
                Request::builder()
                    .uri(format!("/v1/nodes/{node_id}/metrics"))
                    .header("authorization", "Bearer test-admin-token")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(history.status(), StatusCode::OK);
        let history_body: serde_json::Value =
            serde_json::from_slice(&history.into_body().collect().await.unwrap().to_bytes())
                .unwrap();
        assert_eq!(history_body["samples"].as_array().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn metrics_reject_invalid_values_and_cross_node_tokens() {
        let router = super::super::app(test_state());
        let (node_id, token) = register_node(&router, "metrics-worker").await;
        let (other_node_id, _) = register_node(&router, "other-worker").await;

        for invalid_body in [
            sample_body(100.1),
            r#"{"cpu_usage_percent":20.0,"memory_used_kb":18446744073709551615,"memory_available_kb":1,"disk_io_ops":12}"#.to_string(),
            r#"{"cpu_usage_percent":20.0,"memory_used_kb":2048,"memory_available_kb":4096,"disk_io_ops":12,"extra":true}"#.to_string(),
            "{".to_string(),
        ] {
            let invalid = router
                .clone()
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri(format!("/v1/nodes/{node_id}/metrics"))
                        .header("authorization", format!("Bearer {token}"))
                        .header("content-type", "application/json")
                        .body(Body::from(invalid_body))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert!(!invalid.status().is_success());
        }

        let oversized = router
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/v1/nodes/{node_id}/metrics"))
                    .header("authorization", format!("Bearer {token}"))
                    .header("content-type", "application/json")
                    .body(Body::from(" ".repeat(2048)))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(oversized.status(), StatusCode::PAYLOAD_TOO_LARGE);

        let cross_node = router
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/v1/nodes/{other_node_id}/metrics"))
                    .header("authorization", format!("Bearer {token}"))
                    .header("content-type", "application/json")
                    .body(Body::from(sample_body(50.0)))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(cross_node.status(), StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn ring_buffer_keeps_only_the_newest_bounded_window() {
        let node_id = Uuid::new_v4();
        let mut buffer = MetricsBuffer::default();
        for index in 0..=SAMPLES_PER_NODE {
            buffer.push(
                node_id,
                NodeMetricSample {
                    node_id: node_id.to_string(),
                    cpu_usage_percent: 10.0,
                    memory_used_kb: 1,
                    memory_available_kb: 1,
                    disk_io_ops: index as u64,
                    recorded_at: format!("2026-01-01T00:00:{index:03}Z"),
                },
            );
        }
        let recent = buffer.recent(node_id);
        assert_eq!(recent.len(), SAMPLES_PER_NODE);
        assert_eq!(recent.first().unwrap().disk_io_ops, 1);
        assert_eq!(recent.last().unwrap().disk_io_ops, SAMPLES_PER_NODE as u64);
    }

    #[test]
    fn tracked_node_index_stays_bounded_and_evicts_the_oldest_node() {
        let mut buffer = MetricsBuffer::default();
        for index in 0..=MAX_TRACKED_NODES {
            let node_id = Uuid::from_u128(index as u128 + 1);
            buffer.push(
                node_id,
                NodeMetricSample {
                    node_id: node_id.to_string(),
                    cpu_usage_percent: 10.0,
                    memory_used_kb: 1,
                    memory_available_kb: 1,
                    disk_io_ops: index as u64,
                    recorded_at: format!("{index:03}"),
                },
            );
        }

        assert_eq!(buffer.nodes.len(), MAX_TRACKED_NODES);
        assert!(!buffer.nodes.contains_key(&Uuid::from_u128(1)));
        assert!(buffer
            .nodes
            .contains_key(&Uuid::from_u128(MAX_TRACKED_NODES as u128 + 1)));
    }
}
