#![forbid(unsafe_code)]

use super::{
    node_jobs,
    websites::{require_website_permission, website_authorization_context},
    AppState, ErrorDetail, ErrorResponse,
};
use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::{delete, get, post, put},
    Json, Router,
};
use fusionpanel_control_store::WebsiteSpec;
use identity::Permission;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::fs;
use std::path::{Component, Path as FsPath, PathBuf};
use uuid::Uuid;

const MAX_FILE_BYTES: usize = 1024 * 1024; // 1 MiB

#[derive(Deserialize)]
struct PathQuery {
    #[serde(default)]
    path: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WriteFileRequest {
    path: String,
    content: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RenameFileRequest {
    from: String,
    to: String,
}

#[derive(Serialize)]
struct FileEntry {
    name: String,
    path: String,
    kind: &'static str,
    size: u64,
}

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route("/v1/files/{site_id}/list", get(list_files))
        .route("/api/v1/files/{site_id}/list", get(list_files))
        .route("/v1/files/{site_id}/read", get(read_file))
        .route("/api/v1/files/{site_id}/read", get(read_file))
        .route("/v1/files/{site_id}/write", put(write_file))
        .route("/api/v1/files/{site_id}/write", put(write_file))
        .route("/v1/files/{site_id}/rename", post(rename_file))
        .route("/api/v1/files/{site_id}/rename", post(rename_file))
        .route("/v1/files/{site_id}/delete", delete(delete_file))
        .route("/api/v1/files/{site_id}/delete", delete(delete_file))
}

async fn list_files(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(site_id_str): Path<String>,
    Query(query): Query<PathQuery>,
) -> Result<Json<serde_json::Value>, ErrorResponse> {
    let context = website_authorization_context(&state, &headers).await?;
    let site_id = parse_uuid(&site_id_str)?;
    require_website_permission(&context, Permission::SiteRead, Some(site_id))?;

    let root = resolve_site_root(&state, site_id, context.tenant_id.to_uuid()).await?;
    fs::create_dir_all(&root).map_err(|_| node_jobs::internal_error())?;

    let target_dir = if query.path.trim().is_empty() || query.path.trim() == "/" {
        fs::canonicalize(&root).map_err(|_| node_jobs::internal_error())?
    } else {
        resolve_jailed_path(&root, &query.path, true)?
    };
    if !target_dir.is_dir() {
        return Err(bad_request("Target path is not a directory"));
    }

    let canonical_root = fs::canonicalize(&root).map_err(|_| node_jobs::internal_error())?;
    let mut entries = Vec::new();
    let rd = fs::read_dir(&target_dir).map_err(|_| node_jobs::internal_error())?;
    for item in rd {
        let item = item.map_err(|_| node_jobs::internal_error())?;
        let meta = item.metadata().map_err(|_| node_jobs::internal_error())?;
        let name = item.file_name().to_string_lossy().to_string();
        let rel = item
            .path()
            .strip_prefix(&canonical_root)
            .unwrap_or(&item.path())
            .to_string_lossy()
            .replace('\\', "/");
        entries.push(FileEntry {
            name,
            path: rel,
            kind: if meta.is_dir() { "dir" } else { "file" },
            size: if meta.is_file() { meta.len() } else { 0 },
        });
    }
    entries.sort_by(|a, b| a.kind.cmp(b.kind).then_with(|| a.name.cmp(&b.name)));
    Ok(Json(json!({ "entries": entries })))
}

async fn read_file(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(site_id_str): Path<String>,
    Query(query): Query<PathQuery>,
) -> Result<Json<serde_json::Value>, ErrorResponse> {
    let context = website_authorization_context(&state, &headers).await?;
    let site_id = parse_uuid(&site_id_str)?;
    require_website_permission(&context, Permission::SiteRead, Some(site_id))?;

    let root = resolve_site_root(&state, site_id, context.tenant_id.to_uuid()).await?;
    let target = resolve_jailed_path(&root, &query.path, true)?;
    if !target.is_file() {
        return Err(not_found());
    }
    let bytes = fs::read(&target).map_err(|_| node_jobs::internal_error())?;
    if bytes.len() > MAX_FILE_BYTES {
        return Err(bad_request("File exceeds 1 MiB read limit"));
    }
    let content =
        String::from_utf8(bytes).map_err(|_| bad_request("File is not valid UTF-8 text"))?;
    Ok(Json(json!({ "path": query.path, "content": content })))
}

async fn write_file(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(site_id_str): Path<String>,
    Json(body): Json<WriteFileRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>), ErrorResponse> {
    let context = website_authorization_context(&state, &headers).await?;
    let site_id = parse_uuid(&site_id_str)?;
    require_website_permission(&context, Permission::SiteUpdate, Some(site_id))?;

    if body.content.len() > MAX_FILE_BYTES {
        return Err(bad_request("File exceeds 1 MiB write limit"));
    }

    let root = resolve_site_root(&state, site_id, context.tenant_id.to_uuid()).await?;
    fs::create_dir_all(&root).map_err(|_| node_jobs::internal_error())?;
    let target = resolve_jailed_path(&root, &body.path, false)?;
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|_| node_jobs::internal_error())?;
        let canonical_root = fs::canonicalize(&root).map_err(|_| node_jobs::internal_error())?;
        let canonical_parent = fs::canonicalize(parent).map_err(|_| node_jobs::internal_error())?;
        if !canonical_parent.starts_with(&canonical_root) {
            return Err(bad_request("Parent directory escapes site jail"));
        }
    }

    let tmp_path = target.with_extension("fp-tmp");
    fs::write(&tmp_path, body.content.as_bytes()).map_err(|_| node_jobs::internal_error())?;
    fs::rename(&tmp_path, &target).map_err(|_| node_jobs::internal_error())?;
    Ok((
        StatusCode::OK,
        Json(json!({ "path": body.path, "size": body.content.len() })),
    ))
}

