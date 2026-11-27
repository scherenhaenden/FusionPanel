#![forbid(unsafe_code)]

use super::{
    node_jobs,
    websites::{require_website_permission, website_authorization_context},
    AppState, ErrorDetail, ErrorResponse,
};
use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post, put},
    Json, Router,
};
use chrono::Utc;
use fusionpanel_agent_protocol::{
    DnsProbePayload, DnsRecordSpec, DnsRecordType, DnsZoneApplyPayload, DnsZoneDeletePayload,
    Operation, Request,
};
use fusionpanel_control_store::{
    DnsOperation, DnsOperationReservation, DnsRecordEntry, DnsRecordMutation, StoreError,
};
use fusionpanel_jobs::{Job, JobState};
use identity::Permission;
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateZoneRequest {
    domain: String,
    #[serde(default = "default_zone_ttl")]
    default_ttl: u32,
}

fn default_zone_ttl() -> u32 {
    300
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UpsertRecordRequest {
    name: String,
    record_type: DnsRecordType,
    value: String,
    #[serde(default = "default_zone_ttl")]
    ttl: u32,
    #[serde(default)]
    priority: Option<u16>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UpsertDkimRequest {
    selector: String,
    value: String,
    #[serde(default = "default_zone_ttl")]
    ttl: u32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProbeDnsRequest {
    record_name: String,
    record_type: DnsRecordType,
    #[serde(default)]
    expected_substring: Option<String>,
}

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route("/v1/dns/zones", get(list_zones).post(create_zone))
        .route("/api/v1/dns/zones", get(list_zones).post(create_zone))
        .route("/v1/dns/zones/{id}", get(get_zone).delete(delete_zone))
        .route("/api/v1/dns/zones/{id}", get(get_zone).delete(delete_zone))
        .route("/v1/dns/zones/{id}/records", post(create_record))
        .route("/api/v1/dns/zones/{id}/records", post(create_record))
        .route(
            "/v1/dns/zones/{id}/records/{record_id}",
            put(update_record).delete(delete_record),
        )
        .route(
            "/api/v1/dns/zones/{id}/records/{record_id}",
            put(update_record).delete(delete_record),
        )
        .route("/v1/dns/zones/{id}/dkim", post(upsert_dkim))
        .route("/api/v1/dns/zones/{id}/dkim", post(upsert_dkim))
        .route("/v1/dns/zones/{id}/probe", post(probe_zone))
        .route("/api/v1/dns/zones/{id}/probe", post(probe_zone))
}

async fn list_zones(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ErrorResponse> {
    let context = website_authorization_context(&state, &headers).await?;
    require_website_permission(&context, Permission::DnsManage, None)?;
    let zones = state
        .control_store
        .lock()
        .await
        .list_dns_zones(Some(context.tenant_id.to_uuid()))
        .map_err(|_| node_jobs::internal_error())?;
    Ok(Json(json!({ "zones": zones })))
}

async fn get_zone(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ErrorResponse> {
    let context = website_authorization_context(&state, &headers).await?;
    require_website_permission(&context, Permission::DnsManage, None)?;
    let zone_id = parse_uuid(&id)?;
    let zone = state
        .control_store
        .lock()
        .await
        .get_dns_zone(zone_id)
        .map_err(|_| node_jobs::internal_error())?
        .filter(|z| z.zone.tenant_id == context.tenant_id.to_uuid())
        .ok_or_else(not_found)?;
    Ok(Json(json!({ "zone": zone.zone, "records": zone.records })))
}

async fn create_zone(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<CreateZoneRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>), ErrorResponse> {
    let context = website_authorization_context(&state, &headers).await?;
    require_website_permission(&context, Permission::DnsManage, None)?;
    let idempotency_key = require_idempotency_key(&headers)?;
    let domain = body.domain.trim().to_ascii_lowercase();
    let probe_req = Request {
        version: 1,
        request_id: Uuid::new_v4(),
        operation: Operation::DnsZoneApply(DnsZoneApplyPayload {
            zone_id: Uuid::new_v4(),
            domain: domain.clone(),
            serial: 1,
            default_ttl: body.default_ttl,
            records: Vec::new(),
        }),
    };
    probe_req
        .validate()
        .map_err(|_| bad_request("Invalid DNS zone domain or TTL"))?;

    let tenant_id = context.tenant_id.to_uuid();
    let scoped_key = format!("dns-zone-create:{tenant_id}:{idempotency_key}");
    let fingerprint = format!("zone.create:{domain}:{}", body.default_ttl);
    let now = Utc::now().to_rfc3339();

    let reservation = state
        .control_store
        .lock()
        .await
        .reserve_dns_zone_create(
            &scoped_key,
            &fingerprint,
            tenant_id,
            &domain,
            body.default_ttl,
            &now,
        )
        .map_err(map_store_error)?;

    enqueue_zone_apply(&state, &scoped_key, reservation).await
}

async fn create_record(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<UpsertRecordRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>), ErrorResponse> {
    let context = website_authorization_context(&state, &headers).await?;
    require_website_permission(&context, Permission::DnsManage, None)?;
    let idempotency_key = require_idempotency_key(&headers)?;
    let zone_id = parse_uuid(&id)?;
    ensure_zone_ownership(&state, zone_id, context.tenant_id.to_uuid()).await?;

    let spec = DnsRecordSpec {
        name: body.name.trim().to_ascii_lowercase(),
        record_type: body.record_type,
        value: body.value.trim().to_string(),
        ttl: body.ttl,
        priority: body.priority,
    };
    spec.validate()
        .map_err(|_| bad_request("Invalid DNS record specification"))?;

    let scoped_key = format!("dns-record-create:{zone_id}:{idempotency_key}");
    let fingerprint = format!(
        "record.create:{}:{:?}:{}:{}:{:?}",
        spec.name, spec.record_type, spec.value, spec.ttl, spec.priority
    );
    let now = Utc::now().to_rfc3339();
    let rec_type_str = record_type_as_str(spec.record_type);

    let reservation = state
        .control_store
        .lock()
        .await
        .reserve_dns_record_create(
            &scoped_key,
            &fingerprint,
            zone_id,
            DnsRecordMutation {
                name: &spec.name,
                record_type: rec_type_str,
                value: &spec.value,
                ttl: spec.ttl,
                priority: spec.priority,
            },
            &now,
        )
        .map_err(map_store_error)?;

    enqueue_zone_apply(&state, &scoped_key, reservation).await
}

async fn update_record(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((id, record_id_str)): Path<(String, String)>,
    Json(body): Json<UpsertRecordRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>), ErrorResponse> {
    let context = website_authorization_context(&state, &headers).await?;
    require_website_permission(&context, Permission::DnsManage, None)?;
    let idempotency_key = require_idempotency_key(&headers)?;
    let zone_id = parse_uuid(&id)?;
    let record_id = parse_uuid(&record_id_str)?;
    ensure_zone_ownership(&state, zone_id, context.tenant_id.to_uuid()).await?;

    let spec = DnsRecordSpec {
        name: body.name.trim().to_ascii_lowercase(),
        record_type: body.record_type,
        value: body.value.trim().to_string(),
        ttl: body.ttl,
        priority: body.priority,
    };
    spec.validate()
        .map_err(|_| bad_request("Invalid DNS record specification"))?;

    let scoped_key = format!("dns-record-update:{zone_id}:{record_id}:{idempotency_key}");
    let fingerprint = format!(
        "record.update:{record_id}:{}:{:?}:{}:{}:{:?}",
        spec.name, spec.record_type, spec.value, spec.ttl, spec.priority
    );
    let now = Utc::now().to_rfc3339();
    let rec_type_str = record_type_as_str(spec.record_type);

    let reservation = state
        .control_store
        .lock()
        .await
        .reserve_dns_record_update(
            &scoped_key,
            &fingerprint,
            zone_id,
            record_id,
            DnsRecordMutation {
                name: &spec.name,
                record_type: rec_type_str,
                value: &spec.value,
                ttl: spec.ttl,
                priority: spec.priority,
            },
            &now,
        )
        .map_err(map_store_error)?;

    enqueue_zone_apply(&state, &scoped_key, reservation).await
}

async fn delete_record(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((id, record_id_str)): Path<(String, String)>,
) -> Result<(StatusCode, Json<serde_json::Value>), ErrorResponse> {
    let context = website_authorization_context(&state, &headers).await?;
    require_website_permission(&context, Permission::DnsManage, None)?;
    let idempotency_key = require_idempotency_key(&headers)?;
    let zone_id = parse_uuid(&id)?;
    let record_id = parse_uuid(&record_id_str)?;
    ensure_zone_ownership(&state, zone_id, context.tenant_id.to_uuid()).await?;

    let scoped_key = format!("dns-record-delete:{zone_id}:{record_id}:{idempotency_key}");
    let fingerprint = format!("record.delete:{zone_id}:{record_id}");
    let now = Utc::now().to_rfc3339();

    let reservation = state
        .control_store
        .lock()
        .await
        .reserve_dns_record_delete(&scoped_key, &fingerprint, zone_id, record_id, &now)
        .map_err(map_store_error)?;

    enqueue_zone_apply(&state, &scoped_key, reservation).await
}

async fn upsert_dkim(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<UpsertDkimRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>), ErrorResponse> {
    let context = website_authorization_context(&state, &headers).await?;
    require_website_permission(&context, Permission::DnsManage, None)?;
    let idempotency_key = require_idempotency_key(&headers)?;
    let zone_id = parse_uuid(&id)?;
    ensure_zone_ownership(&state, zone_id, context.tenant_id.to_uuid()).await?;

    let selector = body.selector.trim().to_ascii_lowercase();
    if selector.is_empty()
        || selector.len() > 63
        || !selector
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        return Err(bad_request("Invalid DKIM selector"));
    }
    let owner_name = format!("{selector}._domainkey");
    let spec = DnsRecordSpec {
        name: owner_name,
        record_type: DnsRecordType::Txt,
        value: body.value.trim().to_string(),
        ttl: body.ttl,
        priority: None,
    };
    spec.validate()
        .map_err(|_| bad_request("Invalid DKIM TXT value or TTL"))?;

    let scoped_key = format!("dns-dkim-upsert:{zone_id}:{idempotency_key}");
    let fingerprint = format!("dkim.upsert:{selector}:{}:{}", spec.value, spec.ttl);
    let now = Utc::now().to_rfc3339();

    let reservation = state
        .control_store
        .lock()
        .await
        .reserve_dns_dkim_upsert(
            &scoped_key,
            &fingerprint,
            zone_id,
            &selector,
            &spec.value,
            spec.ttl,
            &now,
        )
        .map_err(map_store_error)?;

    enqueue_zone_apply(&state, &scoped_key, reservation).await
}

async fn delete_zone(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<(StatusCode, Json<serde_json::Value>), ErrorResponse> {
    let context = website_authorization_context(&state, &headers).await?;
    require_website_permission(&context, Permission::DnsManage, None)?;
    let idempotency_key = require_idempotency_key(&headers)?;
    let zone_id = parse_uuid(&id)?;
    ensure_zone_ownership(&state, zone_id, context.tenant_id.to_uuid()).await?;

    let scoped_key = format!("dns-zone-delete:{zone_id}:{idempotency_key}");
    let fingerprint = format!("zone.delete:{zone_id}");
    let now = Utc::now().to_rfc3339();

    let reservation = state
        .control_store
        .lock()
        .await
        .reserve_dns_zone_delete(&scoped_key, &fingerprint, zone_id, &now)
        .map_err(map_store_error)?;

    let job = if reservation.idempotent_replay {
        load_existing_job(&state, reservation.job_id).await?
    } else {
        let req = Request {
            version: 1,
            request_id: reservation.request_id,
            operation: Operation::DnsZoneDelete(DnsZoneDeletePayload {
                zone_id: reservation.zone.id,
                domain: reservation.zone.domain.clone(),
            }),
        };
        enqueue_dns_job(&state, &scoped_key, req).await?
    };

    Ok((
        StatusCode::ACCEPTED,
        Json(json!({ "zone": reservation.zone, "job": job })),
    ))
}

async fn probe_zone(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<ProbeDnsRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>), ErrorResponse> {
    let context = website_authorization_context(&state, &headers).await?;
    require_website_permission(&context, Permission::DnsManage, None)?;
    let idempotency_key = require_idempotency_key(&headers)?;
    let zone_id = parse_uuid(&id)?;
    let existing = ensure_zone_ownership(&state, zone_id, context.tenant_id.to_uuid()).await?;

    let record_name = body.record_name.trim().to_ascii_lowercase();
    let expected_substring = body
        .expected_substring
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    let probe_payload = DnsProbePayload {
        zone_id,
        domain: existing.domain.clone(),
        record_name: record_name.clone(),
        record_type: body.record_type,
        expected_substring: expected_substring.clone(),
    };
    let req_check = Request {
        version: 1,
        request_id: Uuid::new_v4(),
        operation: Operation::DnsProbe(probe_payload.clone()),
    };
    req_check
        .validate()
        .map_err(|_| bad_request("Invalid DNS probe parameters"))?;

    let scoped_key = format!("dns-probe:{zone_id}:{idempotency_key}");
    let fingerprint = format!(
        "dns.probe:{}:{:?}:{:?}",
        record_name, body.record_type, expected_substring
    );

    let reservation = state
        .control_store
        .lock()
        .await
        .reserve_dns_probe(&scoped_key, &fingerprint, zone_id)
        .map_err(map_store_error)?;

    let job = if reservation.idempotent_replay {
        load_existing_job(&state, reservation.job_id).await?
    } else {
        let req = Request {
            version: 1,
            request_id: reservation.request_id,
            operation: Operation::DnsProbe(probe_payload),
        };
        enqueue_dns_job(&state, &scoped_key, req).await?
    };

    Ok((
        StatusCode::ACCEPTED,
        Json(json!({ "zone": reservation.zone, "job": job })),
    ))
}

async fn enqueue_zone_apply(
    state: &AppState,
    scoped_key: &str,
    reservation: DnsOperationReservation,
) -> Result<(StatusCode, Json<serde_json::Value>), ErrorResponse> {
    let job = if reservation.idempotent_replay {
        load_existing_job(state, reservation.job_id).await?
    } else {
        let protocol_records = reservation
            .records
            .iter()
            .map(entry_to_protocol_record)
            .collect::<Result<Vec<_>, _>>()?;
        let req = Request {
            version: 1,
            request_id: reservation.request_id,
            operation: Operation::DnsZoneApply(DnsZoneApplyPayload {
                zone_id: reservation.zone.id,
                domain: reservation.zone.domain.clone(),
                serial: reservation.zone.serial,
                default_ttl: reservation.zone.default_ttl,
                records: protocol_records,
            }),
        };
        enqueue_dns_job(state, scoped_key, req).await?
    };

    Ok((
        StatusCode::ACCEPTED,
        Json(json!({
            "zone": reservation.zone,
            "records": reservation.records,
            "job": job
        })),
    ))
}

async fn enqueue_dns_job(
    state: &AppState,
    scoped_key: &str,
    req: Request,
) -> Result<Job, ErrorResponse> {
    req.validate()
        .map_err(|_| bad_request("Invalid DNS operation payload"))?;
    let payload_value = serde_json::to_value(&req).map_err(|_| node_jobs::internal_error())?;
    let job_key = format!("dns-job:{scoped_key}");
    let (job_id, _) = state
        .jobs_store
        .lock()
        .await
        .create_idempotent("agent_operation", &payload_value, &job_key, &payload_value)
        .map_err(node_jobs::map_job_error)?;
    let job_uuid = Uuid::parse_str(&job_id).map_err(|_| node_jobs::internal_error())?;
    state
        .control_store
        .lock()
        .await
        .set_dns_operation_job_id(scoped_key, job_uuid)
        .map_err(|_| node_jobs::internal_error())?;
    state
        .jobs_store
        .lock()
        .await
        .get(&job_id)
        .map_err(node_jobs::map_job_error)
}

async fn load_existing_job(state: &AppState, job_id: Option<Uuid>) -> Result<Job, ErrorResponse> {
    let job_id = job_id.ok_or_else(node_jobs::internal_error)?;
    state
        .jobs_store
        .lock()
        .await
        .get(&job_id.to_string())
        .map_err(node_jobs::map_job_error)
}

async fn ensure_zone_ownership(
    state: &AppState,
    zone_id: Uuid,
    tenant_id: Uuid,
) -> Result<fusionpanel_control_store::DnsZoneRecord, ErrorResponse> {
    let zone = state
        .control_store
        .lock()
        .await
        .get_dns_zone(zone_id)
        .map_err(|_| node_jobs::internal_error())?
        .filter(|z| z.zone.tenant_id == tenant_id)
        .ok_or_else(not_found)?;
    Ok(zone.zone)
}

fn entry_to_protocol_record(entry: &DnsRecordEntry) -> Result<DnsRecordSpec, ErrorResponse> {
    let record_type = match entry.record_type.as_str() {
        "A" => DnsRecordType::A,
        "AAAA" => DnsRecordType::Aaaa,
        "CNAME" => DnsRecordType::Cname,
        "MX" => DnsRecordType::Mx,
        "TXT" => DnsRecordType::Txt,
        _ => return Err(node_jobs::internal_error()),
    };
    Ok(DnsRecordSpec {
        name: entry.name.clone(),
        record_type,
        value: entry.value.clone(),
        ttl: entry.ttl,
        priority: entry.priority,
    })
}

fn record_type_as_str(rt: DnsRecordType) -> &'static str {
    match rt {
        DnsRecordType::A => "A",
        DnsRecordType::Aaaa => "AAAA",
        DnsRecordType::Cname => "CNAME",
        DnsRecordType::Mx => "MX",
        DnsRecordType::Txt => "TXT",
    }
}

pub(super) async fn record_completion(state: &AppState, job: &Job) -> Result<(), ErrorResponse> {
    let Ok(request) = serde_json::from_value::<Request>(job.payload.clone()) else {
        return Ok(());
    };
    let op = match &request.operation {
        Operation::DnsZoneApply(_) => DnsOperation::ZoneApply,
        Operation::DnsZoneDelete(_) => DnsOperation::ZoneDelete,
        Operation::DnsProbe(_) => DnsOperation::Probe,
        _ => return Ok(()),
    };

    let succeeded = job.state == JobState::Succeeded;
    let message = if op == DnsOperation::Probe && succeeded {
        let logs = state
            .jobs_store
            .lock()
            .await
            .list_logs(&job.id)
            .unwrap_or_default();
        logs.last()
            .map(|entry| entry.text.clone())
            .or_else(|| Some("DNS probe succeeded".to_string()))
    } else {
        job.error_text.clone()
    };
    let now = Utc::now().to_rfc3339();
    state
        .control_store
        .lock()
        .await
        .complete_dns_operation(request.request_id, op, succeeded, message.as_deref(), &now)
        .map_err(|_| node_jobs::internal_error())?;
    Ok(())
}

fn require_idempotency_key(headers: &HeaderMap) -> Result<&str, ErrorResponse> {
    headers
        .get("idempotency-key")
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .ok_or_else(|| bad_request("Idempotency-Key header is required"))
}

fn parse_uuid(value: &str) -> Result<Uuid, ErrorResponse> {
    Uuid::parse_str(value).map_err(|_| bad_request("Invalid UUID"))
}

fn bad_request(message: &'static str) -> ErrorResponse {
    ErrorResponse {
        error: ErrorDetail {
            code: "bad_request",
            message,
        },
    }
}

fn not_found() -> ErrorResponse {
    ErrorResponse {
        error: ErrorDetail {
            code: "not_found",
            message: "DNS zone or record not found",
        },
    }
}

fn map_store_error(error: StoreError) -> ErrorResponse {
    match error {
        StoreError::NotFound(_) => not_found(),
        StoreError::Conflict { .. } => ErrorResponse {
            error: ErrorDetail {
                code: "conflict",
                message: "DNS zone already exists or idempotency key mismatch",
            },
        },
        _ => node_jobs::internal_error(),
    }
}

#[cfg(test)]
mod tests {
    use crate::app;
    use crate::AppState;
    use axum::{
        body::Body,
        http::{header, Request as HttpRequest, StatusCode},
    };
    use fusionpanel_control_store::ControlStore;
    use fusionpanel_jobs::JobStore;
    use identity::{Role, RoleBinding, Scope, Session, SqliteIdentityStore, TenantId, User};
    use std::sync::Arc;
    use tokio::sync::Mutex;
    use tower::ServiceExt;
    use uuid::Uuid;

    fn test_state() -> AppState {
        AppState {
            admin_token: Arc::new("test-admin-token".to_string()),
            identity_store: Arc::new(Mutex::new(SqliteIdentityStore::in_memory().unwrap())),
            control_store: Arc::new(Mutex::new(ControlStore::new_in_memory().unwrap())),
            jobs_store: Arc::new(Mutex::new(JobStore::in_memory().unwrap())),
            frontend_dir: None,
            metrics_store: crate::metrics::shared_store(),
            alerts_store: crate::alerts::test_store(),
        }
    }

    async fn create_session(state: &AppState, tenant_id: Uuid, role: Role) -> String {
        let user = User::new(
            format!("dns-test-{}", Uuid::new_v4()),
            format!("{}@example.test", Uuid::new_v4()),
            "synthetic-password-hash".to_string(),
        )
        .unwrap();
        let tid = TenantId::from(tenant_id);
        let session = Session::new(user.id(), tid, 3600, 1800).unwrap();
        let token = session.id().to_string();
        let store = state.identity_store.lock().await;
        store.create_user(&user).unwrap();
        store
            .save_role_binding(&RoleBinding {
                user_id: user.id(),
                role,
                scope: Scope::Tenant { tenant_id: tid },
            })
            .unwrap();
        store.save_session(&session).unwrap();
        token
    }

    #[tokio::test]
    async fn dns_routes_enforce_auth_tenant_isolation_and_serial_increments() {
        let state = test_state();
        let tenant_a = Uuid::new_v4();
        let tenant_b = Uuid::new_v4();
        let sess_a = create_session(&state, tenant_a, Role::tenant_admin()).await;
        let sess_b = create_session(&state, tenant_b, Role::tenant_admin()).await;

        let router = app(state.clone());

        // Create zone as Tenant A
        let resp = router
            .clone()
            .oneshot(
                HttpRequest::builder()
                    .method("POST")
                    .uri("/v1/dns/zones")
                    .header(header::AUTHORIZATION, format!("Bearer {sess_a}"))
                    .header("idempotency-key", "z1")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(r#"{"domain":"example.com","default_ttl":300}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::ACCEPTED);
        let body = axum::body::to_bytes(resp.into_body(), 1024 * 64)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let zone_id = json["zone"]["id"].as_str().unwrap().to_string();
        assert_eq!(json["zone"]["serial"].as_u64(), Some(1));

        // Tenant B cannot read Tenant A's zone
        let resp_b = router
            .clone()
            .oneshot(
                HttpRequest::builder()
                    .method("GET")
                    .uri(format!("/v1/dns/zones/{zone_id}"))
                    .header(header::AUTHORIZATION, format!("Bearer {sess_b}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp_b.status(), StatusCode::NOT_FOUND);

        // Tenant A upserts DKIM TXT -> serial increments to 2
        let resp_dkim = router
            .clone()
            .oneshot(
                HttpRequest::builder()
                    .method("POST")
                    .uri(format!("/v1/dns/zones/{zone_id}/dkim"))
                    .header(header::AUTHORIZATION, format!("Bearer {sess_a}"))
                    .header("idempotency-key", "dkim1")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        r#"{"selector":"default","value":"v=DKIM1; k=rsa; p=MIIBIjAN","ttl":300}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp_dkim.status(), StatusCode::ACCEPTED);
        let dkim_body = axum::body::to_bytes(resp_dkim.into_body(), 1024 * 64)
            .await
            .unwrap();
        let dkim_json: serde_json::Value = serde_json::from_slice(&dkim_body).unwrap();
        assert_eq!(dkim_json["zone"]["serial"].as_u64(), Some(2));
        assert_eq!(
            dkim_json["records"][0]["name"].as_str(),
            Some("default._domainkey")
        );
    }
}
