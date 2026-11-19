#![forbid(unsafe_code)]

use axum::{
    extract::{Path, State},
    http::{header::AUTHORIZATION, HeaderMap, StatusCode},
    routing::get,
    Json, Router,
};
use chrono::Utc;
use fusionpanel_control_store::{
    WebsiteOperation, WebsiteOperationReservation, WebsiteSpec, WebsiteTlsMode,
    WebsiteTlsOperation, WebsiteTlsOperationReservation,
};
use fusionpanel_domain::DomainName;
use identity::{AccessControl, Permission, Scope, SessionId};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{
    fs,
    path::{Component, Path as FsPath, PathBuf},
};
use uuid::Uuid;

use super::{node_jobs, AppState, ErrorDetail, ErrorResponse};

const MAX_UPSTREAM_URL_BYTES: usize = 2048;
const TENANT_CONTEXT_HEADER: &str = "X-FusionPanel-Tenant-ID";

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum CreateWebsiteRequest {
    Static {
        domain: String,
        document_root: String,
    },
    ReverseProxy {
        domain: String,
        upstream_url: String,
    },
}

#[derive(Debug, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case", deny_unknown_fields)]
enum ApplyWebsiteTlsRequest {
    CustomPem {
        certificate_pem: String,
        private_key_pem: String,
    },
    AcmeHttp01 {
        acme_directory_url: String,
        #[serde(default)]
        contact_email: Option<String>,
    },
}

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route("/v1/websites", get(list_websites).post(create_website))
        .route("/v1/websites/{id}", get(get_website).delete(delete_website))
        .route(
            "/v1/websites/{id}/tls",
            get(get_website_tls)
                .post(apply_website_tls)
                .delete(delete_website_tls),
        )
}

