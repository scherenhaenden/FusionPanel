#![forbid(unsafe_code)]

mod node_jobs;
mod websites;

use axum::{
    body::Body,
    extract::State,
    http::{
        header::{AUTHORIZATION, CONTENT_TYPE, COOKIE},
        StatusCode, Uri,
    },
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use chrono::Utc;
use fusionpanel_control_store::ControlStore;
use identity::{SessionId, SqliteIdentityStore};
use integration_identity::{auth_routes, AuthState};
use serde::Serialize;
use std::{
    env,
    path::{Component, Path, PathBuf},
    sync::Arc,
};
use subtle::ConstantTimeEq;
use tokio::net::TcpListener;
use tokio::signal;
use tokio::sync::Mutex;
use tower_cookies::CookieManagerLayer;

#[derive(Clone)]
struct AppState {
    admin_token: Arc<String>,
    identity_store: Arc<Mutex<SqliteIdentityStore>>,
    control_store: Arc<Mutex<ControlStore>>,
    jobs_store: Arc<Mutex<fusionpanel_jobs::JobStore>>,
    frontend_dir: Option<PathBuf>,
}

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
    service: &'static str,
    version: &'static str,
}

#[derive(Serialize)]
struct CapabilitiesResponse {
    capabilities: Vec<&'static str>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    unavailable: Vec<&'static str>,
}

#[derive(Serialize)]
struct ErrorResponse {
    error: ErrorDetail,
}

#[derive(Serialize)]
struct ErrorDetail {
    code: &'static str,
    message: &'static str,
}

impl IntoResponse for ErrorResponse {
    fn into_response(self) -> Response {
        let status = match self.error.code {
            "unauthorized" => StatusCode::UNAUTHORIZED,
            "not_found" => StatusCode::NOT_FOUND,
            "bad_request" => StatusCode::BAD_REQUEST,
            "conflict" => StatusCode::CONFLICT,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };
        (status, Json(self)).into_response()
    }
}

async fn health_handler() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        service: "fusionpanel-api",
        version: option_env!("FUSIONPANEL_BUILD_VERSION").unwrap_or("development"),
    })
}

fn extract_session_from_cookies(headers: &axum::http::HeaderMap) -> Option<String> {
    let cookie_header = headers.get(COOKIE)?.to_str().ok()?;
    for cookie in cookie_header.split(';') {
        let cookie = cookie.trim();
        if let Some(val) = cookie
            .strip_prefix("SESSION_ID=")
            .or_else(|| cookie.strip_prefix("session_id="))
        {
            if !val.is_empty() {
                return Some(val.to_string());
            }
        }
    }
    None
}

async fn validate_session_id(store: &Arc<Mutex<SqliteIdentityStore>>, session_id: &str) -> bool {
    let Ok(sid) = session_id.parse::<SessionId>() else {
        return false;
    };
    let store = store.lock().await;
    let Ok(Some(mut session)) = store.get_session(sid) else {
        return false;
    };
    if session.is_revoked() {
        return false;
    }
    let now = Utc::now();
    if session.validate_and_touch(now).is_err() {
        return false;
    }
    let _ = store.update_session_touch(sid, now);
    true
}

async fn capabilities_handler(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
) -> Result<Json<CapabilitiesResponse>, ErrorResponse> {
    let auth_header = headers.get(AUTHORIZATION).and_then(|h| h.to_str().ok());

    let mut authorized = false;
    let mut bearer_session: Option<&str> = None;

    if let Some(auth_str) = auth_header {
        if let Some(token) = auth_str.strip_prefix("Bearer ") {
            let provided = token.as_bytes();
            let expected = state.admin_token.as_bytes();
            if provided.len() == expected.len() && bool::from(provided.ct_eq(expected)) {
                authorized = true;
            } else if !token.is_empty() {
                bearer_session = Some(token);
            }
        }
    }

    if !authorized {
        if let Some(session_id) = bearer_session {
            authorized = validate_session_id(&state.identity_store, session_id).await;
        } else if let Some(session_id) = extract_session_from_cookies(&headers) {
            authorized = validate_session_id(&state.identity_store, &session_id).await;
        }
    }

    if authorized {
        Ok(Json(CapabilitiesResponse {
            capabilities: vec![
                "control-plane.health.read",
                "control-plane.capabilities.read",
            ],
            unavailable: vec![],
        }))
    } else {
        Err(ErrorResponse {
            error: ErrorDetail {
                code: "unauthorized",
                message: "Authentication required",
            },
        })
    }
}

