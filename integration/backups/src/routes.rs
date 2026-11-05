#![forbid(unsafe_code)]

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use chrono::Utc;
use serde_json::json;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

use crate::models::{BackupArtifact, BackupConfig, BackupOutcome, BackupPlan, BackupStatus};

#[derive(Clone)]
pub struct AppState {
    pub configs: Arc<Mutex<HashMap<String, BackupConfig>>>,
    pub outcomes: Arc<Mutex<HashMap<String, BackupOutcome>>>,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/backups/configs", post(create_config).get(list_configs))
        .route("/backups/configs/:id", get(get_config))
        .route("/backups/configs/:id/run", post(trigger_backup))
        .route("/backups/runs/:id", get(get_run_outcome))
        .with_state(state)
}

async fn create_config(
    State(state): State<AppState>,
    Json(mut config): Json<BackupConfig>,
) -> impl IntoResponse {
    if config.id.is_empty() {
        config.id = Uuid::new_v4().to_string();
    }

    let mut configs = state.configs.lock().unwrap();
    configs.insert(config.id.clone(), config.clone());

    (StatusCode::CREATED, Json(config))
}

async fn list_configs(State(state): State<AppState>) -> impl IntoResponse {
    let configs = state.configs.lock().unwrap();
    let list: Vec<BackupConfig> = configs.values().cloned().collect();
    (StatusCode::OK, Json(list))
}

async fn get_config(State(state): State<AppState>, Path(id): Path<String>) -> impl IntoResponse {
    let configs = state.configs.lock().unwrap();
    if let Some(config) = configs.get(&id) {
        (StatusCode::OK, Json(config.clone())).into_response()
    } else {
        (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "Config not found"})),
        )
            .into_response()
    }
}

async fn trigger_backup(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    // 1. Get config
    let config = {
        let configs = state.configs.lock().unwrap();
        configs.get(&id).cloned()
    };

    let config = match config {
        Some(c) => c,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "Config not found"})),
            )
                .into_response()
        }
    };

    let run_id = Uuid::new_v4().to_string();
    let plan = BackupPlan {
        config_id: id.clone(),
        run_id: run_id.clone(),
        planned_at: Utc::now(),
    };

    {
        let mut outcomes = state.outcomes.lock().unwrap();
        outcomes.insert(
            run_id.clone(),
            BackupOutcome {
                run_id: run_id.clone(),
                status: BackupStatus::Pending,
                completed_at: Utc::now(),
                artifacts: vec![],
                errors: vec![],
            },
        );
    }

    // In a real application, this would dispatch a durable job (via JobStore)
    // to a background worker to execute the dump via `DumpScheduler`.
    // For the sake of exposing the router logic synchronously here we simulate job state update.

    // Simulate transitioning to Running
    {
        let mut outcomes = state.outcomes.lock().unwrap();
        if let Some(outcome) = outcomes.get_mut(&run_id) {
            outcome.status = BackupStatus::Running;
        }
    }

    // Simulate actual job completion with an artifact receipt
    {
        let mut outcomes = state.outcomes.lock().unwrap();
        if let Some(outcome) = outcomes.get_mut(&run_id) {
            outcome.status = BackupStatus::Succeeded;
            outcome.completed_at = Utc::now();
            outcome.artifacts.push(BackupArtifact {
                name: format!("{}-dump", id),
                destination_type: "s3".to_string(),
                path_or_key: format!("s3://bucket/{}/dump.sql.zst", id),
                hash_sha256: "simulated-hash".to_string(),
                size_bytes: 1024,
            });
        }
    }

    (StatusCode::ACCEPTED, Json(plan)).into_response()
}

async fn get_run_outcome(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let outcomes = state.outcomes.lock().unwrap();
    if let Some(outcome) = outcomes.get(&id) {
        (StatusCode::OK, Json(outcome.clone())).into_response()
    } else {
        (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "Run not found"})),
        )
            .into_response()
    }
}