async fn list_websites(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, ErrorResponse> {
    let context = website_authorization_context(&state, &headers).await?;
    require_website_permission(&context, Permission::SiteRead, None)?;
    let websites = state
        .control_store
        .lock()
        .await
        .list_websites_for_tenant(context.tenant_id.to_uuid())
        .map_err(map_store_error)?;
    Ok(Json(json!({ "websites": websites })))
}

async fn get_website(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<Value>, ErrorResponse> {
    let context = website_authorization_context(&state, &headers).await?;
    let site_id = Uuid::parse_str(&id).map_err(|_| node_jobs::bad_request())?;
    let site = state
        .control_store
        .lock()
        .await
        .get_website_for_tenant(site_id, context.tenant_id.to_uuid())
        .map_err(map_store_error)?;
    require_website_permission(&context, Permission::SiteRead, Some(site_id))?;
    Ok(Json(json!({ "website": site })))
}

async fn create_website(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<CreateWebsiteRequest>,
) -> Result<(StatusCode, Json<Value>), ErrorResponse> {
    let context = website_authorization_context(&state, &headers).await?;
    require_website_permission(&context, Permission::SiteCreate, None)?;
    let idempotency_key = idempotency_key(&headers)?;

    let (domain, spec) = match request {
        CreateWebsiteRequest::Static {
            domain,
            document_root,
        } => {
            let domain = validate_domain(&domain)?;
            let document_root = validate_document_root(&document_root, &domain)?;
            (domain, WebsiteSpec::Static { document_root })
        }
        CreateWebsiteRequest::ReverseProxy {
            domain,
            upstream_url,
        } => {
            let domain = validate_domain(&domain)?;
            let upstream_url = validate_upstream_url(&upstream_url)?;
            (domain, WebsiteSpec::ReverseProxy { upstream_url })
        }
    };

    let fingerprint = json!({
        "operation": "site.apply",
        "tenant_id": context.tenant_id,
        "payload": { "domain": domain, "spec": spec },
    });
    let reservation = state
        .control_store
        .lock()
        .await
        .reserve_website_create(
            &idempotency_key,
            &fingerprint,
            context.tenant_id.to_uuid(),
            Uuid::new_v4(),
            Uuid::new_v4(),
            &domain,
            spec,
            &Utc::now().to_rfc3339(),
        )
        .map_err(map_store_error)?;

    enqueue_and_respond(&state, &idempotency_key, reservation).await
}

async fn delete_website(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<(StatusCode, Json<Value>), ErrorResponse> {
    let context = website_authorization_context(&state, &headers).await?;
    let idempotency_key = idempotency_key(&headers)?;
    let site_id = Uuid::parse_str(&id).map_err(|_| node_jobs::bad_request())?;
    require_website_permission(&context, Permission::SiteDelete, Some(site_id))?;
    let fingerprint = json!({
        "operation": "site.delete",
        "tenant_id": context.tenant_id,
        "payload": { "site_id": site_id },
    });
    let reservation = state
        .control_store
        .lock()
        .await
        .reserve_website_delete(
            &idempotency_key,
            &fingerprint,
            context.tenant_id.to_uuid(),
            site_id,
            Uuid::new_v4(),
            &Utc::now().to_rfc3339(),
        )
        .map_err(map_store_error)?;

    enqueue_and_respond(&state, &idempotency_key, reservation).await
}

async fn enqueue_and_respond(
    state: &AppState,
    idempotency_key: &str,
    reservation: WebsiteOperationReservation,
) -> Result<(StatusCode, Json<Value>), ErrorResponse> {
    let payload = match (&reservation.operation, &reservation.site.spec) {
        (WebsiteOperation::Apply, WebsiteSpec::Static { document_root }) => json!({
            "version": 1,
            "request_id": reservation.request_id,
            "operation": "site.apply",
            "payload": {
                "site_id": reservation.site.id,
                "domain": reservation.site.domain,
                "spec": { "kind": "static", "document_root": document_root },
            },
        }),
        (WebsiteOperation::Apply, WebsiteSpec::ReverseProxy { upstream_url }) => json!({
            "version": 1,
            "request_id": reservation.request_id,
            "operation": "site.apply",
            "payload": {
                "site_id": reservation.site.id,
                "domain": reservation.site.domain,
                "spec": { "kind": "reverse_proxy", "upstream_url": upstream_url },
            },
        }),
        (WebsiteOperation::Delete, _) => json!({
            "version": 1,
            "request_id": reservation.request_id,
            "operation": "site.delete",
            "payload": {
                "site_id": reservation.site.id,
                "domain": reservation.site.domain,
            },
        }),
    };
    let fingerprint = match reservation.operation {
        WebsiteOperation::Apply => json!({
            "operation": "site.apply",
            "payload": {
                "domain": reservation.site.domain,
                "spec": reservation.site.spec,
            },
        }),
        WebsiteOperation::Delete => json!({
            "operation": "site.delete",
            "payload": { "site_id": reservation.site.id },
        }),
    };

    let job_result = state.jobs_store.lock().await.create_idempotent(
        "agent_operation",
        &payload,
        idempotency_key,
        &fingerprint,
    );
    let (job_id, _) = match job_result {
        Ok(result) => result,
        Err(error) => {
            if reservation.created {
                state
                    .control_store
                    .lock()
                    .await
                    .rollback_website_operation(idempotency_key)
                    .map_err(|_| node_jobs::internal_error())?;
            }
            return Err(node_jobs::map_job_error(error));
        }
    };

    state
        .control_store
        .lock()
        .await
        .link_website_operation_job(idempotency_key, &job_id)
        .map_err(|_| node_jobs::internal_error())?;
    let job = state
        .jobs_store
        .lock()
        .await
        .get(&job_id)
        .map_err(node_jobs::map_job_error)?;
    Ok((
        StatusCode::ACCEPTED,
        Json(json!({ "site": reservation.site, "job": job })),
    ))
}

async fn get_website_tls(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<Value>, ErrorResponse> {
    let context = website_authorization_context(&state, &headers).await?;
    let site_id = Uuid::parse_str(&id).map_err(|_| node_jobs::bad_request())?;
    require_website_permission(&context, Permission::SiteRead, Some(site_id))?;
    let tls = state
        .control_store
        .lock()
        .await
        .get_website_tls_for_tenant(site_id, context.tenant_id.to_uuid())
        .map_err(map_store_error)?;
    Ok(Json(json!({ "tls": tls })))
}

async fn apply_website_tls(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<ApplyWebsiteTlsRequest>,
) -> Result<(StatusCode, Json<Value>), ErrorResponse> {
    let context = website_authorization_context(&state, &headers).await?;
    let site_id = Uuid::parse_str(&id).map_err(|_| node_jobs::bad_request())?;
    require_website_permission(&context, Permission::SiteUpdate, Some(site_id))?;
    let idempotency_key = idempotency_key(&headers)?;

    let site = state
        .control_store
        .lock()
        .await
        .get_website_for_tenant(site_id, context.tenant_id.to_uuid())
        .map_err(map_store_error)?;

    let (mode, acme_url, contact_email, source, fingerprint) = match request {
        ApplyWebsiteTlsRequest::CustomPem {
            certificate_pem,
            private_key_pem,
        } => {
            let source = fusionpanel_agent_protocol::TlsCertificateSource::CustomPem {
                certificate_pem,
                private_key_pem,
            };
            let fp = json!({
                "operation": "tls.apply",
                "tenant_id": context.tenant_id,
                "site_id": site_id,
                "mode": "custom_pem",
            });
            (WebsiteTlsMode::CustomPem, None, None, source, fp)
        }
        ApplyWebsiteTlsRequest::AcmeHttp01 {
            acme_directory_url,
            contact_email,
        } => {
            let acme_directory_url = validate_upstream_url(&acme_directory_url)?;
            let source = fusionpanel_agent_protocol::TlsCertificateSource::AcmeHttp01 {
                acme_directory_url: acme_directory_url.clone(),
                contact_email: contact_email.clone(),
            };
            let fp = json!({
                "operation": "tls.apply",
                "tenant_id": context.tenant_id,
                "site_id": site_id,
                "mode": "acme_http01",
                "acme_directory_url": acme_directory_url,
                "contact_email": contact_email,
            });
            (
                WebsiteTlsMode::AcmeHttp01,
                Some(acme_directory_url),
                contact_email,
                source,
                fp,
            )
        }
    };

    let spec = match site.spec {
        WebsiteSpec::Static { document_root } => {
            fusionpanel_agent_protocol::SiteSpec::Static { document_root }
        }
        WebsiteSpec::ReverseProxy { upstream_url } => {
            fusionpanel_agent_protocol::SiteSpec::ReverseProxy { upstream_url }
        }
    };

    let protocol_req = fusionpanel_agent_protocol::Request {
        version: 1,
        request_id: Uuid::new_v4(),
        operation: fusionpanel_agent_protocol::Operation::TlsApply(
            fusionpanel_agent_protocol::TlsApplyPayload {
                site_id,
                domain: site.domain.clone(),
                spec,
                source,
            },
        ),
    };
    protocol_req
        .validate()
        .map_err(|_| node_jobs::bad_request())?;

    let reservation = state
        .control_store
        .lock()
        .await
        .reserve_website_tls_apply(
            &idempotency_key,
            &fingerprint,
            context.tenant_id.to_uuid(),
            site_id,
            protocol_req.request_id,
            mode,
            acme_url.as_deref(),
            contact_email.as_deref(),
            &Utc::now().to_rfc3339(),
        )
        .map_err(map_store_error)?;

    enqueue_tls_and_respond(&state, &idempotency_key, reservation, Some(protocol_req)).await
}

async fn delete_website_tls(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<(StatusCode, Json<Value>), ErrorResponse> {
    let context = website_authorization_context(&state, &headers).await?;
    let site_id = Uuid::parse_str(&id).map_err(|_| node_jobs::bad_request())?;
    require_website_permission(&context, Permission::SiteUpdate, Some(site_id))?;
    let idempotency_key = idempotency_key(&headers)?;

    let site = state
        .control_store
        .lock()
        .await
        .get_website_for_tenant(site_id, context.tenant_id.to_uuid())
        .map_err(map_store_error)?;

    let spec = match site.spec {
        WebsiteSpec::Static { document_root } => {
            fusionpanel_agent_protocol::SiteSpec::Static { document_root }
        }
        WebsiteSpec::ReverseProxy { upstream_url } => {
            fusionpanel_agent_protocol::SiteSpec::ReverseProxy { upstream_url }
        }
    };

    let fingerprint = json!({
        "operation": "tls.delete",
        "tenant_id": context.tenant_id,
        "site_id": site_id,
    });
    let protocol_req = fusionpanel_agent_protocol::Request {
        version: 1,
        request_id: Uuid::new_v4(),
        operation: fusionpanel_agent_protocol::Operation::TlsDelete(
            fusionpanel_agent_protocol::TlsDeletePayload {
                site_id,
                domain: site.domain.clone(),
                spec,
            },
        ),
    };
    protocol_req
        .validate()
        .map_err(|_| node_jobs::bad_request())?;

    let reservation = state
        .control_store
        .lock()
        .await
        .reserve_website_tls_delete(
            &idempotency_key,
            &fingerprint,
            context.tenant_id.to_uuid(),
            site_id,
            protocol_req.request_id,
            &Utc::now().to_rfc3339(),
        )
        .map_err(map_store_error)?;

    enqueue_tls_and_respond(&state, &idempotency_key, reservation, Some(protocol_req)).await
}

async fn enqueue_tls_and_respond(
    state: &AppState,
    idempotency_key: &str,
    reservation: WebsiteTlsOperationReservation,
    protocol_req: Option<fusionpanel_agent_protocol::Request>,
) -> Result<(StatusCode, Json<Value>), ErrorResponse> {
    let job_id = if let Some(job_id) = reservation.job_id {
        job_id
    } else {
        let req = protocol_req.ok_or_else(node_jobs::internal_error)?;
        let payload = serde_json::to_value(&req).map_err(|_| node_jobs::internal_error())?;
        let fingerprint = json!({
            "operation": reservation.operation.as_str(),
            "site_id": reservation.tls.site_id,
            "request_id": reservation.request_id,
        });
        match state.jobs_store.lock().await.create_idempotent(
            "agent_operation",
            &payload,
            idempotency_key,
            &fingerprint,
        ) {
            Ok((job_id, _)) => job_id,
            Err(error) => {
                if reservation.created {
                    let _ = state
                        .control_store
                        .lock()
                        .await
                        .cancel_website_tls_operation_reservation(
                            idempotency_key,
                            reservation.tls.site_id,
                            reservation.request_id,
                        );
                }
                return Err(node_jobs::map_job_error(error));
            }
        }
    };

    state
        .control_store
        .lock()
        .await
        .attach_website_tls_job_id(idempotency_key, &job_id)
        .map_err(|_| node_jobs::internal_error())?;

    let job = state
        .jobs_store
        .lock()
        .await
        .get(&job_id)
        .map_err(node_jobs::map_job_error)?;

    Ok((
        StatusCode::ACCEPTED,
        Json(json!({
            "tls": reservation.tls,
            "job": node_jobs::sanitize_job_for_operator(job),
        })),
    ))
}

/// Updates the persisted website only after its agent job reaches a terminal state.
pub(super) async fn record_completion(
    state: &AppState,
    job: &fusionpanel_jobs::Job,
) -> Result<(), ErrorResponse> {
    if job.kind != "agent_operation" {
        return Ok(());
    }
    let Some(operation) = job.payload.get("operation").and_then(Value::as_str) else {
        return Ok(());
    };
    if matches!(operation, "tls.apply" | "tls.delete") {
        let tls_operation = if operation == "tls.apply" {
            WebsiteTlsOperation::Apply
        } else {
            WebsiteTlsOperation::Delete
        };
        let request_id = job
            .payload
            .get("request_id")
            .and_then(Value::as_str)
            .and_then(|value| Uuid::parse_str(value).ok())
            .ok_or_else(node_jobs::internal_error)?;
        let site_id = job
            .payload
            .get("payload")
            .and_then(|payload| payload.get("site_id"))
            .and_then(Value::as_str)
            .and_then(|value| Uuid::parse_str(value).ok())
            .ok_or_else(node_jobs::internal_error)?;
        let succeeded = job.state == fusionpanel_jobs::JobState::Succeeded;
        let safe_error = if succeeded {
            None
        } else {
            Some(match tls_operation {
                WebsiteTlsOperation::Apply => "TLS certificate could not be applied",
                WebsiteTlsOperation::Delete => "TLS certificate could not be removed",
            })
        };
        state
            .control_store
            .lock()
            .await
            .complete_website_tls_operation(
                site_id,
                request_id,
                tls_operation,
                succeeded,
                safe_error,
                &Utc::now().to_rfc3339(),
            )
            .map_err(map_store_error)?;
        return Ok(());
    }
    let website_operation = match operation {
        "site.apply" => WebsiteOperation::Apply,
        "site.delete" => WebsiteOperation::Delete,
        _ => return Ok(()),
    };
    let request_id = job
        .payload
        .get("request_id")
        .and_then(Value::as_str)
        .and_then(|value| Uuid::parse_str(value).ok())
        .ok_or_else(node_jobs::internal_error)?;
    let site_id = job
        .payload
        .get("payload")
        .and_then(|payload| payload.get("site_id"))
        .and_then(Value::as_str)
        .and_then(|value| Uuid::parse_str(value).ok())
        .ok_or_else(node_jobs::internal_error)?;
    let succeeded = job.state == fusionpanel_jobs::JobState::Succeeded;
    let safe_error = if succeeded {
        None
    } else {
        Some(match website_operation {
            WebsiteOperation::Apply => "Website configuration could not be applied",
            WebsiteOperation::Delete => "Website deletion could not be completed",
        })
    };
    state
        .control_store
        .lock()
        .await
        .complete_website_operation(
            site_id,
            request_id,
            website_operation,
            succeeded,
            safe_error,
            &Utc::now().to_rfc3339(),
        )
        .map_err(|_| node_jobs::internal_error())?;
    Ok(())
}

fn validate_domain(domain: &str) -> Result<String, ErrorResponse> {
    if domain != domain.trim() || domain.ends_with('.') {
        return Err(node_jobs::bad_request());
    }
    let parsed = domain
        .parse::<DomainName>()
        .map_err(|_| node_jobs::bad_request())?;
    if parsed.to_string().split('.').count() < 2 {
        return Err(node_jobs::bad_request());
    }
    Ok(parsed.to_string().to_ascii_lowercase())
}

fn validate_document_root(document_root: &str, domain: &str) -> Result<String, ErrorResponse> {
    if document_root.is_empty()
        || !document_root.starts_with('/')
        || document_root.contains('\\')
        || document_root.chars().any(char::is_control)
    {
        return Err(node_jobs::bad_request());
    }
    let parsed = FsPath::new(document_root);
    let mut normalized = PathBuf::new();
    for component in parsed.components() {
        match component {
            Component::RootDir => normalized.push("/"),
            Component::Normal(part) => {
                let part = part.to_str().ok_or_else(node_jobs::bad_request)?;
                if part.is_empty()
                    || part
                        .bytes()
                        .any(|byte| !(byte.is_ascii_alphanumeric() || b"._-".contains(&byte)))
                {
                    return Err(node_jobs::bad_request());
                }
                normalized.push(part);
            }
            Component::CurDir | Component::ParentDir | Component::Prefix(_) => {
                return Err(node_jobs::bad_request());
            }
        }
    }
    let allowed_root = PathBuf::from(format!("/srv/fusionpanel/sites/{domain}"));
    if !normalized.starts_with(&allowed_root) {
        return Err(node_jobs::bad_request());
    }
    reject_existing_symlink_components(&normalized)?;
    Ok(normalized.to_string_lossy().into_owned())
}

fn reject_existing_symlink_components(path: &FsPath) -> Result<(), ErrorResponse> {
    let mut current = PathBuf::new();
    for component in path.components() {
        current.push(component.as_os_str());
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(node_jobs::bad_request());
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(node_jobs::bad_request()),
        }
    }
    Ok(())
}

fn validate_upstream_url(upstream_url: &str) -> Result<String, ErrorResponse> {
    if upstream_url.is_empty()
        || upstream_url.len() > MAX_UPSTREAM_URL_BYTES
        || !upstream_url.is_ascii()
        || upstream_url.chars().any(|character| {
            character.is_control()
                || character.is_whitespace()
                || b";{}\\'\"`".contains(&(character as u8))
        })
        || upstream_url.contains('?')
        || upstream_url.contains('#')
    {
        return Err(node_jobs::bad_request());
    }
    let uri = upstream_url
        .parse::<axum::http::Uri>()
        .map_err(|_| node_jobs::bad_request())?;
    let scheme = uri.scheme_str().ok_or_else(node_jobs::bad_request)?;
    if !matches!(scheme, "http" | "https") {
        return Err(node_jobs::bad_request());
    }
    let authority = uri.authority().ok_or_else(node_jobs::bad_request)?;
    let host = authority.host();
    if host.is_empty()
        || authority.as_str().contains('@')
        || authority.as_str().ends_with(':')
        || !host.is_ascii()
    {
        return Err(node_jobs::bad_request());
    }
    let ip_host = host
        .strip_prefix('[')
        .and_then(|host| host.strip_suffix(']'))
        .unwrap_or(host);
    if ip_host.parse::<std::net::IpAddr>().is_err() && ip_host.parse::<DomainName>().is_err() {
        return Err(node_jobs::bad_request());
    }
    if let Some(port) = authority.port_u16() {
        if port == 0 {
            return Err(node_jobs::bad_request());
        }
    } else if authority.port().is_some() {
        return Err(node_jobs::bad_request());
    }
    Ok(uri.to_string())
}

fn idempotency_key(headers: &HeaderMap) -> Result<String, ErrorResponse> {
    let key = headers
        .get("Idempotency-Key")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| {
            !value.is_empty()
                && value.len() <= 255
                && value.is_ascii()
                && !value.bytes().any(|byte| byte.is_ascii_control())
        })
        .ok_or_else(node_jobs::bad_request)?;
    Ok(key.to_string())
}

fn map_store_error(error: fusionpanel_control_store::StoreError) -> ErrorResponse {
    match error {
        fusionpanel_control_store::StoreError::WebsiteNotFound(_) => ErrorResponse {
            error: ErrorDetail {
                code: "not_found",
                message: "Not found",
            },
        },
        fusionpanel_control_store::StoreError::WebsiteConflict
        | fusionpanel_control_store::StoreError::IdempotencyConflict => ErrorResponse {
            error: ErrorDetail {
                code: "conflict",
                message: "Website request conflicts with existing state",
            },
        },
        fusionpanel_control_store::StoreError::WebsiteOperationInProgress => ErrorResponse {
            error: ErrorDetail {
                code: "conflict",
                message: "Website is still provisioning; retry deletion after it completes",
            },
        },
        _ => node_jobs::internal_error(),
    }
}

pub(super) struct WebsiteAuthorizationContext {
    pub(super) tenant_id: identity::TenantId,
    pub(super) is_admin_token: bool,
    pub(super) user_id: Option<identity::UserId>,
    pub(super) role_bindings: Vec<identity::RoleBinding>,
    pub(super) token_scopes: Option<Vec<String>>,
}

fn scope_allows(scopes: &[String], permission: Permission) -> bool {
    let exact = permission.to_string();
    let is_read = matches!(
        permission,
        Permission::SiteRead | Permission::TenantRead | Permission::SystemAuditRead
    );
    scopes.iter().any(|s| {
        s == "*"
            || s == &exact
            || (is_read && (s == "read" || s == "sites:read" || s == "site:read"))
    })
}

pub(super) fn require_website_permission(
    context: &WebsiteAuthorizationContext,
    permission: Permission,
    site_id: Option<Uuid>,
) -> Result<(), ErrorResponse> {
    if let Some(scopes) = &context.token_scopes {
        if !scope_allows(scopes, permission) {
            return Err(ErrorResponse {
                error: ErrorDetail {
                    code: "forbidden",
                    message: "API token scope does not allow this operation",
                },
            });
        }
    }
    if context.is_admin_token {
        return Ok(());
    }
    let user_id = context.user_id.ok_or_else(node_jobs::unauthorized)?;
    let scope = match site_id {
        Some(site_id) => Scope::Site {
            tenant_id: context.tenant_id,
            site_id: identity::SiteId::from(site_id),
        },
        None => Scope::Tenant {
            tenant_id: context.tenant_id,
        },
    };
    AccessControl::evaluate_principal(user_id, &context.role_bindings, permission, &scope).map_err(
        |_| ErrorResponse {
            error: ErrorDetail {
                code: "forbidden",
                message: "Permission denied",
            },
        },
    )
}

pub(super) async fn website_authorization_context(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<WebsiteAuthorizationContext, ErrorResponse> {
    if node_jobs::authorized_admin(state, headers) {
        let tenant_id = headers
            .get(TENANT_CONTEXT_HEADER)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<Uuid>().ok())
            .ok_or_else(node_jobs::bad_request)?;
        let exists = state
            .control_store
            .lock()
            .await
            .tenant_exists(tenant_id)
            .map_err(|_| node_jobs::internal_error())?;
        if !exists {
            return Err(ErrorResponse {
                error: ErrorDetail {
                    code: "not_found",
                    message: "Tenant not found",
                },
            });
        }
        return Ok(WebsiteAuthorizationContext {
            tenant_id: identity::TenantId::from(tenant_id),
            is_admin_token: true,
            user_id: None,
            role_bindings: Vec::new(),
            token_scopes: None,
        });
    }

    let raw_bearer = headers
        .get(AUTHORIZATION)
        .and_then(|header| header.to_str().ok())
        .and_then(|header| header.strip_prefix("Bearer "))
        .filter(|token| !token.is_empty())
        .map(str::to_owned);

    if let Some(ref bearer) = raw_bearer {
        if bearer.starts_with("fp_tok_") {
            let token_hash = node_jobs::hash_token(bearer);
            let token_rec = state
                .control_store
                .lock()
                .await
                .lookup_active_api_token_by_hash(&token_hash)
                .map_err(|_| node_jobs::internal_error())?
                .ok_or_else(node_jobs::unauthorized)?;
            let user_id = identity::UserId::from(token_rec.user_id);
            let bindings = state
                .identity_store
                .lock()
                .await
                .get_user_role_bindings(user_id)
                .map_err(|_| node_jobs::internal_error())?;
            return Ok(WebsiteAuthorizationContext {
                tenant_id: identity::TenantId::from(token_rec.tenant_id),
                is_admin_token: bindings.is_empty() && token_rec.user_id == token_rec.tenant_id,
                user_id: Some(user_id),
                role_bindings: bindings,
                token_scopes: Some(token_rec.scopes),
            });
        }
    }

    let session_id = raw_bearer
        .or_else(|| super::extract_session_from_cookies(headers))
        .ok_or_else(node_jobs::unauthorized)?;
    let session_id = session_id
        .parse::<SessionId>()
        .map_err(|_| node_jobs::unauthorized())?;

    let now = Utc::now();
    let identity_store = state.identity_store.lock().await;
    let Some(mut session) = identity_store
        .get_session(session_id)
        .map_err(|_| node_jobs::unauthorized())?
    else {
        return Err(node_jobs::unauthorized());
    };
    if session.is_revoked() || session.validate_and_touch(now).is_err() {
        return Err(node_jobs::unauthorized());
    }
    identity_store
        .update_session_touch(session_id, now)
        .map_err(|_| node_jobs::internal_error())?;
    let bindings = identity_store
        .get_user_role_bindings(session.user_id())
        .map_err(|_| node_jobs::internal_error())?;

    Ok(WebsiteAuthorizationContext {
        tenant_id: session.tenant_id(),
        is_admin_token: false,
        user_id: Some(session.user_id()),
        role_bindings: bindings,
        token_scopes: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{header::AUTHORIZATION, Request, Response},
    };
    use http_body_util::BodyExt;
    use serde_json::Value;
    use tempfile::TempDir;
    use tower::ServiceExt;

    const ADMIN_TOKEN: &str = "websites-test-admin-token";
    const ADMIN_TENANT_ID: &str = "00000000-0000-0000-0000-000000000001";

    fn test_app() -> (
        Router,
        TempDir,
        std::sync::Arc<tokio::sync::Mutex<identity::SqliteIdentityStore>>,
    ) {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("websites.sqlite");
        let identity_store = std::sync::Arc::new(tokio::sync::Mutex::new(
            identity::SqliteIdentityStore::open(&db_path).unwrap(),
        ));
        let admin_tenant_id = Uuid::from_u128(1);
        let mut control_store = fusionpanel_control_store::ControlStore::new(&db_path).unwrap();
        control_store
            .create_resource(
                admin_tenant_id,
                admin_tenant_id,
                fusionpanel_control_store::OwnershipMode::PlatformManaged,
                json!({ "name": "Admin Test Tenant" }),
            )
            .unwrap();
        let state = AppState {
            admin_token: std::sync::Arc::new(ADMIN_TOKEN.to_string()),
            identity_store: identity_store.clone(),
            control_store: std::sync::Arc::new(tokio::sync::Mutex::new(control_store)),
            jobs_store: std::sync::Arc::new(tokio::sync::Mutex::new(
                fusionpanel_jobs::JobStore::new(&db_path).unwrap(),
            )),
            frontend_dir: None,
            metrics_store: crate::metrics::shared_store(),
            alerts_store: crate::alerts::test_store(),
        };
        (super::super::app(state), dir, identity_store)
    }

    async fn session_token(
        identity_store: &std::sync::Arc<tokio::sync::Mutex<identity::SqliteIdentityStore>>,
        role: identity::Role,
        scope: identity::Scope,
    ) -> String {
        let tenant_id = match &scope {
            identity::Scope::Global => identity::TenantId::new_v4(),
            identity::Scope::Tenant { tenant_id } | identity::Scope::Site { tenant_id, .. } => {
                *tenant_id
            }
        };
        session_token_with_roles(identity_store, tenant_id, vec![(role, scope)]).await
    }

    async fn session_token_with_roles(
        identity_store: &std::sync::Arc<tokio::sync::Mutex<identity::SqliteIdentityStore>>,
        tenant_id: identity::TenantId,
        roles_and_scopes: Vec<(identity::Role, identity::Scope)>,
    ) -> String {
        let user = identity::User::new(
            format!("website-test-{}", Uuid::new_v4()),
            format!("{}@example.test", Uuid::new_v4()),
            "synthetic-password-hash".to_string(),
        )
        .unwrap();
        let session = identity::Session::new(user.id(), tenant_id, 3600, 1800).unwrap();
        let token = session.id().to_string();
        let identity_store = identity_store.lock().await;
        identity_store.create_user(&user).unwrap();
        for (role, scope) in roles_and_scopes {
            identity_store
                .save_role_binding(&identity::RoleBinding {
                    user_id: user.id(),
                    role,
                    scope,
                })
                .unwrap();
        }
        identity_store.save_session(&session).unwrap();
        token
    }

    async fn send(
        app: &Router,
        method: &str,
        uri: &str,
        token: Option<&str>,
        idempotency_key: Option<&str>,
        body: Option<Value>,
    ) -> Response<Body> {
        let tenant_context = if token == Some(ADMIN_TOKEN) {
            Some(ADMIN_TENANT_ID)
        } else {
            None
        };
        send_with_tenant_context(
            app,
            method,
            uri,
            token,
            idempotency_key,
            body,
            tenant_context,
        )
        .await
    }

    async fn send_with_tenant_context(
        app: &Router,
        method: &str,
        uri: &str,
        token: Option<&str>,
        idempotency_key: Option<&str>,
        body: Option<Value>,
        tenant_context: Option<&str>,
    ) -> Response<Body> {
        let mut builder = Request::builder().method(method).uri(uri);
        if let Some(token) = token {
            builder = builder.header(AUTHORIZATION, format!("Bearer {token}"));
        }
        if let Some(tenant_id) = tenant_context {
            builder = builder.header(TENANT_CONTEXT_HEADER, tenant_id);
        }
        if let Some(key) = idempotency_key {
            builder = builder.header("Idempotency-Key", key);
        }
        if body.is_some() {
            builder = builder.header("content-type", "application/json");
        }
        let body = body
            .map(|value| Body::from(value.to_string()))
            .unwrap_or_else(Body::empty);
        app.clone()
            .oneshot(builder.body(body).unwrap())
            .await
            .unwrap()
    }

    async fn response_json(response: Response<Body>) -> Value {
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap()
    }

    async fn claim_job(app: &Router, node_id: &str, token: &str) -> Value {
        let response = send(
            app,
            "POST",
            "/v1/jobs/claim",
            Some(token),
            None,
            Some(json!({ "node_id": node_id })),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        response_json(response).await["job"].clone()
    }

    async fn complete_claimed_job(
        app: &Router,
        job_id: &str,
        node_id: &str,
        token: &str,
        state: &str,
        error: Option<&str>,
    ) {
        let mut body = json!({ "node_id": node_id, "state": state });
        if let Some(error) = error {
            body["error_text"] = json!(error);
        }
        let response = send(
            app,
            "POST",
            &format!("/v1/jobs/{job_id}/complete"),
            Some(token),
            None,
            Some(body),
        )
        .await;
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
    }

    #[tokio::test]
    async fn website_routes_validate_auth_and_emit_the_agent_contract() {
        let (app, _dir, _identity_store) = test_app();
        let unauthenticated = send(&app, "GET", "/v1/websites", None, None, None).await;
        assert_eq!(unauthenticated.status(), StatusCode::UNAUTHORIZED);

        let missing_key = send(
            &app,
            "POST",
            "/v1/websites",
            Some(ADMIN_TOKEN),
            None,
            Some(json!({
                "kind": "static",
                "domain": "example.com",
                "document_root": "/srv/fusionpanel/sites/example.com",
            })),
        )
        .await;
        assert_eq!(missing_key.status(), StatusCode::BAD_REQUEST);

        let invalid_domain = send(
            &app,
            "POST",
            "/v1/websites",
            Some(ADMIN_TOKEN),
            Some("invalid-domain"),
            Some(json!({
                "kind": "static",
                "domain": "127.0.0.1",
                "document_root": "/srv/fusionpanel/sites/127.0.0.1",
            })),
        )
        .await;
        assert_eq!(invalid_domain.status(), StatusCode::BAD_REQUEST);

        let created = send(
            &app,
            "POST",
            "/v1/websites",
            Some(ADMIN_TOKEN),
            Some("create-example"),
            Some(json!({
                "kind": "static",
                "domain": "Example.COM",
                "document_root": "/srv/fusionpanel/sites/example.com/public",
            })),
        )
        .await;
        assert_eq!(created.status(), StatusCode::ACCEPTED);
        let created = response_json(created).await;
        let site_id = created["site"]["id"].as_str().unwrap().to_string();
        let job_id = created["job"]["id"].as_str().unwrap().to_string();
        assert_eq!(created["site"]["domain"], "example.com");
        assert_eq!(created["site"]["state"], "provisioning");
        assert_eq!(created["job"]["kind"], "agent_operation");
        assert_eq!(created["job"]["payload"]["version"], 1);
        assert_eq!(created["job"]["payload"]["operation"], "site.apply");
        assert_eq!(created["job"]["payload"]["payload"]["site_id"], site_id);
        assert_eq!(
            created["job"]["payload"]["payload"]["domain"],
            "example.com"
        );
        assert_eq!(
            created["job"]["payload"]["payload"]["spec"]["kind"],
            "static"
        );
        assert_eq!(
            created["job"]["payload"]["payload"]["spec"]["document_root"],
            "/srv/fusionpanel/sites/example.com/public"
        );

        let replay = send(
            &app,
            "POST",
            "/v1/websites",
            Some(ADMIN_TOKEN),
            Some("create-example"),
            Some(json!({
                "kind": "static",
                "domain": "example.com",
                "document_root": "/srv/fusionpanel/sites/example.com/public",
            })),
        )
        .await;
        assert_eq!(replay.status(), StatusCode::ACCEPTED);
        let replay = response_json(replay).await;
        assert_eq!(replay["site"]["id"], site_id);
        assert_eq!(replay["job"]["id"], job_id);

        let changed_replay = send(
            &app,
            "POST",
            "/v1/websites",
            Some(ADMIN_TOKEN),
            Some("create-example"),
            Some(json!({
                "kind": "reverse_proxy",
                "domain": "example.com",
                "upstream_url": "http://127.0.0.1:3000",
            })),
        )
        .await;
        assert_eq!(changed_replay.status(), StatusCode::CONFLICT);

        let listed = send(&app, "GET", "/v1/websites", Some(ADMIN_TOKEN), None, None).await;
        assert_eq!(listed.status(), StatusCode::OK);
        assert_eq!(
            response_json(listed).await["websites"]
                .as_array()
                .unwrap()
                .len(),
            1
        );

        let detail = send(
            &app,
            "GET",
            &format!("/v1/websites/{site_id}"),
            Some(ADMIN_TOKEN),
            None,
            None,
        )
        .await;
        let detail = response_json(detail).await;
        assert!(detail.get("site").is_none());
        assert_eq!(detail["website"]["id"], site_id);
    }

    #[tokio::test]
    async fn website_routes_enforce_tenant_ownership_and_rbac_scopes() {
        let (app, _dir, identity_store) = test_app();
        let tenant_a = identity::TenantId::new_v4();
        let tenant_b = identity::TenantId::new_v4();
        let tenant_a_admin = session_token(
            &identity_store,
            identity::Role::super_admin(),
            identity::Scope::Tenant {
                tenant_id: tenant_a,
            },
        )
        .await;

        let create = send(
            &app,
            "POST",
            "/v1/websites",
            Some(&tenant_a_admin),
            Some("tenant-a-create"),
            Some(json!({
                "kind": "reverse_proxy",
                "domain": "tenant-a.example.com",
                "upstream_url": "http://127.0.0.1:3000",
            })),
        )
        .await;
        assert_eq!(create.status(), StatusCode::ACCEPTED);
        let site = response_json(create).await["site"].clone();
        let site_id = site["id"].as_str().unwrap();
        assert_eq!(site["tenant_id"], tenant_a.to_string());

        let tenant_a_list = send(
            &app,
            "GET",
            "/v1/websites",
            Some(&tenant_a_admin),
            None,
            None,
        )
        .await;
        assert_eq!(tenant_a_list.status(), StatusCode::OK);
        assert_eq!(
            response_json(tenant_a_list).await["websites"]
                .as_array()
                .unwrap()
                .len(),
            1
        );

        let tenant_b_admin = session_token(
            &identity_store,
            identity::Role::tenant_admin(),
            identity::Scope::Tenant {
                tenant_id: tenant_b,
            },
        )
        .await;
        let tenant_b_list = send(
            &app,
            "GET",
            "/v1/websites",
            Some(&tenant_b_admin),
            None,
            None,
        )
        .await;
        assert_eq!(tenant_b_list.status(), StatusCode::OK);
        assert!(response_json(tenant_b_list).await["websites"]
            .as_array()
            .unwrap()
            .is_empty());

        let foreign_detail = send(
            &app,
            "GET",
            &format!("/v1/websites/{site_id}"),
            Some(&tenant_b_admin),
            None,
            None,
        )
        .await;
        assert_eq!(foreign_detail.status(), StatusCode::NOT_FOUND);
        let foreign_delete = send(
            &app,
            "DELETE",
            &format!("/v1/websites/{site_id}"),
            Some(&tenant_b_admin),
            Some("tenant-b-delete-foreign"),
            None,
        )
        .await;
        assert_eq!(foreign_delete.status(), StatusCode::NOT_FOUND);

        let site_operator = session_token(
            &identity_store,
            identity::Role::site_operator(),
            identity::Scope::Site {
                tenant_id: tenant_a,
                site_id: identity::SiteId::from(Uuid::parse_str(site_id).unwrap()),
            },
        )
        .await;
        let site_detail = send(
            &app,
            "GET",
            &format!("/v1/websites/{site_id}"),
            Some(&site_operator),
            None,
            None,
        )
        .await;
        assert_eq!(site_detail.status(), StatusCode::OK);
        let forbidden_delete = send(
            &app,
            "DELETE",
            &format!("/v1/websites/{site_id}"),
            Some(&site_operator),
            Some("site-operator-delete"),
            None,
        )
        .await;
        assert_eq!(forbidden_delete.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn admin_token_requires_an_explicit_existing_tenant_context() {
        let (app, _dir, _identity_store) = test_app();
        let missing = send_with_tenant_context(
            &app,
            "GET",
            "/v1/websites",
            Some(ADMIN_TOKEN),
            None,
            None,
            None,
        )
        .await;
        assert_eq!(missing.status(), StatusCode::BAD_REQUEST);

        let malformed = send_with_tenant_context(
            &app,
            "GET",
            "/v1/websites",
            Some(ADMIN_TOKEN),
            None,
            None,
            Some("not-a-uuid"),
        )
        .await;
        assert_eq!(malformed.status(), StatusCode::BAD_REQUEST);

        let unknown = send_with_tenant_context(
            &app,
            "GET",
            "/v1/websites",
            Some(ADMIN_TOKEN),
            None,
            None,
            Some("00000000-0000-0000-0000-000000000002"),
        )
        .await;
        assert_eq!(unknown.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn agent_completion_persists_site_state_and_delete_waits_for_success() {
        let (app, _dir, _identity_store) = test_app();
        let created = send(
            &app,
            "POST",
            "/v1/websites",
            Some(ADMIN_TOKEN),
            Some("create-app"),
            Some(json!({
                "kind": "reverse_proxy",
                "domain": "app.example.com",
                "upstream_url": "http://127.0.0.1:3000/api",
            })),
        )
        .await;
        assert_eq!(created.status(), StatusCode::ACCEPTED);
        let created = response_json(created).await;
        let site_id = created["site"]["id"].as_str().unwrap().to_string();
        assert_eq!(
            created["job"]["payload"]["payload"]["spec"]["kind"],
            "reverse_proxy"
        );
        assert_eq!(
            created["job"]["payload"]["payload"]["spec"]["upstream_url"],
            "http://127.0.0.1:3000/api"
        );

        let early_delete = send(
            &app,
            "DELETE",
            &format!("/v1/websites/{site_id}"),
            Some(ADMIN_TOKEN),
            Some("delete-while-provisioning"),
            None,
        )
        .await;
        assert_eq!(early_delete.status(), StatusCode::CONFLICT);
        let early_delete = response_json(early_delete).await;
        assert_eq!(early_delete["error"]["code"], "conflict");
        assert!(early_delete["error"]["message"]
            .as_str()
            .unwrap()
            .contains("still provisioning"));

        let registered = send(
            &app,
            "POST",
            "/v1/nodes/register",
            Some(ADMIN_TOKEN),
            None,
            Some(json!({ "name": "websites-test-node" })),
        )
        .await;
        assert_eq!(registered.status(), StatusCode::CREATED);
        let registered = response_json(registered).await;
        let node_id = registered["node_id"].as_str().unwrap();
        let node_token = registered["token"].as_str().unwrap();

        let apply_job = claim_job(&app, node_id, node_token).await;
        assert_eq!(apply_job["state"], "running");
        complete_claimed_job(
            &app,
            apply_job["id"].as_str().unwrap(),
            node_id,
            node_token,
            "succeeded",
            None,
        )
        .await;
        let detail = send(
            &app,
            "GET",
            &format!("/v1/websites/{site_id}"),
            Some(ADMIN_TOKEN),
            None,
            None,
        )
        .await;
        assert_eq!(response_json(detail).await["website"]["state"], "active");

        let deletion = send(
            &app,
            "DELETE",
            &format!("/v1/websites/{site_id}"),
            Some(ADMIN_TOKEN),
            Some("delete-while-provisioning"),
            None,
        )
        .await;
        assert_eq!(deletion.status(), StatusCode::ACCEPTED);
        let deletion = response_json(deletion).await;
        let delete_job_id = deletion["job"]["id"].as_str().unwrap().to_string();
        assert_eq!(deletion["site"]["state"], "deleting");
        assert_eq!(deletion["job"]["payload"]["operation"], "site.delete");
        assert_eq!(deletion["job"]["payload"]["payload"]["site_id"], site_id);
        assert_eq!(
            deletion["job"]["payload"]["payload"]["domain"],
            "app.example.com"
        );

        let delete_job = claim_job(&app, node_id, node_token).await;
        assert_eq!(delete_job["id"], delete_job_id);
        complete_claimed_job(
            &app,
            &delete_job_id,
            node_id,
            node_token,
            "failed",
            Some("backend secret: do-not-persist-this"),
        )
        .await;
        let detail = send(
            &app,
            "GET",
            &format!("/v1/websites/{site_id}"),
            Some(ADMIN_TOKEN),
            None,
            None,
        )
        .await;
        let failed = response_json(detail).await["website"].clone();
        assert_eq!(failed["state"], "failed");
        assert_eq!(
            failed["last_error"],
            "Website deletion could not be completed"
        );
        assert!(!failed.to_string().contains("do-not-persist-this"));

        let deletion_replay = send(
            &app,
            "DELETE",
            &format!("/v1/websites/{site_id}"),
            Some(ADMIN_TOKEN),
            Some("delete-while-provisioning"),
            None,
        )
        .await;
        assert_eq!(deletion_replay.status(), StatusCode::ACCEPTED);
        let deletion_replay = response_json(deletion_replay).await;
        assert_eq!(deletion_replay["job"]["id"], delete_job_id);

        let retry = send(
            &app,
            "DELETE",
            &format!("/v1/websites/{site_id}"),
            Some(ADMIN_TOKEN),
            Some("delete-app-retry"),
            None,
        )
        .await;
        assert_eq!(retry.status(), StatusCode::ACCEPTED);
        let retry = response_json(retry).await;
        let retry_job_id = retry["job"]["id"].as_str().unwrap().to_string();
        let retry_job = claim_job(&app, node_id, node_token).await;
        assert_eq!(retry_job["id"], retry_job_id);
        complete_claimed_job(&app, &retry_job_id, node_id, node_token, "succeeded", None).await;
        let removed = send(
            &app,
            "GET",
            &format!("/v1/websites/{site_id}"),
            Some(ADMIN_TOKEN),
            None,
            None,
        )
        .await;
        assert_eq!(removed.status(), StatusCode::NOT_FOUND);

        let deletion_replay = send(
            &app,
            "DELETE",
            &format!("/v1/websites/{site_id}"),
            Some(ADMIN_TOKEN),
            Some("delete-app-retry"),
            None,
        )
        .await;
        assert_eq!(deletion_replay.status(), StatusCode::ACCEPTED);
        let deletion_replay = response_json(deletion_replay).await;
        assert_eq!(deletion_replay["site"]["id"], site_id);
        assert_eq!(deletion_replay["job"]["id"], retry_job_id);
    }

    #[test]
    fn website_input_validation_rejects_traversal_and_unsafe_upstreams() {
        assert!(validate_domain("Example.COM").is_ok());
        assert!(validate_domain("localhost").is_err());
        assert!(validate_domain("example.com.").is_err());
        assert!(validate_document_root(
            "/srv/fusionpanel/sites/example.com/../other",
            "example.com"
        )
        .is_err());
        assert!(validate_document_root(
            "/srv/fusionpanel/sites/other.example.com/public",
            "example.com"
        )
        .is_err());
        assert!(validate_upstream_url("file:///etc/passwd").is_err());
        assert!(validate_upstream_url("http://user:pass@example.com").is_err());
        assert!(validate_upstream_url("http://example.com/?admin=1").is_err());
        assert!(validate_upstream_url("http://example.com;include=/tmp/x").is_err());
        assert!(validate_upstream_url("http://example.com:0").is_err());
        assert!(validate_upstream_url("http://example.com:").is_err());
        assert!(validate_upstream_url("http://bad_host:8080").is_err());
        assert!(validate_upstream_url("https://backend.example.com:8443/api").is_ok());

        let prefix = "https://backend.example.com/";
        let at_limit = format!(
            "{prefix}{}",
            "a".repeat(MAX_UPSTREAM_URL_BYTES - prefix.len())
        );
        let over_limit = format!("{at_limit}a");
        assert_eq!(at_limit.len(), MAX_UPSTREAM_URL_BYTES);
        assert!(validate_upstream_url(&at_limit).is_ok());
        assert_eq!(over_limit.len(), MAX_UPSTREAM_URL_BYTES + 1);
        assert!(validate_upstream_url(&over_limit).is_err());
    }

    #[tokio::test]
    async fn tls_lifecycle_endpoints_and_private_key_redaction() {
        let (app, _dir, _identity_store) = test_app();

        let created = send(
            &app,
            "POST",
            "/v1/websites",
            Some(ADMIN_TOKEN),
            Some("site-tls-test-1"),
            Some(json!({
                "kind": "static",
                "domain": "tls-api.test",
                "document_root": "/srv/fusionpanel/sites/tls-api.test"
            })),
        )
        .await;
        assert_eq!(created.status(), StatusCode::ACCEPTED);
        let site_body = response_json(created).await;
        let site_id = site_body["site"]["id"].as_str().unwrap().to_string();
        let site_job_id = site_body["job"]["id"].as_str().unwrap().to_string();

        let reg_resp = send(
            &app,
            "POST",
            "/v1/nodes/register",
            Some(ADMIN_TOKEN),
            None,
            Some(json!({ "name": "tls-node" })),
        )
        .await;
        assert_eq!(reg_resp.status(), StatusCode::CREATED);
        let reg_json = response_json(reg_resp).await;
        let node_id = reg_json["node_id"].as_str().unwrap().to_string();
        let node_token = reg_json["token"].as_str().unwrap().to_string();

        let claimed_site = claim_job(&app, &node_id, &node_token).await;
        assert_eq!(claimed_site["id"], site_job_id);
        complete_claimed_job(&app, &site_job_id, &node_id, &node_token, "succeeded", None).await;

        let tls_apply_resp = send(
            &app,
            "POST",
            &format!("/v1/websites/{site_id}/tls"),
            Some(ADMIN_TOKEN),
            Some("tls-apply-1"),
            Some(json!({
                "mode": "custom_pem",
                "certificate_pem": "-----BEGIN CERTIFICATE-----\nMIIB\n-----END CERTIFICATE-----",
                "private_key_pem": "-----BEGIN PRIVATE KEY-----\nSECRET_KEY_MATERIAL\n-----END PRIVATE KEY-----"
            })),
        )
        .await;
        assert_eq!(tls_apply_resp.status(), StatusCode::ACCEPTED);
        let tls_body = response_json(tls_apply_resp).await;
        let tls_text = tls_body.to_string();
        assert!(!tls_text.contains("SECRET_KEY_MATERIAL"));
        assert!(tls_text.contains("[REDACTED]"));
        assert_eq!(tls_body["tls"]["state"], "provisioning");
        let tls_job_id = tls_body["job"]["id"].as_str().unwrap().to_string();

        // Agent claim MUST still receive full private_key_pem
        let claimed_tls = claim_job(&app, &node_id, &node_token).await;
        assert_eq!(claimed_tls["id"], tls_job_id);
        assert!(claimed_tls.to_string().contains("SECRET_KEY_MATERIAL"));
        complete_claimed_job(&app, &tls_job_id, &node_id, &node_token, "succeeded", None).await;

        // GET /v1/websites/{id}/tls reports active without exposing key material
        let get_tls_resp = send(
            &app,
            "GET",
            &format!("/v1/websites/{site_id}/tls"),
            Some(ADMIN_TOKEN),
            None,
            None,
        )
        .await;
        assert_eq!(get_tls_resp.status(), StatusCode::OK);
        let get_tls_json = response_json(get_tls_resp).await;
        assert_eq!(get_tls_json["tls"]["state"], "active");
        assert_eq!(get_tls_json["tls"]["mode"], "custom_pem");

        // DELETE /v1/websites/{id}/tls transitions through deleting -> disabled
        let del_resp = send(
            &app,
            "DELETE",
            &format!("/v1/websites/{site_id}/tls"),
            Some(ADMIN_TOKEN),
            Some("tls-del-1"),
            None,
        )
        .await;
        assert_eq!(del_resp.status(), StatusCode::ACCEPTED);
        let del_json = response_json(del_resp).await;
        let del_job_id = del_json["job"]["id"].as_str().unwrap().to_string();
        let claimed_del = claim_job(&app, &node_id, &node_token).await;
        assert_eq!(claimed_del["id"], del_job_id);
        complete_claimed_job(&app, &del_job_id, &node_id, &node_token, "succeeded", None).await;

        let get_tls_after = send(
            &app,
            "GET",
            &format!("/v1/websites/{site_id}/tls"),
            Some(ADMIN_TOKEN),
            None,
            None,
        )
        .await;
        assert_eq!(get_tls_after.status(), StatusCode::OK);
        let get_tls_after_json = response_json(get_tls_after).await;
        assert_eq!(get_tls_after_json["tls"]["state"], "disabled");
    }
}