async fn rename_file(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(site_id_str): Path<String>,
    Json(body): Json<RenameFileRequest>,
) -> Result<Json<serde_json::Value>, ErrorResponse> {
    let context = website_authorization_context(&state, &headers).await?;
    let site_id = parse_uuid(&site_id_str)?;
    require_website_permission(&context, Permission::SiteUpdate, Some(site_id))?;

    let root = resolve_site_root(&state, site_id, context.tenant_id.to_uuid()).await?;
    let src = resolve_jailed_path(&root, &body.from, true)?;
    let dst = resolve_jailed_path(&root, &body.to, false)?;
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent).map_err(|_| node_jobs::internal_error())?;
    }
    fs::rename(&src, &dst).map_err(|_| node_jobs::internal_error())?;
    Ok(Json(json!({ "from": body.from, "to": body.to })))
}

async fn delete_file(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(site_id_str): Path<String>,
    Query(query): Query<PathQuery>,
) -> Result<StatusCode, ErrorResponse> {
    let context = website_authorization_context(&state, &headers).await?;
    let site_id = parse_uuid(&site_id_str)?;
    require_website_permission(&context, Permission::SiteUpdate, Some(site_id))?;

    let root = resolve_site_root(&state, site_id, context.tenant_id.to_uuid()).await?;
    let target = resolve_jailed_path(&root, &query.path, true)?;
    if target.is_dir() {
        fs::remove_dir(&target)
            .map_err(|_| bad_request("Directory must be empty before deletion"))?;
    } else {
        fs::remove_file(&target).map_err(|_| node_jobs::internal_error())?;
    }
    Ok(StatusCode::NO_CONTENT)
}

async fn resolve_site_root(
    state: &AppState,
    site_id: Uuid,
    tenant_id: Uuid,
) -> Result<PathBuf, ErrorResponse> {
    let site = state
        .control_store
        .lock()
        .await
        .get_website_for_tenant(site_id, tenant_id)
        .map_err(|e| match e {
            fusionpanel_control_store::StoreError::WebsiteNotFound(_)
            | fusionpanel_control_store::StoreError::NotFound(_) => not_found(),
            _ => node_jobs::internal_error(),
        })?;
    match site.spec {
        WebsiteSpec::Static { document_root } => Ok(PathBuf::from(document_root)),
        WebsiteSpec::ReverseProxy { .. } => Err(bad_request(
            "File manager is only available for static websites",
        )),
    }
}

