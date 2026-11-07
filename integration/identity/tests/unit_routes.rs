#![forbid(unsafe_code)]

#[path = "../src/routes.rs"]
mod routes;

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use identity::password::PasswordManager;
use identity::{types::User, SqliteIdentityStore};
use routes::{auth_routes, AuthState};
use std::sync::Arc;
use tokio::sync::Mutex;
use tower::ServiceExt;
use tower_cookies::CookieManagerLayer;

fn setup_store() -> Arc<Mutex<SqliteIdentityStore>> {
    let store = SqliteIdentityStore::in_memory().unwrap();
    // Insert test user
    let hash = PasswordManager::hash_password("password123").unwrap();
    let user = User::new("testuser".to_string(), "test@test.com".to_string(), hash).unwrap();
    store.create_user(&user).unwrap();

    Arc::new(Mutex::new(store))
}

fn setup_app(store: Arc<Mutex<SqliteIdentityStore>>) -> axum::Router {
    let state = AuthState { store };
    auth_routes(state).layer(CookieManagerLayer::new())
}

#[tokio::test]
async fn test_login_success() {
    let store = setup_store();
    let app = setup_app(store);

    let body = serde_json::json!({
        "username": "testuser",
        "password": "password123"
    });

    let request = Request::builder()
        .method("POST")
        .uri("/api/v1/auth/login")
        .header("Content-Type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // Verify cookie
    let set_cookie = response
        .headers()
        .get("set-cookie")
        .unwrap()
        .to_str()
        .unwrap();
    assert!(set_cookie.contains("SESSION_ID="));
    assert!(set_cookie.contains("HttpOnly"));
    assert!(set_cookie.contains("Secure"));
    assert!(set_cookie.contains("SameSite=Strict"));
}

#[tokio::test]
async fn test_login_bad_credentials() {
    let store = setup_store();
    let app = setup_app(store);

    let body = serde_json::json!({
        "username": "testuser",
        "password": "wrongpassword"
    });

    let request = Request::builder()
        .method("POST")
        .uri("/api/v1/auth/login")
        .header("Content-Type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_session_invalidation() {
    let store = setup_store();
    let app = setup_app(store.clone());

    // Login first
    let body = serde_json::json!({
        "username": "testuser",
        "password": "password123"
    });

    let request = Request::builder()
        .method("POST")
        .uri("/api/v1/auth/login")
        .header("Content-Type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let set_cookie = response
        .headers()
        .get("set-cookie")
        .unwrap()
        .to_str()
        .unwrap();
    let cookie_val = set_cookie.split(";").next().unwrap(); // e.g., SESSION_ID=...

    // Refresh should work
    let request_refresh = Request::builder()
        .method("POST")
        .uri("/api/v1/auth/refresh")
        .header("Cookie", cookie_val)
        .body(Body::empty())
        .unwrap();
    let response_refresh = app.clone().oneshot(request_refresh).await.unwrap();
    assert_eq!(response_refresh.status(), StatusCode::OK);

    // Logout
    let request_logout = Request::builder()
        .method("POST")
        .uri("/api/v1/auth/logout")
        .header("Cookie", cookie_val)
        .body(Body::empty())
        .unwrap();
    let response_logout = app.clone().oneshot(request_logout).await.unwrap();
    assert_eq!(response_logout.status(), StatusCode::OK);

    // Verify cookie removal format
    let set_cookie_logout = response_logout
        .headers()
        .get("set-cookie")
        .unwrap()
        .to_str()
        .unwrap();
    assert!(set_cookie_logout.contains("Max-Age=0") || set_cookie_logout.contains("Expires="));

    // Refresh should now fail because session was revoked in store
    let request_refresh2 = Request::builder()
        .method("POST")
        .uri("/api/v1/auth/refresh")
        .header("Cookie", cookie_val) // sending old cookie manually
        .body(Body::empty())
        .unwrap();
    let response_refresh2 = app.clone().oneshot(request_refresh2).await.unwrap();
    assert_eq!(response_refresh2.status(), StatusCode::UNAUTHORIZED);
}