fn content_type_for_path(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()).unwrap_or("") {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "application/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" | "map" => "application/json; charset=utf-8",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "ico" => "image/x-icon",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "txt" => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

async fn try_serve_static(frontend_dir: &Path, uri_path: &str) -> Option<Response> {
    let base = tokio::fs::canonicalize(frontend_dir).await.ok()?;
    let rel = uri_path.trim_start_matches('/');

    let mut clean_rel = PathBuf::new();
    for comp in Path::new(rel).components() {
        match comp {
            Component::Normal(part) => clean_rel.push(part),
            Component::CurDir => {}
            _ => return None,
        }
    }

    let candidate = if rel.is_empty() {
        base.join("index.html")
    } else {
        base.join(&clean_rel)
    };

    let file_path = match tokio::fs::canonicalize(&candidate).await {
        Ok(p) if p.starts_with(&base) && p.is_file() => p,
        _ => {
            let index = base.join("index.html");
            if index.is_file() {
                index
            } else {
                return None;
            }
        }
    };

    let bytes = tokio::fs::read(&file_path).await.ok()?;
    let mime = content_type_for_path(&file_path);

    Response::builder()
        .status(StatusCode::OK)
        .header(CONTENT_TYPE, mime)
        .body(Body::from(bytes))
        .ok()
}

async fn fallback_handler(State(state): State<AppState>, uri: Uri) -> Response {
    let path = uri.path();
    let is_api_path =
        path == "/v1" || path.starts_with("/v1/") || path == "/api" || path.starts_with("/api/");

    if !is_api_path {
        if let Some(frontend_dir) = &state.frontend_dir {
            if let Some(resp) = try_serve_static(frontend_dir, path).await {
                return resp;
            }
        }
    }

    ErrorResponse {
        error: ErrorDetail {
            code: "not_found",
            message: "Not found",
        },
    }
    .into_response()
}

fn app(state: AppState) -> Router {
    let auth_router = auth_routes(AuthState {
        store: state.identity_store.clone(),
    });

    Router::new()
        .route("/v1/health", get(health_handler))
        .route("/api/v1/health", get(health_handler))
        .route("/v1/capabilities", get(capabilities_handler))
        .route("/api/v1/capabilities", get(capabilities_handler))
        .merge(node_jobs::routes())
        .merge(websites::routes())
        .fallback(fallback_handler)
        .with_state(state)
        .merge(auth_router)
        .layer(CookieManagerLayer::new())
}

fn resolve_frontend_dir() -> Option<PathBuf> {
    if let Ok(dir) = env::var("FUSIONPANEL_FRONTEND_DIR") {
        let p = PathBuf::from(dir);
        if p.join("index.html").is_file() {
            return Some(p);
        }
    }
    let default = PathBuf::from("./frontend/dist");
    if default.join("index.html").is_file() {
        return Some(default);
    }
    None
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let admin_token = match env::var("FUSIONPANEL_ADMIN_TOKEN") {
        Ok(token) if !token.is_empty() => token,
        _ => {
            eprintln!("FUSIONPANEL_ADMIN_TOKEN is required and must not be empty");
            std::process::exit(1);
        }
    };

    let listen_addr =
        env::var("FUSIONPANEL_LISTEN").unwrap_or_else(|_| "127.0.0.1:8080".to_string());

    let db_path =
        env::var("FUSIONPANEL_DATABASE_URL").unwrap_or_else(|_| "./fusionpanel.db".to_string());

    // Ensure core control-store schema migrations are applied on the shared SQLite file
    let control_store = ControlStore::new(Path::new(&db_path))?;
    let jobs_store = fusionpanel_jobs::JobStore::new(Path::new(&db_path))?;
    let identity_store = SqliteIdentityStore::open(&db_path)?;
    let frontend_dir = resolve_frontend_dir();

    let state = AppState {
        admin_token: Arc::new(admin_token),
        identity_store: Arc::new(Mutex::new(identity_store)),
        control_store: Arc::new(Mutex::new(control_store)),
        jobs_store: Arc::new(Mutex::new(jobs_store)),
        frontend_dir,
    };

    let app = app(state);

    let listener = TcpListener::bind(&listen_addr).await?;
    println!("Listening on {} (db={})", listen_addr, db_path);

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    fn test_app() -> Router {
        let store = SqliteIdentityStore::in_memory().unwrap();
        app(AppState {
            admin_token: Arc::new("test-secret-token".to_string()),
            identity_store: Arc::new(Mutex::new(store)),
            control_store: Arc::new(Mutex::new(ControlStore::new_in_memory().unwrap())),
            jobs_store: Arc::new(Mutex::new(fusionpanel_jobs::JobStore::in_memory().unwrap())),
            frontend_dir: None,
        })
    }

    #[tokio::test]
    async fn test_health() {
        let app = test_app();

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/v1/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = response.into_body().collect().await.unwrap().to_bytes();
        let body_str = String::from_utf8(body.to_vec()).unwrap();
        assert_eq!(
            body_str,
            format!(
                r#"{{"status":"ok","service":"fusionpanel-api","version":"{}"}}"#,
                option_env!("FUSIONPANEL_BUILD_VERSION").unwrap_or("development")
            )
        );
    }

    #[tokio::test]
    async fn test_capabilities_no_auth() {
        let app = test_app();

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/v1/capabilities")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        let body = response.into_body().collect().await.unwrap().to_bytes();
        let body_str = String::from_utf8(body.to_vec()).unwrap();
        assert_eq!(
            body_str,
            r#"{"error":{"code":"unauthorized","message":"Authentication required"}}"#
        );
    }

    #[tokio::test]
    async fn test_capabilities_valid_auth() {
        let app = test_app();

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/v1/capabilities")
                    .header("Authorization", "Bearer test-secret-token")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = response.into_body().collect().await.unwrap().to_bytes();
        let body_str = String::from_utf8(body.to_vec()).unwrap();
        assert_eq!(
            body_str,
            r#"{"capabilities":["control-plane.health.read","control-plane.capabilities.read"]}"#
        );
    }

    #[tokio::test]
    async fn test_capabilities_invalid_auth() {
        let app = test_app();

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/v1/capabilities")
                    .header("Authorization", "Bearer wrong-token")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_not_found() {
        let app = test_app();

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/v1/unknown")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);

        let body = response.into_body().collect().await.unwrap().to_bytes();
        let body_str = String::from_utf8(body.to_vec()).unwrap();
        assert_eq!(
            body_str,
            r#"{"error":{"code":"not_found","message":"Not found"}}"#
        );
    }

    #[tokio::test]
    async fn test_cookie_session_and_persistence_flow() {
        let temp_dir = tempfile::tempdir().unwrap();
        let db_path = temp_dir.path().join("fusionpanel.db");
        let db_str = db_path.to_str().unwrap();

        let _control = ControlStore::new(&db_path).unwrap();
        {
            let store = SqliteIdentityStore::open(db_str).unwrap();
            let password_hash =
                identity::PasswordManager::hash_password("secret-pass-123").unwrap();
            let user = identity::User::new(
                "admin".to_string(),
                "admin@example.com".to_string(),
                password_hash,
            )
            .unwrap();
            store.create_user(&user).unwrap();
        }

        let make_app = || {
            let store = SqliteIdentityStore::open(db_str).unwrap();
            app(AppState {
                admin_token: Arc::new("test-secret-token".to_string()),
                identity_store: Arc::new(Mutex::new(store)),
                control_store: Arc::new(Mutex::new(ControlStore::new(&db_path).unwrap())),
                jobs_store: Arc::new(Mutex::new(
                    fusionpanel_jobs::JobStore::new(&db_path).unwrap(),
                )),
                frontend_dir: None,
            })
        };

        let app1 = make_app();
        let login_body = serde_json::json!({
            "username": "admin",
            "password": "secret-pass-123"
        });
        let login_resp = app1
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/auth/login")
                    .header("Content-Type", "application/json")
                    .body(Body::from(login_body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(login_resp.status(), StatusCode::OK);

        let set_cookie = login_resp
            .headers()
            .get("set-cookie")
            .unwrap()
            .to_str()
            .unwrap();
        let cookie_pair = set_cookie.split(';').next().unwrap().to_string();

        // Check /v1/auth/me and /v1/capabilities with session cookie
        let me_resp = app1
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/v1/auth/me")
                    .header("Cookie", &cookie_pair)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(me_resp.status(), StatusCode::OK);

        let cap_resp = app1
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/v1/capabilities")
                    .header("Cookie", &cookie_pair)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(cap_resp.status(), StatusCode::OK);

        let nodes_resp = app1
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/v1/nodes")
                    .header("Cookie", &cookie_pair)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(nodes_resp.status(), StatusCode::OK);

        let jobs_resp = app1
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/v1/jobs")
                    .header("Cookie", &cookie_pair)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(jobs_resp.status(), StatusCode::OK);

        // Simulate API restart: reopen store from same SQLite DB
        let app2 = make_app();
        let cap_after_restart = app2
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/v1/capabilities")
                    .header("Cookie", &cookie_pair)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(cap_after_restart.status(), StatusCode::OK);

        // Logout revokes session in SQLite
        let logout_resp = app2
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/auth/logout")
                    .header("Cookie", &cookie_pair)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(logout_resp.status(), StatusCode::OK);

        let cap_after_logout = app2
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/v1/capabilities")
                    .header("Cookie", &cookie_pair)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(cap_after_logout.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_static_frontend_and_spa_fallback() {
        let temp_dir = tempfile::tempdir().unwrap();
        let dist = temp_dir.path().join("dist");
        std::fs::create_dir_all(dist.join("assets")).unwrap();
        std::fs::write(
            dist.join("index.html"),
            "<html><body>FusionPanel</body></html>",
        )
        .unwrap();
        std::fs::write(dist.join("assets/app.js"), "console.log('ok');").unwrap();

        let store = SqliteIdentityStore::in_memory().unwrap();
        let router = app(AppState {
            admin_token: Arc::new("test-secret-token".to_string()),
            identity_store: Arc::new(Mutex::new(store)),
            control_store: Arc::new(Mutex::new(ControlStore::new_in_memory().unwrap())),
            jobs_store: Arc::new(Mutex::new(fusionpanel_jobs::JobStore::in_memory().unwrap())),
            frontend_dir: Some(dist),
        });

        let root_resp = router
            .clone()
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(root_resp.status(), StatusCode::OK);
        assert_eq!(
            root_resp.headers().get(CONTENT_TYPE).unwrap(),
            "text/html; charset=utf-8"
        );

        let asset_resp = router
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/assets/app.js")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(asset_resp.status(), StatusCode::OK);
        assert_eq!(
            asset_resp.headers().get(CONTENT_TYPE).unwrap(),
            "application/javascript; charset=utf-8"
        );

        let spa_resp = router
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/dashboard")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(spa_resp.status(), StatusCode::OK);
        let spa_body = spa_resp.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(
            String::from_utf8(spa_body.to_vec()).unwrap(),
            "<html><body>FusionPanel</body></html>"
        );

        let api_404 = router
            .oneshot(
                Request::builder()
                    .uri("/v1/nonexistent")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(api_404.status(), StatusCode::NOT_FOUND);
    }
}