fn resolve_jailed_path(
    root: &FsPath,
    raw_rel: &str,
    must_exist: bool,
) -> Result<PathBuf, ErrorResponse> {
    let rel = raw_rel.trim();
    if rel.is_empty()
        || rel.contains('\0')
        || rel.starts_with('/')
        || rel.starts_with('\\')
        || rel.contains('\\')
    {
        return Err(bad_request("Invalid relative file path"));
    }

    let mut clean_parts = Vec::new();
    for component in FsPath::new(rel).components() {
        match component {
            Component::Normal(part) => clean_parts.push(part.to_owned()),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(bad_request("Path traversal is not allowed"));
            }
        }
    }
    if clean_parts.is_empty() {
        return Err(bad_request("File path cannot be empty"));
    }

    let canonical_root = fs::canonicalize(root).map_err(|_| not_found())?;
    let mut cursor = canonical_root.clone();
    for (idx, part) in clean_parts.iter().enumerate() {
        cursor.push(part);
        if let Ok(meta) = fs::symlink_metadata(&cursor) {
            if meta.file_type().is_symlink() {
                let resolved = fs::canonicalize(&cursor)
                    .map_err(|_| bad_request("Symlink escape is not allowed"))?;
                if !resolved.starts_with(&canonical_root) {
                    return Err(bad_request(
                        "Symlink escape outside site root is not allowed",
                    ));
                }
            }
        } else if must_exist {
            return Err(not_found());
        } else {
            // Check remaining components don't contain anything tricky; already Normal components.
            let _ = idx;
        }
    }

    if must_exist {
        let resolved = fs::canonicalize(&cursor).map_err(|_| not_found())?;
        if !resolved.starts_with(&canonical_root) {
            return Err(bad_request("Resolved path escapes site root"));
        }
        return Ok(resolved);
    }

    Ok(cursor)
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
            message: "Site or file not found",
        },
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
    use fusionpanel_control_store::{ControlStore, WebsiteSpec};
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
            format!("files-test-{}", Uuid::new_v4()),
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
    async fn file_manager_jails_paths_rejects_traversal_and_enforces_tenant_isolation() {
        let temp = tempfile::tempdir().unwrap();
        let site_root = temp.path().join("app.local");
        std::fs::create_dir_all(&site_root).unwrap();

        let state = test_state();
        let tenant_a = Uuid::new_v4();
        let tenant_b = Uuid::new_v4();
        let sess_a = create_session(&state, tenant_a, Role::tenant_admin()).await;
        let sess_b = create_session(&state, tenant_b, Role::tenant_admin()).await;

        let res = state
            .control_store
            .lock()
            .await
            .reserve_website_create(
                "k1",
                &serde_json::json!({}),
                tenant_a,
                Uuid::new_v4(),
                Uuid::new_v4(),
                "app.local",
                WebsiteSpec::Static {
                    document_root: site_root.to_string_lossy().to_string(),
                },
                "2026-11-17T15:00:00Z",
            )
            .unwrap();
        let site_id = res.site.id;
        let router = app(state);

        // Traversal rejected
        let bad = router
            .clone()
            .oneshot(
                HttpRequest::builder()
                    .method("PUT")
                    .uri(format!("/v1/files/{site_id}/write"))
                    .header(header::AUTHORIZATION, format!("Bearer {sess_a}"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(r#"{"path":"../escape.txt","content":"bad"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(bad.status(), StatusCode::BAD_REQUEST);

        // Tenant B cannot access Tenant A's files
        let foreign = router
            .clone()
            .oneshot(
                HttpRequest::builder()
                    .method("GET")
                    .uri(format!("/v1/files/{site_id}/list"))
                    .header(header::AUTHORIZATION, format!("Bearer {sess_b}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(foreign.status(), StatusCode::NOT_FOUND);

        // Write -> Read -> Rename -> Delete roundtrip
        let put_ok = router
            .clone()
            .oneshot(
                HttpRequest::builder()
                    .method("PUT")
                    .uri(format!("/v1/files/{site_id}/write"))
                    .header(header::AUTHORIZATION, format!("Bearer {sess_a}"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(r#"{"path":"sub/hello.txt","content":"hi"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(put_ok.status(), StatusCode::OK);

        let read_ok = router
            .clone()
            .oneshot(
                HttpRequest::builder()
                    .method("GET")
                    .uri(format!("/v1/files/{site_id}/read?path=sub/hello.txt"))
                    .header(header::AUTHORIZATION, format!("Bearer {sess_a}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(read_ok.status(), StatusCode::OK);
    }
}
