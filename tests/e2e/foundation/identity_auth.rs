#![forbid(unsafe_code)]
use identity::{
    AccessControl, IdentityError, PasswordManager, Permission, Role, RoleBinding, Scope, Session,
    SqliteIdentityStore, TenantId, User,
};
use reqwest::{Client, StatusCode};
use serde_json::Value;
use std::process::{Child, Command};
use std::time::Duration;
use tokio::time::sleep;

struct ApiServer {
    child: Child,
}

impl ApiServer {
    fn start(token: &str, port: u16) -> Self {
        let listen_addr = format!("127.0.0.1:{}", port);

        // Use cargo from the root workspace
        let mut cmd = Command::new("cargo");
        cmd.args([
            "run",
            "--manifest-path",
            "../../../Cargo.toml",
            "--bin",
            "fusionpanel-api",
        ])
        .env("FUSIONPANEL_LISTEN", &listen_addr)
        .env("FUSIONPANEL_ADMIN_TOKEN", token);

        let child = cmd.spawn().expect("Failed to start API server");

        Self { child }
    }
}

impl Drop for ApiServer {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[tokio::test]
async fn test_system_boot_and_identity_lifecycle() {
    // 1. System Boot & Config Loading
    let admin_token = "e2e-super-secret-token";
    let port = 8085;
    let _server = ApiServer::start(admin_token, port);

    // Wait for server to boot (cargo run can take a while)
    let mut ok = false;
    let client = Client::new();
    let base_url = format!("http://127.0.0.1:{}", port);

    for _ in 0..30 {
        sleep(Duration::from_secs(1)).await;
        if let Ok(resp) = client.get(format!("{}/v1/health", base_url)).send().await {
            if resp.status() == StatusCode::OK {
                ok = true;
                break;
            }
        }
    }

    assert!(ok, "API Server failed to start");

    // Health check should pass without auth
    let health_resp = client
        .get(format!("{}/v1/health", base_url))
        .send()
        .await
        .expect("Failed to call health endpoint");
    assert_eq!(health_resp.status(), StatusCode::OK);

    let health_json: Value = health_resp.json().await.unwrap();
    assert_eq!(health_json["status"], "ok");
    assert_eq!(health_json["service"], "fusionpanel-api");

    // Capabilities check should fail without auth
    let cap_fail = client
        .get(format!("{}/v1/capabilities", base_url))
        .send()
        .await
        .expect("Failed to call capabilities");
    assert_eq!(cap_fail.status(), StatusCode::UNAUTHORIZED);

    // Capabilities should pass with auth
    let cap_pass = client
        .get(format!("{}/v1/capabilities", base_url))
        .header("Authorization", format!("Bearer {}", admin_token))
        .send()
        .await
        .expect("Failed to call capabilities");
    assert_eq!(cap_pass.status(), StatusCode::OK);

    let cap_json: Value = cap_pass.json().await.unwrap();
    assert!(cap_json["capabilities"].as_array().unwrap().len() > 0);

    // 2. Identity Lifecycle
    std::fs::create_dir_all("target/tmp").unwrap();
    let tmp_file = tempfile::Builder::new()
        .prefix("identity-e2e-")
        .tempfile_in("target/tmp")
        .unwrap();
    let db_path = tmp_file.path().to_path_buf();

    let store = SqliteIdentityStore::open(&db_path).unwrap();

    // User Creation
    let raw_password = "SecurePassword123!";
    let hashed = PasswordManager::hash_password(raw_password).unwrap();
    let user = User::new(
        "e2e_user".to_string(),
        "e2e@example.com".to_string(),
        hashed.clone(),
    )
    .unwrap();

    store.create_user(&user).unwrap();

    let fetched_user = store.get_user(user.id()).unwrap().unwrap();
    assert_eq!(fetched_user.username(), "e2e_user");

    // Password Verification
    assert!(PasswordManager::verify_password(raw_password, fetched_user.password_hash()).is_ok());
    assert!(PasswordManager::verify_password("wrong", fetched_user.password_hash()).is_err());

    // Token / Session Issuance
    let tenant_id = TenantId::new_v4();
    let mut session = Session::new(user.id(), tenant_id, 3600, 600).unwrap();
    store.save_session(&session).unwrap();

    let fetched_session = store.get_session(session.id()).unwrap().unwrap();
    assert_eq!(fetched_session.user_id(), user.id());

    // Session Timeout Verification
    let old_time = chrono::Utc::now() + chrono::Duration::seconds(700);
    let timeout_res = session.validate_and_touch(old_time);
    assert!(matches!(timeout_res, Err(IdentityError::SessionExpired)));

    // Permission Enforcement
    let role = Role::tenant_admin();
    let scope = Scope::Tenant { tenant_id };
    let binding = RoleBinding {
        user_id: user.id(),
        role,
        scope: scope.clone(),
    };

    store.save_role_binding(&binding).unwrap();
    let bindings = store.get_user_role_bindings(user.id()).unwrap();

    assert!(AccessControl::evaluate_principal(
        user.id(),
        &bindings,
        Permission::SiteCreate,
        &scope
    )
    .is_ok());

    assert!(matches!(
        AccessControl::evaluate_principal(user.id(), &bindings, Permission::SystemAdmin, &scope),
        Err(IdentityError::Unauthorized { .. })
    ));

    // Multi-tenant isolation verification
    let tenant_b_id = TenantId::new_v4();
    let scope_b = Scope::Tenant { tenant_id: tenant_b_id };

    // User from Tenant A should not have access to Tenant B
    assert!(matches!(
        AccessControl::evaluate_principal(user.id(), &bindings, Permission::SiteCreate, &scope_b),
        Err(IdentityError::Unauthorized { .. })
    ));
    assert!(matches!(
        AccessControl::evaluate_principal(user.id(), &bindings, Permission::TenantRead, &scope_b),
        Err(IdentityError::Unauthorized { .. })
    ));

    // Token revocation and brute force lockout verification
    session.revoke("User compromised");
    assert!(session.is_revoked());
    let token_revocation_res = session.validate_and_touch(chrono::Utc::now());
    assert!(matches!(token_revocation_res, Err(IdentityError::SessionRevoked { .. })));

}

#[tokio::test]
async fn test_brute_force_lockout() {
    // Because full brute force lockout is not implemented in core yet, panic with BLOCKED
    panic!("RESULT=BLOCKED: Missing core implementation for Brute force lockout protection");
}

#[tokio::test]
async fn test_identity_flows_blocked() {
    let port = 8087;
    let _server = ApiServer::start("blocked-test-token", port);

    let base_url = format!("http://127.0.0.1:{}", port);
    let mut ok = false;
    let client = Client::new();
    for _ in 0..30 {
        sleep(Duration::from_secs(1)).await;
        if let Ok(resp) = client.get(format!("{}/v1/health", base_url)).send().await {
            if resp.status() == StatusCode::OK {
                ok = true;
                break;
            }
        }
    }
    assert!(ok, "API Server failed to start");

    // Missing required capabilities via CLI should exit nonzero
    let login_cli = Command::new("cargo")
        .args([
            "run",
            "--manifest-path",
            "../../../Cargo.toml",
            "--bin",
            "fusionpanel-cli",
            "--",
            "--server",
            &base_url,
            "login",
        ])
        .output()
        .expect("Failed to run CLI login");
    assert!(!login_cli.status.success(), "BLOCKED capabilities must return nonzero");

    let user_cli = Command::new("cargo")
        .args([
            "run",
            "--manifest-path",
            "../../../Cargo.toml",
            "--bin",
            "fusionpanel-cli",
            "--",
            "--server",
            &base_url,
            "users",
            "create",
        ])
        .output()
        .expect("Failed to run CLI user create");
    assert!(!user_cli.status.success(), "BLOCKED capabilities must return nonzero");
    
    // HTTP capabilities should be BLOCKED over HTTP (404 Not Found)
    
    let user_creation_res = client
        .post(format!("{}/v1/users", base_url))
        .header("Authorization", "Bearer blocked-test-token")
        .json(&serde_json::json!({
            "username": "e2e_user",
            "email": "e2e@example.com",
            "password": "SecurePassword123!"
        }))
        .send()
        .await
        .expect("Request failed");
    assert_eq!(user_creation_res.status(), reqwest::StatusCode::NOT_FOUND);

    let login_res = client
        .post(format!("{}/v1/login", base_url))
        .json(&serde_json::json!({
            "username": "e2e_user",
            "password": "SecurePassword123!"
        }))
        .send()
        .await
        .expect("Request failed");
    assert_eq!(login_res.status(), reqwest::StatusCode::NOT_FOUND);
    
    let get_tenant_res = client
        .get(format!("{}/v1/tenants/current", base_url))
        .header("Authorization", "Bearer blocked-test-token")
        .send()
        .await
        .expect("Request failed");
    assert_eq!(get_tenant_res.status(), reqwest::StatusCode::NOT_FOUND);
    
    let revoke_session_res = client
        .delete(format!("{}/v1/sessions/current", base_url))
        .header("Authorization", "Bearer blocked-test-token")
        .send()
        .await
        .expect("Request failed");
    assert_eq!(revoke_session_res.status(), reqwest::StatusCode::NOT_FOUND);

    // If API endpoints are not implemented, fail closed with BLOCKED panic
    // to strictly enforce zero-mock policies for E2E missing prerequisites.
    if user_creation_res.status() == reqwest::StatusCode::NOT_FOUND {
        panic!("RESULT=BLOCKED: Missing core implementation for User Registration and API Auth");
    }
}
