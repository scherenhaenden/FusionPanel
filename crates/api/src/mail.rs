#![forbid(unsafe_code)]

use super::{
    node_jobs,
    websites::{require_website_permission, website_authorization_context, WebsiteAuthorizationContext},
    AppState, ErrorDetail, ErrorResponse,
};
use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    routing::get,
    Json, Router,
};
use chrono::Utc;
use fusionpanel_agent_protocol::{
    valid_dkim_selector, valid_mail_address, valid_mail_domain, valid_mail_local_part,
    MailAliasSpec, MailDkimApplyPayload, MailDkimSpec, MailDomainApplyPayload,
    MailDomainDeletePayload, MailMailboxSpec, MailOperationResult, Operation, Request,
};
use fusionpanel_control_store::{
    MailOperation, MailOperationReservation, StoreError,
};
use fusionpanel_jobs::{Job, JobState};
use identity::{PasswordManager, Permission};
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use uuid::Uuid;

const DEFAULT_MAILBOX_QUOTA_BYTES: u64 = 1024 * 1024 * 1024; // 1 GiB

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateMailDomainRequest {
    domain: String,
    node_id: Uuid,
    #[serde(default)]
    dns_zone_id: Option<Uuid>,
    #[serde(default = "default_true")]
    active: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UpdateMailDomainRequest {
    #[serde(default)]
    domain: Option<String>,
    #[serde(default)]
    node_id: Option<Uuid>,
    #[serde(default)]
    dns_zone_id: Option<Option<Uuid>>,
    #[serde(default)]
    active: Option<bool>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateMailboxRequest {
    #[serde(default)]
    address: Option<String>,
    #[serde(default)]
    local_part: Option<String>,
    password: String,
    #[serde(default = "default_quota_bytes")]
    quota_bytes: u64,
    #[serde(default = "default_true")]
    active: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UpdateMailboxRequest {
    #[serde(default)]
    address: Option<String>,
    #[serde(default)]
    password: Option<String>,
    #[serde(default)]
    quota_bytes: Option<u64>,
    #[serde(default)]
    active: Option<bool>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateAliasRequest {
    #[serde(default)]
    source: Option<String>,
    #[serde(default)]
    local_part: Option<String>,
    destinations: Vec<String>,
    #[serde(default = "default_true")]
    active: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UpdateAliasRequest {
    #[serde(default)]
    source: Option<String>,
    #[serde(default)]
    destinations: Option<Vec<String>>,
    #[serde(default)]
    active: Option<bool>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UpsertDkimRequest {
    selector: String,
    #[serde(default = "default_true")]
    enabled: bool,
    #[serde(default)]
    dns_zone_id: Option<Uuid>,
    #[serde(default)]
    rotate: bool,
}

fn default_true() -> bool {
    true
}

fn default_quota_bytes() -> u64 {
    DEFAULT_MAILBOX_QUOTA_BYTES
}

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route("/v1/mail/domains", get(list_domains).post(create_domain))
        .route(
            "/api/v1/mail/domains",
            get(list_domains).post(create_domain),
        )
        .route(
            "/v1/mail/domains/{id}",
            get(get_domain).put(update_domain).delete(delete_domain),
        )
        .route(
            "/api/v1/mail/domains/{id}",
            get(get_domain).put(update_domain).delete(delete_domain),
        )
        .route(
            "/v1/mail/domains/{id}/mailboxes",
            get(list_mailboxes).post(create_mailbox),
        )
        .route(
            "/api/v1/mail/domains/{id}/mailboxes",
            get(list_mailboxes).post(create_mailbox),
        )
        .route(
            "/v1/mail/domains/{id}/mailboxes/{mailbox_id}",
            get(get_mailbox)
                .put(update_mailbox)
                .delete(delete_mailbox),
        )
        .route(
            "/api/v1/mail/domains/{id}/mailboxes/{mailbox_id}",
            get(get_mailbox)
                .put(update_mailbox)
                .delete(delete_mailbox),
        )
        .route(
            "/v1/mail/domains/{id}/aliases",
            get(list_aliases).post(create_alias),
        )
        .route(
            "/api/v1/mail/domains/{id}/aliases",
            get(list_aliases).post(create_alias),
        )
        .route(
            "/v1/mail/domains/{id}/aliases/{alias_id}",
            get(get_alias).put(update_alias).delete(delete_alias),
        )
        .route(
            "/api/v1/mail/domains/{id}/aliases/{alias_id}",
            get(get_alias).put(update_alias).delete(delete_alias),
        )
        .route(
            "/v1/mail/domains/{id}/dkim",
            get(get_dkim)
                .post(upsert_dkim)
                .put(upsert_dkim)
                .delete(delete_dkim),
        )
        .route(
            "/api/v1/mail/domains/{id}/dkim",
            get(get_dkim)
                .post(upsert_dkim)
                .put(upsert_dkim)
                .delete(delete_dkim),
        )
}

async fn list_domains(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, ErrorResponse> {
    let context = website_authorization_context(&state, &headers).await?;
    require_website_permission(&context, Permission::MailManage, None)?;
    let domains = state
        .control_store
        .lock()
        .await
        .list_mail_domains(context.tenant_id.to_uuid())
        .map_err(|_| node_jobs::internal_error())?;
    Ok(Json(json!({ "domains": domains })))
}

async fn get_domain(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>, ErrorResponse> {
    let context = website_authorization_context(&state, &headers).await?;
    require_website_permission(&context, Permission::MailManage, None)?;
    let domain_id = parse_uuid(&id)?;
    let details = state
        .control_store
        .lock()
        .await
        .get_mail_domain_details(domain_id, context.tenant_id.to_uuid())
        .map_err(|_| node_jobs::internal_error())?
        .ok_or_else(not_found)?;
    Ok(Json(json!({
        "domain": details.domain,
        "mailboxes": details.mailboxes,
        "aliases": details.aliases,
        "dkim": details.dkim,
    })))
}

async fn create_domain(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<CreateMailDomainRequest>,
) -> Result<(StatusCode, Json<Value>), ErrorResponse> {
    let context = website_authorization_context(&state, &headers).await?;
    require_website_permission(&context, Permission::MailManage, None)?;
    let idempotency_key = require_idempotency_key(&headers)?;
    let tenant_id = context.tenant_id.to_uuid();

    let domain = body.domain.trim().to_ascii_lowercase();
    if !valid_mail_domain(&domain) {
        return Err(bad_request("Invalid canonical mail domain name"));
    }

    ensure_manageable_node(&state, &context, body.node_id).await?;

    let scoped_key = format!("mail-domain-create:{tenant_id}:{idempotency_key}");
    let fingerprint = format!(
        "mail.domain.create:{}:{}:{:?}:{}",
        domain, body.node_id, body.dns_zone_id, body.active
    );
    let now = Utc::now().to_rfc3339();

    let reservation = state
        .control_store
        .lock()
        .await
        .reserve_mail_domain_create(
            &scoped_key,
            &fingerprint,
            tenant_id,
            body.node_id,
            &domain,
            body.dns_zone_id,
            body.active,
            &now,
        )
        .map_err(map_store_error)?;

    let job = enqueue_domain_apply(&state, &scoped_key, &reservation).await?;
    record_mail_audit(
        &state,
        &context,
        "mail.domain.create",
        "mail_domain",
        &reservation.domain.id.to_string(),
        Some(&format!("domain={} rev={}", reservation.domain.domain, reservation.revision)),
    )
    .await;

    let mut domain_out = reservation.domain;
    if domain_out.pending_job_id.is_none() {
        domain_out.pending_job_id = Uuid::parse_str(&job.id).ok();
    }

    Ok((
        StatusCode::ACCEPTED,
        Json(json!({
            "domain": domain_out,
            "job": node_jobs::sanitize_job_for_operator(job),
        })),
    ))
}

async fn update_domain(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<UpdateMailDomainRequest>,
) -> Result<(StatusCode, Json<Value>), ErrorResponse> {
    let context = website_authorization_context(&state, &headers).await?;
    require_website_permission(&context, Permission::MailManage, None)?;
    let idempotency_key = require_idempotency_key(&headers)?;
    let domain_id = parse_uuid(&id)?;
    let tenant_id = context.tenant_id.to_uuid();

    let existing = state
        .control_store
        .lock()
        .await
        .get_mail_domain_details(domain_id, tenant_id)
        .map_err(|_| node_jobs::internal_error())?
        .ok_or_else(not_found)?;

    if let Some(ref requested_domain) = body.domain {
        if requested_domain.trim().to_ascii_lowercase() != existing.domain.domain {
            return Err(conflict("Mail domain name is immutable and cannot be renamed"));
        }
    }

    let target_node = body.node_id.unwrap_or(existing.domain.node_id);
    ensure_manageable_node(&state, &context, target_node).await?;

    let scoped_key = format!("mail-domain-update:{domain_id}:{idempotency_key}");
    let fingerprint = format!(
        "mail.domain.update:{}:{:?}:{:?}:{:?}",
        domain_id, body.node_id, body.dns_zone_id, body.active
    );
    let now = Utc::now().to_rfc3339();

    let reservation = state
        .control_store
        .lock()
        .await
        .reserve_mail_domain_update(
            &scoped_key,
            &fingerprint,
            domain_id,
            tenant_id,
            body.node_id,
            body.dns_zone_id,
            body.active,
            &now,
        )
        .map_err(map_store_error)?
        .ok_or_else(not_found)?;

    let job = enqueue_domain_apply(&state, &scoped_key, &reservation).await?;
    record_mail_audit(
        &state,
        &context,
        "mail.domain.update",
        "mail_domain",
        &domain_id.to_string(),
        Some(&format!("domain={} rev={}", reservation.domain.domain, reservation.revision)),
    )
    .await;

    let mut domain_out = reservation.domain;
    if domain_out.pending_job_id.is_none() {
        domain_out.pending_job_id = Uuid::parse_str(&job.id).ok();
    }

    Ok((
        StatusCode::ACCEPTED,
        Json(json!({
            "domain": domain_out,
            "job": node_jobs::sanitize_job_for_operator(job),
        })),
    ))
}

async fn delete_domain(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<(StatusCode, Json<Value>), ErrorResponse> {
    let context = website_authorization_context(&state, &headers).await?;
    require_website_permission(&context, Permission::MailManage, None)?;
    let idempotency_key = require_idempotency_key(&headers)?;
    let domain_id = parse_uuid(&id)?;
    let tenant_id = context.tenant_id.to_uuid();

    let existing = state
        .control_store
        .lock()
        .await
        .get_mail_domain_details(domain_id, tenant_id)
        .map_err(|_| node_jobs::internal_error())?
        .ok_or_else(not_found)?;
    ensure_manageable_node(&state, &context, existing.domain.node_id).await?;

    let scoped_key = format!("mail-domain-delete:{domain_id}:{idempotency_key}");
    let fingerprint = format!("mail.domain.delete:{domain_id}");
    let now = Utc::now().to_rfc3339();

    let reservation = state
        .control_store
        .lock()
        .await
        .reserve_mail_domain_delete(&scoped_key, &fingerprint, domain_id, tenant_id, &now)
        .map_err(map_store_error)?
        .ok_or_else(not_found)?;

    let job = if reservation.idempotent_replay {
        load_existing_job(&state, reservation.job_id).await?
    } else {
        let req = Request {
            version: 1,
            request_id: reservation.request_id,
            operation: Operation::MailDomainDelete(MailDomainDeletePayload {
                domain_id: reservation.domain.id,
                tenant_id: reservation.domain.tenant_id,
                node_id: reservation.domain.node_id,
                domain: reservation.domain.domain.clone(),
                revision: reservation.revision,
            }),
        };
        enqueue_mail_job(&state, &scoped_key, reservation.domain.id, MailOperation::DomainDelete, req).await?
    };

    record_mail_audit(
        &state,
        &context,
        "mail.domain.delete",
        "mail_domain",
        &domain_id.to_string(),
        Some(&format!("domain={} rev={}", reservation.domain.domain, reservation.revision)),
    )
    .await;

    let mut domain_out = reservation.domain;
    if domain_out.pending_job_id.is_none() {
        domain_out.pending_job_id = Uuid::parse_str(&job.id).ok();
    }

    Ok((
        StatusCode::ACCEPTED,
        Json(json!({
            "domain": domain_out,
            "job": node_jobs::sanitize_job_for_operator(job),
        })),
    ))
}

async fn list_mailboxes(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>, ErrorResponse> {
    let context = website_authorization_context(&state, &headers).await?;
    require_website_permission(&context, Permission::MailManage, None)?;
    let domain_id = parse_uuid(&id)?;
    let mailboxes = state
        .control_store
        .lock()
        .await
        .list_mail_mailboxes(domain_id, context.tenant_id.to_uuid())
        .map_err(|_| node_jobs::internal_error())?
        .ok_or_else(not_found)?;
    Ok(Json(json!({ "mailboxes": mailboxes })))
}

async fn get_mailbox(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((id, mailbox_id)): Path<(String, String)>,
) -> Result<Json<Value>, ErrorResponse> {
    let context = website_authorization_context(&state, &headers).await?;
    require_website_permission(&context, Permission::MailManage, None)?;
    let domain_id = parse_uuid(&id)?;
    let mailbox_id = parse_uuid(&mailbox_id)?;
    let mailbox = state
        .control_store
        .lock()
        .await
        .get_mail_mailbox(domain_id, mailbox_id, context.tenant_id.to_uuid())
        .map_err(|_| node_jobs::internal_error())?
        .ok_or_else(not_found)?;
    Ok(Json(json!({ "mailbox": mailbox })))
}

async fn create_mailbox(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<CreateMailboxRequest>,
) -> Result<(StatusCode, Json<Value>), ErrorResponse> {
    let context = website_authorization_context(&state, &headers).await?;
    require_website_permission(&context, Permission::MailManage, None)?;
    let idempotency_key = require_idempotency_key(&headers)?;
    let domain_id = parse_uuid(&id)?;
    let tenant_id = context.tenant_id.to_uuid();

    let domain_details = state
        .control_store
        .lock()
        .await
        .get_mail_domain_details(domain_id, tenant_id)
        .map_err(|_| node_jobs::internal_error())?
        .ok_or_else(not_found)?;
    ensure_manageable_node(&state, &context, domain_details.domain.node_id).await?;

    let (address, local_part) = resolve_domain_scoped_address(
        body.address.as_deref(),
        body.local_part.as_deref(),
        &domain_details.domain.domain,
    )?;

    if body.password.len() < 8 || body.password.chars().any(|c| c == '\0' || c == '\r' || c == '\n') {
        return Err(bad_request(
            "Mailbox password must be at least 8 characters without control characters",
        ));
    }

    let pw_digest = sha256_hex(body.password.as_bytes());
    let password_hash = PasswordManager::hash_password(&body.password)
        .map_err(|_| bad_request("Invalid mailbox password"))?;

    let scoped_key = format!("mail-mailbox-create:{domain_id}:{idempotency_key}");
    let fingerprint = format!(
        "mail.mailbox.create:{}:{}:{}:{}:{}",
        domain_id, address, body.quota_bytes, body.active, pw_digest
    );
    let now = Utc::now().to_rfc3339();

    let reservation = state
        .control_store
        .lock()
        .await
        .reserve_mail_mailbox_create(
            &scoped_key,
            &fingerprint,
            domain_id,
            tenant_id,
            &address,
            &local_part,
            body.quota_bytes,
            body.active,
            &password_hash,
            &now,
        )
        .map_err(map_store_error)?
        .ok_or_else(not_found)?;

    let job = enqueue_domain_apply(&state, &scoped_key, &reservation).await?;
    let mailbox_out = reservation.target_mailbox.clone();
    if let Some(ref mb) = mailbox_out {
        record_mail_audit(
            &state,
            &context,
            "mail.mailbox.create",
            "mail_mailbox",
            &mb.id.to_string(),
            Some(&format!("address={} rev={}", mb.address, reservation.revision)),
        )
        .await;
    }

    let mut domain_out = reservation.domain;
    if domain_out.pending_job_id.is_none() {
        domain_out.pending_job_id = Uuid::parse_str(&job.id).ok();
    }

    Ok((
        StatusCode::ACCEPTED,
        Json(json!({
            "mailbox": mailbox_out,
            "domain": domain_out,
            "job": node_jobs::sanitize_job_for_operator(job),
        })),
    ))
}

async fn update_mailbox(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((id, mailbox_id)): Path<(String, String)>,
    Json(body): Json<UpdateMailboxRequest>,
) -> Result<(StatusCode, Json<Value>), ErrorResponse> {
    let context = website_authorization_context(&state, &headers).await?;
    require_website_permission(&context, Permission::MailManage, None)?;
    let idempotency_key = require_idempotency_key(&headers)?;
    let domain_id = parse_uuid(&id)?;
    let mailbox_id = parse_uuid(&mailbox_id)?;
    let tenant_id = context.tenant_id.to_uuid();

    let domain_details = state
        .control_store
        .lock()
        .await
        .get_mail_domain_details(domain_id, tenant_id)
        .map_err(|_| node_jobs::internal_error())?
        .ok_or_else(not_found)?;
    ensure_manageable_node(&state, &context, domain_details.domain.node_id).await?;

    let existing_mb = state
        .control_store
        .lock()
        .await
        .get_mail_mailbox(domain_id, mailbox_id, tenant_id)
        .map_err(|_| node_jobs::internal_error())?
        .ok_or_else(not_found)?;

    if let Some(ref addr) = body.address {
        if addr.trim().to_ascii_lowercase() != existing_mb.address {
            return Err(conflict("Mailbox address is immutable and cannot be renamed"));
        }
    }

    let (pw_digest, password_hash) = match body.password.as_deref() {
        Some(pw) => {
            if pw.len() < 8 || pw.chars().any(|c| c == '\0' || c == '\r' || c == '\n') {
                return Err(bad_request(
                    "Mailbox password must be at least 8 characters without control characters",
                ));
            }
            let hash = PasswordManager::hash_password(pw)
                .map_err(|_| bad_request("Invalid mailbox password"))?;
            (Some(sha256_hex(pw.as_bytes())), Some(hash))
        }
        None => (None, None),
    };

    let scoped_key = format!("mail-mailbox-update:{mailbox_id}:{idempotency_key}");
    let fingerprint = format!(
        "mail.mailbox.update:{}:{:?}:{:?}:{:?}",
        mailbox_id, body.quota_bytes, body.active, pw_digest
    );
    let now = Utc::now().to_rfc3339();

    let reservation = state
        .control_store
        .lock()
        .await
        .reserve_mail_mailbox_update(
            &scoped_key,
            &fingerprint,
            domain_id,
            mailbox_id,
            tenant_id,
            body.quota_bytes,
            body.active,
            password_hash.as_deref(),
            &now,
        )
        .map_err(map_store_error)?
        .ok_or_else(not_found)?;

    let job = enqueue_domain_apply(&state, &scoped_key, &reservation).await?;
    record_mail_audit(
        &state,
        &context,
        "mail.mailbox.update",
        "mail_mailbox",
        &mailbox_id.to_string(),
        Some(&format!("address={} rev={}", existing_mb.address, reservation.revision)),
    )
    .await;

    let mut domain_out = reservation.domain;
    if domain_out.pending_job_id.is_none() {
        domain_out.pending_job_id = Uuid::parse_str(&job.id).ok();
    }

    Ok((
        StatusCode::ACCEPTED,
        Json(json!({
            "mailbox": reservation.target_mailbox,
            "domain": domain_out,
            "job": node_jobs::sanitize_job_for_operator(job),
        })),
    ))
}

async fn delete_mailbox(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((id, mailbox_id)): Path<(String, String)>,
) -> Result<(StatusCode, Json<Value>), ErrorResponse> {
    let context = website_authorization_context(&state, &headers).await?;
    require_website_permission(&context, Permission::MailManage, None)?;
    let idempotency_key = require_idempotency_key(&headers)?;
    let domain_id = parse_uuid(&id)?;
    let mailbox_id = parse_uuid(&mailbox_id)?;
    let tenant_id = context.tenant_id.to_uuid();

    let domain_details = state
        .control_store
        .lock()
        .await
        .get_mail_domain_details(domain_id, tenant_id)
        .map_err(|_| node_jobs::internal_error())?
        .ok_or_else(not_found)?;
    ensure_manageable_node(&state, &context, domain_details.domain.node_id).await?;

    let scoped_key = format!("mail-mailbox-delete:{mailbox_id}:{idempotency_key}");
    let fingerprint = format!("mail.mailbox.delete:{mailbox_id}");
    let now = Utc::now().to_rfc3339();

    let reservation = state
        .control_store
        .lock()
        .await
        .reserve_mail_mailbox_delete(&scoped_key, &fingerprint, domain_id, mailbox_id, tenant_id, &now)
        .map_err(map_store_error)?
        .ok_or_else(not_found)?;

    let job = enqueue_domain_apply(&state, &scoped_key, &reservation).await?;
    if let Some(ref mb) = reservation.target_mailbox {
        record_mail_audit(
            &state,
            &context,
            "mail.mailbox.delete",
            "mail_mailbox",
            &mailbox_id.to_string(),
            Some(&format!("address={} rev={}", mb.address, reservation.revision)),
        )
        .await;
    }

    let mut domain_out = reservation.domain;
    if domain_out.pending_job_id.is_none() {
        domain_out.pending_job_id = Uuid::parse_str(&job.id).ok();
    }

    Ok((
        StatusCode::ACCEPTED,
        Json(json!({
            "domain": domain_out,
            "job": node_jobs::sanitize_job_for_operator(job),
        })),
    ))
}

async fn list_aliases(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>, ErrorResponse> {
    let context = website_authorization_context(&state, &headers).await?;
    require_website_permission(&context, Permission::MailManage, None)?;
    let domain_id = parse_uuid(&id)?;
    let aliases = state
        .control_store
        .lock()
        .await
        .list_mail_aliases(domain_id, context.tenant_id.to_uuid())
        .map_err(|_| node_jobs::internal_error())?
        .ok_or_else(not_found)?;
    Ok(Json(json!({ "aliases": aliases })))
}

async fn get_alias(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((id, alias_id)): Path<(String, String)>,
) -> Result<Json<Value>, ErrorResponse> {
    let context = website_authorization_context(&state, &headers).await?;
    require_website_permission(&context, Permission::MailManage, None)?;
    let domain_id = parse_uuid(&id)?;
    let alias_id = parse_uuid(&alias_id)?;
    let alias = state
        .control_store
        .lock()
        .await
        .get_mail_alias(domain_id, alias_id, context.tenant_id.to_uuid())
        .map_err(|_| node_jobs::internal_error())?
        .ok_or_else(not_found)?;
    Ok(Json(json!({ "alias": alias })))
}

async fn create_alias(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<CreateAliasRequest>,
) -> Result<(StatusCode, Json<Value>), ErrorResponse> {
    let context = website_authorization_context(&state, &headers).await?;
    require_website_permission(&context, Permission::MailManage, None)?;
    let idempotency_key = require_idempotency_key(&headers)?;
    let domain_id = parse_uuid(&id)?;
    let tenant_id = context.tenant_id.to_uuid();

    let domain_details = state
        .control_store
        .lock()
        .await
        .get_mail_domain_details(domain_id, tenant_id)
        .map_err(|_| node_jobs::internal_error())?
        .ok_or_else(not_found)?;
    ensure_manageable_node(&state, &context, domain_details.domain.node_id).await?;

    let (source, _) = resolve_domain_scoped_address(
        body.source.as_deref(),
        body.local_part.as_deref(),
        &domain_details.domain.domain,
    )?;
    let destinations = canonicalize_alias_destinations(&source, &body.destinations)?;

    let scoped_key = format!("mail-alias-create:{domain_id}:{idempotency_key}");
    let fingerprint = format!(
        "mail.alias.create:{}:{}:{:?}:{}",
        domain_id, source, destinations, body.active
    );
    let now = Utc::now().to_rfc3339();

    let reservation = state
        .control_store
        .lock()
        .await
        .reserve_mail_alias_create(
            &scoped_key,
            &fingerprint,
            domain_id,
            tenant_id,
            &source,
            &destinations,
            body.active,
            &now,
        )
        .map_err(map_store_error)?
        .ok_or_else(not_found)?;

    let job = enqueue_domain_apply(&state, &scoped_key, &reservation).await?;
    if let Some(ref al) = reservation.target_alias {
        record_mail_audit(
            &state,
            &context,
            "mail.alias.create",
            "mail_alias",
            &al.id.to_string(),
            Some(&format!("source={} rev={}", al.source, reservation.revision)),
        )
        .await;
    }

    let mut domain_out = reservation.domain;
    if domain_out.pending_job_id.is_none() {
        domain_out.pending_job_id = Uuid::parse_str(&job.id).ok();
    }

    Ok((
        StatusCode::ACCEPTED,
        Json(json!({
            "alias": reservation.target_alias,
            "domain": domain_out,
            "job": node_jobs::sanitize_job_for_operator(job),
        })),
    ))
}

async fn update_alias(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((id, alias_id)): Path<(String, String)>,
    Json(body): Json<UpdateAliasRequest>,
) -> Result<(StatusCode, Json<Value>), ErrorResponse> {
    let context = website_authorization_context(&state, &headers).await?;
    require_website_permission(&context, Permission::MailManage, None)?;
    let idempotency_key = require_idempotency_key(&headers)?;
    let domain_id = parse_uuid(&id)?;
    let alias_id = parse_uuid(&alias_id)?;
    let tenant_id = context.tenant_id.to_uuid();

    let domain_details = state
        .control_store
        .lock()
        .await
        .get_mail_domain_details(domain_id, tenant_id)
        .map_err(|_| node_jobs::internal_error())?
        .ok_or_else(not_found)?;
    ensure_manageable_node(&state, &context, domain_details.domain.node_id).await?;

    let existing_alias = state
        .control_store
        .lock()
        .await
        .get_mail_alias(domain_id, alias_id, tenant_id)
        .map_err(|_| node_jobs::internal_error())?
        .ok_or_else(not_found)?;

    if let Some(ref src) = body.source {
        if src.trim().to_ascii_lowercase() != existing_alias.source {
            return Err(conflict("Alias source address is immutable and cannot be renamed"));
        }
    }

    let destinations = match body.destinations.as_ref() {
        Some(dests) => Some(canonicalize_alias_destinations(&existing_alias.source, dests)?),
        None => None,
    };

    let scoped_key = format!("mail-alias-update:{alias_id}:{idempotency_key}");
    let fingerprint = format!(
        "mail.alias.update:{}:{:?}:{:?}",
        alias_id, destinations, body.active
    );
    let now = Utc::now().to_rfc3339();

    let reservation = state
        .control_store
        .lock()
        .await
        .reserve_mail_alias_update(
            &scoped_key,
            &fingerprint,
            domain_id,
            alias_id,
            tenant_id,
            destinations.as_deref(),
            body.active,
            &now,
        )
        .map_err(map_store_error)?
        .ok_or_else(not_found)?;

    let job = enqueue_domain_apply(&state, &scoped_key, &reservation).await?;
    record_mail_audit(
        &state,
        &context,
        "mail.alias.update",
        "mail_alias",
        &alias_id.to_string(),
        Some(&format!("source={} rev={}", existing_alias.source, reservation.revision)),
    )
    .await;

    let mut domain_out = reservation.domain;
    if domain_out.pending_job_id.is_none() {
        domain_out.pending_job_id = Uuid::parse_str(&job.id).ok();
    }

    Ok((
        StatusCode::ACCEPTED,
        Json(json!({
            "alias": reservation.target_alias,
            "domain": domain_out,
            "job": node_jobs::sanitize_job_for_operator(job),
        })),
    ))
}

async fn delete_alias(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((id, alias_id)): Path<(String, String)>,
) -> Result<(StatusCode, Json<Value>), ErrorResponse> {
    let context = website_authorization_context(&state, &headers).await?;
    require_website_permission(&context, Permission::MailManage, None)?;
    let idempotency_key = require_idempotency_key(&headers)?;
    let domain_id = parse_uuid(&id)?;
    let alias_id = parse_uuid(&alias_id)?;
    let tenant_id = context.tenant_id.to_uuid();

    let domain_details = state
        .control_store
        .lock()
        .await
        .get_mail_domain_details(domain_id, tenant_id)
        .map_err(|_| node_jobs::internal_error())?
        .ok_or_else(not_found)?;
    ensure_manageable_node(&state, &context, domain_details.domain.node_id).await?;

    let scoped_key = format!("mail-alias-delete:{alias_id}:{idempotency_key}");
    let fingerprint = format!("mail.alias.delete:{alias_id}");
    let now = Utc::now().to_rfc3339();

    let reservation = state
        .control_store
        .lock()
        .await
        .reserve_mail_alias_delete(&scoped_key, &fingerprint, domain_id, alias_id, tenant_id, &now)
        .map_err(map_store_error)?
        .ok_or_else(not_found)?;

    let job = enqueue_domain_apply(&state, &scoped_key, &reservation).await?;
    if let Some(ref al) = reservation.target_alias {
        record_mail_audit(
            &state,
            &context,
            "mail.alias.delete",
            "mail_alias",
            &alias_id.to_string(),
            Some(&format!("source={} rev={}", al.source, reservation.revision)),
        )
        .await;
    }

    let mut domain_out = reservation.domain;
    if domain_out.pending_job_id.is_none() {
        domain_out.pending_job_id = Uuid::parse_str(&job.id).ok();
    }

    Ok((
        StatusCode::ACCEPTED,
        Json(json!({
            "domain": domain_out,
            "job": node_jobs::sanitize_job_for_operator(job),
        })),
    ))
}

async fn get_dkim(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>, ErrorResponse> {
    let context = website_authorization_context(&state, &headers).await?;
    require_website_permission(&context, Permission::MailManage, None)?;
    let domain_id = parse_uuid(&id)?;
    let tenant_id = context.tenant_id.to_uuid();

    let details = state
        .control_store
        .lock()
        .await
        .get_mail_domain_details(domain_id, tenant_id)
        .map_err(|_| node_jobs::internal_error())?
        .ok_or_else(not_found)?;

    Ok(Json(json!({ "dkim": details.dkim })))
}

async fn upsert_dkim(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<UpsertDkimRequest>,
) -> Result<(StatusCode, Json<Value>), ErrorResponse> {
    let context = website_authorization_context(&state, &headers).await?;
    require_website_permission(&context, Permission::MailManage, None)?;
    let idempotency_key = require_idempotency_key(&headers)?;
    let domain_id = parse_uuid(&id)?;
    let tenant_id = context.tenant_id.to_uuid();

    let selector = body.selector.trim().to_ascii_lowercase();
    if !valid_dkim_selector(&selector) {
        return Err(bad_request("Invalid DKIM selector"));
    }

    let domain_details = state
        .control_store
        .lock()
        .await
        .get_mail_domain_details(domain_id, tenant_id)
        .map_err(|_| node_jobs::internal_error())?
        .ok_or_else(not_found)?;
    ensure_manageable_node(&state, &context, domain_details.domain.node_id).await?;

    let scoped_key = format!("mail-dkim-upsert:{domain_id}:{idempotency_key}");
    let fingerprint = format!(
        "mail.dkim.apply:{}:{}:{}:{:?}:{}",
        domain_id, selector, body.enabled, body.dns_zone_id, body.rotate
    );
    let now = Utc::now().to_rfc3339();

    let reservation = state
        .control_store
        .lock()
        .await
        .reserve_mail_dkim_upsert(
            &scoped_key,
            &fingerprint,
            domain_id,
            tenant_id,
            &selector,
            body.enabled,
            body.dns_zone_id,
            &now,
        )
        .map_err(map_store_error)?
        .ok_or_else(not_found)?;

    let job = if reservation.idempotent_replay {
        load_existing_job(&state, reservation.job_id).await?
    } else {
        let dkim_rec = reservation.dkim.as_ref().ok_or_else(node_jobs::internal_error)?;
        let req = Request {
            version: 1,
            request_id: reservation.request_id,
            operation: Operation::MailDkimApply(MailDkimApplyPayload {
                domain_id: reservation.domain.id,
                tenant_id: reservation.domain.tenant_id,
                node_id: reservation.domain.node_id,
                domain: reservation.domain.domain.clone(),
                revision: reservation.revision,
                selector: dkim_rec.selector.clone(),
                enabled: dkim_rec.enabled,
                dns_zone_id: dkim_rec.dns_zone_id,
                rotate: body.rotate,
            }),
        };
        enqueue_mail_job(&state, &scoped_key, reservation.domain.id, MailOperation::DkimApply, req).await?
    };

    record_mail_audit(
        &state,
        &context,
        "mail.dkim.upsert",
        "mail_dkim",
        &domain_id.to_string(),
        Some(&format!("selector={} rev={}", selector, reservation.revision)),
    )
    .await;

    let mut dkim_out = reservation.dkim;
    if let Some(ref mut d) = dkim_out {
        if d.pending_job_id.is_none() {
            d.pending_job_id = Uuid::parse_str(&job.id).ok();
        }
    }
    let mut domain_out = reservation.domain;
    if domain_out.pending_job_id.is_none() {
        domain_out.pending_job_id = Uuid::parse_str(&job.id).ok();
    }

    Ok((
        StatusCode::ACCEPTED,
        Json(json!({
            "dkim": dkim_out,
            "domain": domain_out,
            "job": node_jobs::sanitize_job_for_operator(job),
        })),
    ))
}

async fn delete_dkim(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<(StatusCode, Json<Value>), ErrorResponse> {
    let context = website_authorization_context(&state, &headers).await?;
    require_website_permission(&context, Permission::MailManage, None)?;
    let idempotency_key = require_idempotency_key(&headers)?;
    let domain_id = parse_uuid(&id)?;
    let tenant_id = context.tenant_id.to_uuid();

    let domain_details = state
        .control_store
        .lock()
        .await
        .get_mail_domain_details(domain_id, tenant_id)
        .map_err(|_| node_jobs::internal_error())?
        .ok_or_else(not_found)?;
    ensure_manageable_node(&state, &context, domain_details.domain.node_id).await?;

    let scoped_key = format!("mail-dkim-delete:{domain_id}:{idempotency_key}");
    let fingerprint = format!("mail.dkim.delete:{domain_id}");
    let now = Utc::now().to_rfc3339();

    let reservation = state
        .control_store
        .lock()
        .await
        .reserve_mail_dkim_delete(&scoped_key, &fingerprint, domain_id, tenant_id, &now)
        .map_err(map_store_error)?
        .ok_or_else(not_found)?;

    let job = if reservation.idempotent_replay {
        load_existing_job(&state, reservation.job_id).await?
    } else {
        let dkim_rec = reservation.dkim.as_ref().ok_or_else(node_jobs::internal_error)?;
        let req = Request {
            version: 1,
            request_id: reservation.request_id,
            operation: Operation::MailDkimApply(MailDkimApplyPayload {
                domain_id: reservation.domain.id,
                tenant_id: reservation.domain.tenant_id,
                node_id: reservation.domain.node_id,
                domain: reservation.domain.domain.clone(),
                revision: reservation.revision,
                selector: dkim_rec.selector.clone(),
                enabled: false,
                dns_zone_id: dkim_rec.dns_zone_id,
                rotate: false,
            }),
        };
        enqueue_mail_job(&state, &scoped_key, reservation.domain.id, MailOperation::DkimApply, req).await?
    };

    record_mail_audit(
        &state,
        &context,
        "mail.dkim.delete",
        "mail_dkim",
        &domain_id.to_string(),
        Some(&format!("rev={}", reservation.revision)),
    )
    .await;

    Ok((
        StatusCode::ACCEPTED,
        Json(json!({
            "dkim": reservation.dkim,
            "domain": reservation.domain,
            "job": node_jobs::sanitize_job_for_operator(job),
        })),
    ))
}

async fn enqueue_domain_apply(
    state: &AppState,
    scoped_key: &str,
    reservation: &MailOperationReservation,
) -> Result<Job, ErrorResponse> {
    if reservation.idempotent_replay {
        return load_existing_job(state, reservation.job_id).await;
    }

    let secret_map: HashMap<Uuid, &str> = reservation
        .mailbox_secrets
        .iter()
        .map(|(id, hash)| (*id, hash.as_str()))
        .collect();

    let mailboxes = reservation
        .mailboxes
        .iter()
        .map(|mb| MailMailboxSpec {
            mailbox_id: mb.id,
            address: mb.address.clone(),
            local_part: mb.local_part.clone(),
            quota_bytes: mb.quota_bytes,
            active: mb.active,
            password_hash: secret_map.get(&mb.id).map(|h| (*h).to_string()),
        })
        .collect();

    let aliases = reservation
        .aliases
        .iter()
        .map(|al| MailAliasSpec {
            alias_id: al.id,
            source: al.source.clone(),
            destinations: al.destinations.clone(),
            active: al.active,
        })
        .collect();

    let dkim = reservation.dkim.as_ref().map(|d| MailDkimSpec {
        selector: d.selector.clone(),
        enabled: d.enabled,
        dns_zone_id: d.dns_zone_id,
        txt_record: d.txt_record.clone(),
    });

    let req = Request {
        version: 1,
        request_id: reservation.request_id,
        operation: Operation::MailDomainApply(MailDomainApplyPayload {
            domain_id: reservation.domain.id,
            tenant_id: reservation.domain.tenant_id,
            node_id: reservation.domain.node_id,
            domain: reservation.domain.domain.clone(),
            revision: reservation.revision,
            active: reservation.domain.active,
            dns_zone_id: reservation.domain.dns_zone_id,
            mailboxes,
            aliases,
            dkim,
        }),
    };

    enqueue_mail_job(
        state,
        scoped_key,
        reservation.domain.id,
        MailOperation::DomainApply,
        req,
    )
    .await
}

async fn enqueue_mail_job(
    state: &AppState,
    scoped_key: &str,
    domain_id: Uuid,
    op: MailOperation,
    req: Request,
) -> Result<Job, ErrorResponse> {
    req.validate()
        .map_err(|_| bad_request("Invalid mail operation payload"))?;
    let payload_value = serde_json::to_value(&req).map_err(|_| node_jobs::internal_error())?;
    let job_key = format!("mail-job:{scoped_key}");
    let fingerprint = json!({
        "operation": op.as_str(),
        "domain_id": domain_id,
        "request_id": req.request_id,
    });
    let job_id = match state.jobs_store.lock().await.create_idempotent(
        "agent_operation",
        &payload_value,
        &job_key,
        &fingerprint,
    ) {
        Ok((job_id, _)) => job_id,
        Err(err) => {
            let _ = state
                .control_store
                .lock()
                .await
                .cancel_mail_operation_reservation(scoped_key);
            return Err(node_jobs::map_job_error(err));
        }
    };
    let job_uuid = Uuid::parse_str(&job_id).map_err(|_| node_jobs::internal_error())?;
    state
        .control_store
        .lock()
        .await
        .set_mail_operation_job_id(scoped_key, domain_id, op, job_uuid)
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

pub(super) async fn record_completion(
    state: &AppState,
    job: &Job,
    result_value: Option<&Value>,
) -> Result<(), ErrorResponse> {
    let Ok(request) = serde_json::from_value::<Request>(job.payload.clone()) else {
        return Ok(());
    };
    let op = match &request.operation {
        Operation::MailDomainApply(_) => MailOperation::DomainApply,
        Operation::MailDomainDelete(_) => MailOperation::DomainDelete,
        Operation::MailDkimApply(_) => MailOperation::DkimApply,
        _ => return Ok(()),
    };

    let parsed_result: Option<MailOperationResult> = result_value
        .map(|val| serde_json::from_value(val.clone()))
        .transpose()
        .map_err(|_| bad_request("Invalid mail operation result payload"))?;

    let succeeded = job.state == JobState::Succeeded;
    let safe_error = if succeeded {
        None
    } else {
        Some(match op {
            MailOperation::DomainApply => "Mail domain configuration could not be applied",
            MailOperation::DomainDelete => "Mail domain deletion could not be completed",
            MailOperation::DkimApply => "DKIM configuration could not be applied",
        })
    };

    let reported_revision = parsed_result.as_ref().map(|r| r.revision);
    let dkim_txt = parsed_result
        .as_ref()
        .and_then(|r| r.dkim.as_ref())
        .map(|d| d.txt_record.as_str());
    let dkim_pub = parsed_result
        .as_ref()
        .and_then(|r| r.dkim.as_ref())
        .and_then(|d| d.public_key_pem.as_deref());

    let now = Utc::now().to_rfc3339();
    state
        .control_store
        .lock()
        .await
        .complete_mail_operation(
            request.request_id,
            op,
            succeeded,
            reported_revision,
            dkim_txt,
            dkim_pub,
            safe_error,
            &now,
        )
        .map_err(|_| node_jobs::internal_error())?;
    Ok(())
}

async fn ensure_manageable_node(
    state: &AppState,
    context: &WebsiteAuthorizationContext,
    node_id: Uuid,
) -> Result<(), ErrorResponse> {
    if let Some(scopes) = &context.token_scopes {
        let has_node_scope = scopes.iter().any(|s| s.starts_with("node:"));
        if has_node_scope {
            let expected = format!("node:{node_id}");
            let allowed = scopes
                .iter()
                .any(|s| s == "*" || s == "node:*" || s == &expected);
            if !allowed {
                return Err(forbidden("API token scope does not allow managing this node"));
            }
        }
    }

    let node_tenant = state
        .control_store
        .lock()
        .await
        .get_node_tenant(node_id)
        .map_err(|_| node_jobs::internal_error())?
        .ok_or(ErrorResponse {
            error: ErrorDetail {
                code: "not_found",
                message: "Node not found",
            },
        })?;

    if let Some(owner_tenant) = node_tenant {
        if owner_tenant != context.tenant_id.to_uuid() {
            return Err(forbidden("Node is not managed by this tenant"));
        }
    }

    Ok(())
}

fn resolve_domain_scoped_address(
    address: Option<&str>,
    local_part: Option<&str>,
    domain: &str,
) -> Result<(String, String), ErrorResponse> {
    match (address, local_part) {
        (Some(addr), lp_opt) => {
            let canonical = addr.trim().to_ascii_lowercase();
            if !valid_mail_address(&canonical) {
                return Err(bad_request("Invalid mail address"));
            }
            let (lp, addr_domain) = canonical
                .split_once('@')
                .ok_or_else(|| bad_request("Invalid mail address"))?;
            if addr_domain != domain {
                return Err(bad_request("Mail address domain does not match hosted domain"));
            }
            if let Some(lp_given) = lp_opt {
                if lp_given.trim().to_ascii_lowercase() != lp {
                    return Err(bad_request("local_part does not match address"));
                }
            }
            let lp = lp.to_string();
            Ok((canonical, lp))
        }
        (None, Some(lp)) => {
            let canonical_lp = lp.trim().to_ascii_lowercase();
            if !valid_mail_local_part(&canonical_lp) {
                return Err(bad_request("Invalid mail local_part"));
            }
            let canonical = format!("{canonical_lp}@{domain}");
            if !valid_mail_address(&canonical) {
                return Err(bad_request("Invalid mail address"));
            }
            Ok((canonical, canonical_lp))
        }
        (None, None) => Err(bad_request("address or local_part is required")),
    }
}

fn canonicalize_alias_destinations(
    source: &str,
    destinations: &[String],
) -> Result<Vec<String>, ErrorResponse> {
    if destinations.is_empty() {
        return Err(bad_request("Alias must specify at least one destination"));
    }
    let mut out = Vec::with_capacity(destinations.len());
    for raw in destinations {
        let canonical = raw.trim().to_ascii_lowercase();
        if !valid_mail_address(&canonical) {
            return Err(bad_request("Invalid alias destination address"));
        }
        if canonical == source {
            return Err(bad_request("Alias cannot forward to itself"));
        }
        if !out.contains(&canonical) {
            out.push(canonical);
        }
    }
    Ok(out)
}

async fn record_mail_audit(
    state: &AppState,
    context: &WebsiteAuthorizationContext,
    action: &str,
    resource_type: &str,
    resource_id: &str,
    detail: Option<&str>,
) {
    let actor = context
        .user_id
        .map(|u| u.to_string())
        .unwrap_or_else(|| "admin-token".to_string());
    let now = Utc::now().to_rfc3339();
    let _ = state.control_store.lock().await.record_audit_log(
        context.tenant_id.to_uuid(),
        &actor,
        action,
        resource_type,
        resource_id,
        "accepted",
        detail,
        &now,
    );
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn require_idempotency_key(headers: &HeaderMap) -> Result<&str, ErrorResponse> {
    headers
        .get("idempotency-key")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty() && value.len() <= 255)
        .ok_or_else(|| bad_request("Missing or invalid Idempotency-Key header"))
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

fn conflict(message: &'static str) -> ErrorResponse {
    ErrorResponse {
        error: ErrorDetail {
            code: "conflict",
            message,
        },
    }
}

fn forbidden(message: &'static str) -> ErrorResponse {
    ErrorResponse {
        error: ErrorDetail {
            code: "forbidden",
            message,
        },
    }
}

fn not_found() -> ErrorResponse {
    ErrorResponse {
        error: ErrorDetail {
            code: "not_found",
            message: "Not found",
        },
    }
}

fn map_store_error(err: StoreError) -> ErrorResponse {
    match err {
        StoreError::NotFound(_) => not_found(),
        StoreError::IdempotencyConflict => conflict("Idempotency key was reused for a different request"),
        StoreError::MailConflict(_) | StoreError::MailOperationInProgress | StoreError::Conflict { .. } => {
            conflict("Mail request conflicts with existing state")
        }
        _ => node_jobs::internal_error(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{header::AUTHORIZATION, Request as HttpRequest},
    };
    use http_body_util::BodyExt;
    use identity::{Role, RoleBinding, Scope, TenantId, User};
    use std::sync::Arc;
    use tempfile::TempDir;
    use tokio::sync::Mutex;
    use tower::ServiceExt;

    const ADMIN_TOKEN: &str = "mail-test-admin-token";

    fn setup_app() -> (Router, AppState, TempDir, Uuid, Uuid) {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("mail.sqlite");
        let identity_store = Arc::new(Mutex::new(
            identity::SqliteIdentityStore::open(&db_path).unwrap(),
        ));
        let tenant_a = Uuid::from_u128(0xA);
        let tenant_b = Uuid::from_u128(0xB);
        let mut control_store = fusionpanel_control_store::ControlStore::new(&db_path).unwrap();
        for (tid, name) in [(tenant_a, "Tenant A"), (tenant_b, "Tenant B")] {
            control_store
                .create_resource(
                    tid,
                    tid,
                    fusionpanel_control_store::OwnershipMode::PlatformManaged,
                    json!({ "name": name }),
                )
                .unwrap();
        }
        let state = AppState {
            admin_token: Arc::new(ADMIN_TOKEN.to_string()),
            identity_store,
            control_store: Arc::new(Mutex::new(control_store)),
            jobs_store: Arc::new(Mutex::new(fusionpanel_jobs::JobStore::new(&db_path).unwrap())),
            frontend_dir: None,
        };
        let router = super::super::app(state.clone());
        (router, state, dir, tenant_a, tenant_b)
    }

    async fn register_node(app: &Router, name: &str) -> (String, String) {
        let resp = app
            .clone()
            .oneshot(
                HttpRequest::builder()
                    .method("POST")
                    .uri("/v1/nodes/register")
                    .header(AUTHORIZATION, format!("Bearer {ADMIN_TOKEN}"))
                    .header("content-type", "application/json")
                    .body(Body::from(json!({ "name": name }).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
        let body: Value =
            serde_json::from_slice(&resp.into_body().collect().await.unwrap().to_bytes()).unwrap();
        (
            body["node_id"].as_str().unwrap().to_string(),
            body["token"].as_str().unwrap().to_string(),
        )
    }

    async fn create_scoped_token(
        state: &AppState,
        tenant_id: Uuid,
        role: Role,
        scopes: Vec<String>,
    ) -> String {
        let password_hash = PasswordManager::hash_password("CorrectHorseBatteryStaple1!").unwrap();
        let user = User::new(
            format!("user-{}", Uuid::new_v4().simple()),
            format!("{}@example.test", Uuid::new_v4().simple()),
            password_hash,
        )
        .unwrap();
        let user_id = user.id();
        {
            let id_store = state.identity_store.lock().await;
            id_store.create_user(&user).unwrap();
            let binding = RoleBinding {
                user_id,
                role,
                scope: Scope::Tenant {
                    tenant_id: TenantId::from(tenant_id),
                },
            };
            id_store.save_role_binding(&binding).unwrap();
        }
        let raw_token = format!("fp_tok_{}", Uuid::new_v4().simple());
        let token_hash = node_jobs::hash_token(&raw_token);
        state
            .control_store
            .lock()
            .await
            .create_api_token(
                Uuid::new_v4(),
                tenant_id,
                user_id.to_uuid(),
                "mail-test-token",
                &token_hash,
                &scopes,
                &Utc::now().to_rfc3339(),
            )
            .unwrap();
        raw_token
    }

    async fn req_json(
        app: &Router,
        method: &str,
        uri: &str,
        bearer: Option<&str>,
        idem: Option<&str>,
        body: Option<Value>,
    ) -> (StatusCode, Value) {
        let mut builder = HttpRequest::builder().method(method).uri(uri);
        if let Some(tok) = bearer {
            builder = builder.header(AUTHORIZATION, format!("Bearer {tok}"));
        }
        if let Some(key) = idem {
            builder = builder.header("idempotency-key", key);
        }
        let req_body = if let Some(json_body) = body {
            builder = builder.header("content-type", "application/json");
            Body::from(json_body.to_string())
        } else {
            Body::empty()
        };
        let resp = app.clone().oneshot(builder.body(req_body).unwrap()).await.unwrap();
        let status = resp.status();
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        let val = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        (status, val)
    }

    #[tokio::test]
    async fn mail_routes_reject_unauthenticated_and_missing_permission() {
        let (app, state, _dir, tenant_a, _) = setup_app();

        // Unauthenticated
        let (status, _) = req_json(&app, "GET", "/v1/mail/domains", None, None, None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        let (status, _) = req_json(&app, "GET", "/api/v1/mail/domains", None, None, None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);

        // SiteOperator role lacks MailManage
        let no_perm_tok = create_scoped_token(
            &state,
            tenant_a,
            Role::site_operator(),
            vec!["*".to_string()],
        )
        .await;
        let (status, _) =
            req_json(&app, "GET", "/v1/mail/domains", Some(&no_perm_tok), None, None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);

        // TenantAdmin role but token scope lacks mail:manage
        let wrong_scope_tok = create_scoped_token(
            &state,
            tenant_a,
            Role::tenant_admin(),
            vec!["sites:read".to_string()],
        )
        .await;
        let (status, _) = req_json(
            &app,
            "GET",
            "/v1/mail/domains",
            Some(&wrong_scope_tok),
            None,
            None,
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn mail_routes_reject_bad_addresses_selectors_and_local_alias_targets() {
        let (app, state, _dir, tenant_a, _) = setup_app();
        let tok_a = create_scoped_token(
            &state,
            tenant_a,
            Role::tenant_admin(),
            vec!["mail:manage".to_string()],
        )
        .await;
        let (node_id, _) = register_node(&app, "mail-node-1").await;

        // Bad domain
        let (status, _) = req_json(
            &app,
            "POST",
            "/v1/mail/domains",
            Some(&tok_a),
            Some("bad-dom-1"),
            Some(json!({ "domain": "not_a_domain", "node_id": node_id })),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);

        // Create valid domain
        let (status, dom_resp) = req_json(
            &app,
            "POST",
            "/v1/mail/domains",
            Some(&tok_a),
            Some("good-dom-1"),
            Some(json!({ "domain": "Example.COM", "node_id": node_id })),
        )
        .await;
        assert_eq!(status, StatusCode::ACCEPTED);
        let domain_id = dom_resp["domain"]["id"].as_str().unwrap();
        assert_eq!(dom_resp["domain"]["domain"], "example.com");

        // Bad mailbox address (traversal / wrong domain)
        let (status, _) = req_json(
            &app,
            "POST",
            &format!("/v1/mail/domains/{domain_id}/mailboxes"),
            Some(&tok_a),
            Some("bad-mb-1"),
            Some(json!({ "address": "../evil@example.com", "password": "Password123!" })),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);

        let (status, _) = req_json(
            &app,
            "POST",
            &format!("/v1/mail/domains/{domain_id}/mailboxes"),
            Some(&tok_a),
            Some("bad-mb-2"),
            Some(json!({ "address": "alice@other.com", "password": "Password123!" })),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);

        // Alias pointing to non-existent local mailbox -> 409 Conflict
        let (status, _) = req_json(
            &app,
            "POST",
            &format!("/v1/mail/domains/{domain_id}/aliases"),
            Some(&tok_a),
            Some("bad-al-1"),
            Some(json!({
                "source": "info@example.com",
                "destinations": ["missing@example.com"]
            })),
        )
        .await;
        assert_eq!(status, StatusCode::CONFLICT);

        // Bad DKIM selector
        let (status, _) = req_json(
            &app,
            "POST",
            &format!("/v1/mail/domains/{domain_id}/dkim"),
            Some(&tok_a),
            Some("bad-dkim-1"),
            Some(json!({ "selector": "bad.selector!" })),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn mail_routes_deny_foreign_tenant_and_foreign_node_access() {
        let (app, state, _dir, tenant_a, tenant_b) = setup_app();
        let tok_a = create_scoped_token(
            &state,
            tenant_a,
            Role::tenant_admin(),
            vec!["mail:manage".to_string()],
        )
        .await;
        let tok_b = create_scoped_token(
            &state,
            tenant_b,
            Role::tenant_admin(),
            vec!["mail:manage".to_string()],
        )
        .await;

        let (node_a, _) = register_node(&app, "node-a").await;
        let (node_b, _) = register_node(&app, "node-b").await;
        {
            let mut store = state.control_store.lock().await;
            store
                .set_node_tenant(Uuid::parse_str(&node_a).unwrap(), Some(tenant_a))
                .unwrap();
            store
                .set_node_tenant(Uuid::parse_str(&node_b).unwrap(), Some(tenant_b))
                .unwrap();
        }

        // Tenant A cannot create domain on Tenant B's node
        let (status, _) = req_json(
            &app,
            "POST",
            "/v1/mail/domains",
            Some(&tok_a),
            Some("foreign-node-1"),
            Some(json!({ "domain": "a.example.com", "node_id": node_b })),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);

        // Tenant A creates domain on Tenant A's node
        let (status, dom_a) = req_json(
            &app,
            "POST",
            "/v1/mail/domains",
            Some(&tok_a),
            Some("own-node-1"),
            Some(json!({ "domain": "a.example.com", "node_id": node_a })),
        )
        .await;
        assert_eq!(status, StatusCode::ACCEPTED);
        let dom_a_id = dom_a["domain"]["id"].as_str().unwrap();

        // Tenant B cannot read or mutate Tenant A's domain
        let (status, _) = req_json(
            &app,
            "GET",
            &format!("/v1/mail/domains/{dom_a_id}"),
            Some(&tok_b),
            None,
            None,
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);

        // Tenant A creates mailbox; Tenant B cannot alias to it
        let (status, _) = req_json(
            &app,
            "POST",
            &format!("/v1/mail/domains/{dom_a_id}/mailboxes"),
            Some(&tok_a),
            Some("mb-a-1"),
            Some(json!({ "local_part": "alice", "password": "Password123!" })),
        )
        .await;
        assert_eq!(status, StatusCode::ACCEPTED);

        let (status, dom_b) = req_json(
            &app,
            "POST",
            "/v1/mail/domains",
            Some(&tok_b),
            Some("dom-b-1"),
            Some(json!({ "domain": "b.example.com", "node_id": node_b })),
        )
        .await;
        assert_eq!(status, StatusCode::ACCEPTED);
        let dom_b_id = dom_b["domain"]["id"].as_str().unwrap();

        let (status, _) = req_json(
            &app,
            "POST",
            &format!("/v1/mail/domains/{dom_b_id}/aliases"),
            Some(&tok_b),
            Some("al-b-foreign"),
            Some(json!({
                "local_part": "sales",
                "destinations": ["alice@a.example.com"]
            })),
        )
        .await;
        assert_eq!(status, StatusCode::CONFLICT);
    }

    #[tokio::test]
    async fn mail_routes_lifecycle_idempotency_conflicts_and_secret_redaction() {
        let (app, state, _dir, tenant_a, _) = setup_app();
        let tok_a = create_scoped_token(
            &state,
            tenant_a,
            Role::tenant_admin(),
            vec!["mail:manage".to_string()],
        )
        .await;
        let (node_id, node_token) = register_node(&app, "mail-node-sec").await;

        // Create domain via /api/v1 mirror
        let (status, dom_resp) = req_json(
            &app,
            "POST",
            "/api/v1/mail/domains",
            Some(&tok_a),
            Some("dom-idem-1"),
            Some(json!({ "domain": "sec.example.com", "node_id": node_id })),
        )
        .await;
        assert_eq!(status, StatusCode::ACCEPTED);
        let domain_id = dom_resp["domain"]["id"].as_str().unwrap().to_string();
        assert_eq!(dom_resp["domain"]["queued_revision"], 1);
        assert!(dom_resp["domain"]["pending_job_id"].is_string());

        // Exact replay returns same domain & job
        let (status, replay_resp) = req_json(
            &app,
            "POST",
            "/api/v1/mail/domains",
            Some(&tok_a),
            Some("dom-idem-1"),
            Some(json!({ "domain": "sec.example.com", "node_id": node_id })),
        )
        .await;
        assert_eq!(status, StatusCode::ACCEPTED);
        assert_eq!(replay_resp["job"]["id"], dom_resp["job"]["id"]);

        // Conflicting replay returns 409 Conflict
        let (status, _) = req_json(
            &app,
            "POST",
            "/api/v1/mail/domains",
            Some(&tok_a),
            Some("dom-idem-1"),
            Some(json!({ "domain": "other.example.com", "node_id": node_id })),
        )
        .await;
        assert_eq!(status, StatusCode::CONFLICT);

        // Create mailbox with secret password
        let (status, mb_resp) = req_json(
            &app,
            "POST",
            &format!("/v1/mail/domains/{domain_id}/mailboxes"),
            Some(&tok_a),
            Some("mb-sec-1"),
            Some(json!({ "local_part": "bob", "password": "SuperSecretMailboxPassword!" })),
        )
        .await;
        assert_eq!(status, StatusCode::ACCEPTED);
        let mb_resp_str = mb_resp.to_string();
        assert!(!mb_resp_str.contains("SuperSecretMailboxPassword!"));
        assert!(!mb_resp_str.contains("$argon2"));
        assert!(mb_resp["mailbox"].get("password_hash").is_none());

        // Claim & append log containing a private key + hash; verify redaction
        let (_claim_status, claimed) = req_json(
            &app,
            "POST",
            "/v1/jobs/claim",
            Some(&node_token),
            None,
            Some(json!({ "node_id": node_id })),
        )
        .await;
        let job1_id = claimed["job"]["id"].as_str().unwrap().to_string();
        let (log_status, _) = req_json(
            &app,
            "POST",
            &format!("/v1/jobs/{job1_id}/logs"),
            Some(&node_token),
            None,
            Some(json!({
                "node_id": node_id,
                "stream": "stderr",
                "text": "leaked -----BEGIN RSA PRIVATE KEY-----ABC123-----END RSA PRIVATE KEY----- and $argon2id$v=19$m=19456$salt$hash"
            })),
        )
        .await;
        assert_eq!(log_status, StatusCode::CREATED);

        let (_logs_status, logs_json) = req_json(
            &app,
            "GET",
            &format!("/v1/jobs/{job1_id}/logs"),
            Some(ADMIN_TOKEN),
            None,
            None,
        )
        .await;
        let logs_str = logs_json.to_string();
        assert!(!logs_str.contains("ABC123"));
        assert!(!logs_str.contains("salt$hash"));

        // Audit logs also must not contain secrets
        let audits = state
            .control_store
            .lock()
            .await
            .list_audit_logs(tenant_a, 50)
            .unwrap();
        let audits_str = json!(audits).to_string();
        assert!(!audits_str.contains("SuperSecretMailboxPassword!"));
        assert!(!audits_str.contains("$argon2"));
    }

    #[tokio::test]
    async fn mail_routes_failed_and_obsolete_completions_and_active_children_conflict() {
        let (app, _state, _dir, tenant_a, _) = setup_app();
        let tok_a = create_scoped_token(
            &_state,
            tenant_a,
            Role::tenant_admin(),
            vec!["mail:manage".to_string()],
        )
        .await;
        let (node_id, node_token) = register_node(&app, "mail-node-comp").await;

        let (_, dom_resp) = req_json(
            &app,
            "POST",
            "/v1/mail/domains",
            Some(&tok_a),
            Some("comp-dom-1"),
            Some(json!({ "domain": "comp.example.com", "node_id": node_id })),
        )
        .await;
        let domain_id = dom_resp["domain"]["id"].as_str().unwrap().to_string();

        // Claim rev 1 job, then queue rev 2 (DKIM) before rev 1 completes
        let (_, claim1) = req_json(
            &app,
            "POST",
            "/v1/jobs/claim",
            Some(&node_token),
            None,
            Some(json!({ "node_id": node_id })),
        )
        .await;
        let job1_id = claim1["job"]["id"].as_str().unwrap().to_string();

        let (dkim_status, _) = req_json(
            &app,
            "POST",
            &format!("/v1/mail/domains/{domain_id}/dkim"),
            Some(&tok_a),
            Some("comp-dkim-1"),
            Some(json!({ "selector": "mail" })),
        )
        .await;
        assert_eq!(dkim_status, StatusCode::ACCEPTED);

        // Obsolete rev 1 completion completes job1, but does NOT overwrite rev 2
        let (c1_status, _) = req_json(
            &app,
            "POST",
            &format!("/v1/jobs/{job1_id}/complete"),
            Some(&node_token),
            None,
            Some(json!({
                "node_id": node_id,
                "state": "succeeded",
                "result": { "domain_id": domain_id, "revision": 1 }
            })),
        )
        .await;
        assert_eq!(c1_status, StatusCode::NO_CONTENT);

        let (_, details1) = req_json(
            &app,
            "GET",
            &format!("/v1/mail/domains/{domain_id}"),
            Some(&tok_a),
            None,
            None,
        )
        .await;
        assert_eq!(details1["domain"]["state"], "provisioning");
        assert_eq!(details1["domain"]["queued_revision"], 2);
        assert!(details1["domain"]["applied_revision"].is_null());

        // Claim rev 2 DKIM job and complete with public TXT material
        let (_, claim2) = req_json(
            &app,
            "POST",
            "/v1/jobs/claim",
            Some(&node_token),
            None,
            Some(json!({ "node_id": node_id })),
        )
        .await;
        let job2_id = claim2["job"]["id"].as_str().unwrap().to_string();
        let (c2_status, _) = req_json(
            &app,
            "POST",
            &format!("/v1/jobs/{job2_id}/complete"),
            Some(&node_token),
            None,
            Some(json!({
                "node_id": node_id,
                "state": "succeeded",
                "result": {
                    "domain_id": domain_id,
                    "revision": 2,
                    "dkim": {
                        "selector": "mail",
                        "txt_record": "v=DKIM1; k=rsa; p=MIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8A"
                    }
                }
            })),
        )
        .await;
        assert_eq!(c2_status, StatusCode::NO_CONTENT);

        let (_, details2) = req_json(
            &app,
            "GET",
            &format!("/v1/mail/domains/{domain_id}"),
            Some(&tok_a),
            None,
            None,
        )
        .await;
        assert_eq!(details2["domain"]["state"], "ready");
        assert_eq!(details2["domain"]["applied_revision"], 2);
        assert!(details2["domain"]["queued_revision"].is_null());
        assert_eq!(
            details2["dkim"]["txt_record"],
            "v=DKIM1; k=rsa; p=MIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8A"
        );

        // Add mailbox and test failed completion + delete conflict
        let (_, _) = req_json(
            &app,
            "POST",
            &format!("/v1/mail/domains/{domain_id}/mailboxes"),
            Some(&tok_a),
            Some("comp-mb-1"),
            Some(json!({ "local_part": "carol", "password": "Password123!" })),
        )
        .await;
        let (_, claim3) = req_json(
            &app,
            "POST",
            "/v1/jobs/claim",
            Some(&node_token),
            None,
            Some(json!({ "node_id": node_id })),
        )
        .await;
        let job3_id = claim3["job"]["id"].as_str().unwrap().to_string();
        let (c3_status, _) = req_json(
            &app,
            "POST",
            &format!("/v1/jobs/{job3_id}/complete"),
            Some(&node_token),
            None,
            Some(json!({
                "node_id": node_id,
                "state": "failed",
                "error_text": "postfix reload failed"
            })),
        )
        .await;
        assert_eq!(c3_status, StatusCode::NO_CONTENT);

        let (_, details3) = req_json(
            &app,
            "GET",
            &format!("/v1/mail/domains/{domain_id}"),
            Some(&tok_a),
            None,
            None,
        )
        .await;
        assert_eq!(details3["domain"]["state"], "failed");
        assert_eq!(details3["domain"]["failed_revision"], 3);
        assert_eq!(details3["domain"]["applied_revision"], 2);

        // Deleting domain with active mailbox returns 409 Conflict
        let (del_status, _) = req_json(
            &app,
            "DELETE",
            &format!("/v1/mail/domains/{domain_id}"),
            Some(&tok_a),
            Some("comp-del-1"),
            None,
        )
        .await;
        assert_eq!(del_status, StatusCode::CONFLICT);
    }
}
