#![forbid(unsafe_code)]

use axum::{
    extract::State,
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use std::sync::Arc;

use crate::{
    audit::{AuditLog, AuditRecord},
    firewall::{Firewall, FirewallRule},
    middleware::{rate_limit_middleware, RateLimiter, SecurityState},
};

pub fn router() -> Router {
    let state = Arc::new(SecurityState {
        rate_limiter: RateLimiter::new(10.0, 1.0), // 10 tokens capacity, 1 token per second
        audit_log: AuditLog::new(),
        firewall: Firewall::new(),
    });

    Router::new()
        .route("/v1/security/audit-log", get(get_audit_log))
        .route("/v1/security/firewall", get(get_firewall_rules))
        .route("/v1/security/firewall/rules", post(add_firewall_rule))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            rate_limit_middleware,
        ))
        .with_state(state)
}

async fn get_audit_log(State(state): State<Arc<SecurityState>>) -> Json<Vec<AuditRecord>> {
    let records = state.audit_log.get_records().await;
    Json(records)
}

async fn get_firewall_rules(State(state): State<Arc<SecurityState>>) -> Json<Vec<FirewallRule>> {
    let rules = state.firewall.get_rules().await;
    Json(rules)
}

async fn add_firewall_rule(
    State(state): State<Arc<SecurityState>>,
    Json(rule): Json<FirewallRule>,
) -> Result<StatusCode, (StatusCode, String)> {
    match state.firewall.add_rule(rule.clone()).await {
        Ok(_) => {
            state
                .audit_log
                .log(AuditRecord {
                    timestamp: chrono::Utc::now(),
                    action: "ADD_FIREWALL_RULE".to_string(),
                    ip_address: None,
                    resource: Some(rule.cidr),
                    success: true,
                })
                .await;
            Ok(StatusCode::CREATED)
        }
        Err(e) => Err((StatusCode::BAD_REQUEST, e)),
    }
}
