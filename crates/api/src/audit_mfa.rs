#![forbid(unsafe_code)]

use crate::node_jobs;
use crate::websites::{require_website_permission, website_authorization_context};
use crate::{AppState, ErrorDetail, ErrorResponse};
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use chrono::Utc;
use identity::Permission;
use serde::Deserialize;
use serde_json::{json, Value};
use subtle::ConstantTimeEq;
use uuid::Uuid;

#[derive(Deserialize)]
struct AuditQuery {
    limit: Option<usize>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct VerifyMfaRequest {
    code: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateSecurityRuleRequest {
    rule_type: String,
    target: String,
    action: String,
    #[serde(default)]
    description: String,
}

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route("/v1/audit", get(list_audit_logs))
        .route("/v1/admin/audit", get(list_audit_logs))
        .route("/v1/auth/mfa", get(get_mfa_status).delete(disable_mfa))
        .route("/v1/auth/mfa/enroll", post(enroll_mfa))
        .route("/v1/auth/mfa/verify", post(verify_mfa))
        .route(
            "/v1/security/rules",
            get(list_security_rules).post(create_security_rule),
        )
        .route("/v1/security/rules/{id}", delete(delete_security_rule))
}

fn bad_request_msg(message: &'static str) -> ErrorResponse {
    ErrorResponse {
        error: ErrorDetail {
            code: "bad_request",
            message,
        },
    }
}

async fn list_audit_logs(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<AuditQuery>,
) -> Result<Json<Value>, ErrorResponse> {
    let context = website_authorization_context(&state, &headers).await?;
    require_website_permission(&context, Permission::TenantManageUsers, None)?;

    let limit = query.limit.unwrap_or(100);
    let logs = state
        .control_store
        .lock()
        .await
        .list_audit_logs(context.tenant_id.to_uuid(), limit)
        .map_err(|_| node_jobs::internal_error())?;

    Ok(Json(json!({ "audit_logs": logs })))
}

async fn get_mfa_status(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, ErrorResponse> {
    let context = website_authorization_context(&state, &headers).await?;
    let mfa_user_id = context
        .user_id
        .map(|u| u.to_uuid())
        .unwrap_or_else(|| context.tenant_id.to_uuid());

    let record = state
        .control_store
        .lock()
        .await
        .get_user_mfa(mfa_user_id)
        .map_err(|_| node_jobs::internal_error())?;

    Ok(Json(json!({
        "enabled": record.map(|r| r.enabled).unwrap_or(false)
    })))
}

async fn enroll_mfa(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, ErrorResponse> {
    let context = website_authorization_context(&state, &headers).await?;
    let mfa_user_id = context
        .user_id
        .map(|u| u.to_uuid())
        .unwrap_or_else(|| context.tenant_id.to_uuid());

    let mut raw = [0u8; 20];
    let u1 = Uuid::new_v4();
    let u2 = Uuid::new_v4();
    raw[..16].copy_from_slice(u1.as_bytes());
    raw[16..20].copy_from_slice(&u2.as_bytes()[..4]);
    let secret = base32_encode(&raw);

    let now = Utc::now().to_rfc3339();
    let mut store = state.control_store.lock().await;
    store
        .upsert_user_mfa_secret(mfa_user_id, &secret, &now)
        .map_err(|_| node_jobs::internal_error())?;

    let actor = context
        .user_id
        .map(|u| u.to_string())
        .unwrap_or_else(|| "admin-token".to_string());
    let _ = store.record_audit_log(
        context.tenant_id.to_uuid(),
        &actor,
        "mfa.enroll",
        "user_mfa",
        &mfa_user_id.to_string(),
        "success",
        None,
        &now,
    );

    let otpauth_url = format!(
        "otpauth://totp/FusionPanel:{mfa_user_id}?secret={secret}&issuer=FusionPanel&algorithm=SHA1&digits=6&period=30"
    );

    Ok(Json(json!({
        "secret": secret,
        "otpauth_url": otpauth_url,
        "enabled": false
    })))
}

async fn verify_mfa(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<VerifyMfaRequest>,
) -> Result<Json<Value>, ErrorResponse> {
    let context = website_authorization_context(&state, &headers).await?;
    let mfa_user_id = context
        .user_id
        .map(|u| u.to_uuid())
        .unwrap_or_else(|| context.tenant_id.to_uuid());

    let code = body.code.trim();
    if code.len() != 6 || !code.bytes().all(|b| b.is_ascii_digit()) {
        return Err(bad_request_msg("TOTP code must be 6 digits"));
    }

    let mut store = state.control_store.lock().await;
    let record = store
        .get_user_mfa(mfa_user_id)
        .map_err(|_| node_jobs::internal_error())?
        .ok_or_else(|| bad_request_msg("MFA enrollment not started"))?;

    let secret_bytes = base32_decode(&record.totp_secret).ok_or_else(node_jobs::internal_error)?;
    let now_secs = Utc::now().timestamp() as u64;
    if !verify_totp_code(&secret_bytes, now_secs, code) {
        return Err(ErrorResponse {
            error: ErrorDetail {
                code: "bad_request",
                message: "Invalid TOTP verification code",
            },
        });
    }

    let now = Utc::now().to_rfc3339();
    store
        .enable_user_mfa(mfa_user_id, &now)
        .map_err(|_| node_jobs::internal_error())?;

    let actor = context
        .user_id
        .map(|u| u.to_string())
        .unwrap_or_else(|| "admin-token".to_string());
    let _ = store.record_audit_log(
        context.tenant_id.to_uuid(),
        &actor,
        "mfa.enable",
        "user_mfa",
        &mfa_user_id.to_string(),
        "success",
        None,
        &now,
    );

    Ok(Json(json!({ "enabled": true })))
}

async fn disable_mfa(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<StatusCode, ErrorResponse> {
    let context = website_authorization_context(&state, &headers).await?;
    let mfa_user_id = context
        .user_id
        .map(|u| u.to_uuid())
        .unwrap_or_else(|| context.tenant_id.to_uuid());

    let mut store = state.control_store.lock().await;
    store
        .disable_user_mfa(mfa_user_id)
        .map_err(|_| node_jobs::internal_error())?;

    let now = Utc::now().to_rfc3339();
    let actor = context
        .user_id
        .map(|u| u.to_string())
        .unwrap_or_else(|| "admin-token".to_string());
    let _ = store.record_audit_log(
        context.tenant_id.to_uuid(),
        &actor,
        "mfa.disable",
        "user_mfa",
        &mfa_user_id.to_string(),
        "success",
        None,
        &now,
    );

    Ok(StatusCode::NO_CONTENT)
}

async fn list_security_rules(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, ErrorResponse> {
    let context = website_authorization_context(&state, &headers).await?;
    require_website_permission(&context, Permission::TenantRead, None)?;

    let rules = state
        .control_store
        .lock()
        .await
        .list_security_rules(context.tenant_id.to_uuid())
        .map_err(|_| node_jobs::internal_error())?;

    Ok(Json(json!({ "rules": rules })))
}

async fn create_security_rule(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<CreateSecurityRuleRequest>,
) -> Result<(StatusCode, Json<Value>), ErrorResponse> {
    let context = website_authorization_context(&state, &headers).await?;
    require_website_permission(&context, Permission::TenantManageUsers, None)?;

    let rule_type = body.rule_type.trim();
    let target = body.target.trim();
    let action = body.action.trim();
    if rule_type.is_empty() || target.is_empty() || action.is_empty() {
        return Err(bad_request_msg(
            "rule_type, target, and action are required",
        ));
    }
    if !matches!(action, "allow" | "deny") {
        return Err(bad_request_msg("action must be allow or deny"));
    }

    let now = Utc::now().to_rfc3339();
    let mut store = state.control_store.lock().await;
    let rule = store
        .create_security_rule(
            context.tenant_id.to_uuid(),
            rule_type,
            target,
            action,
            body.description.trim(),
            &now,
        )
        .map_err(|_| node_jobs::internal_error())?;

    let actor = context
        .user_id
        .map(|u| u.to_string())
        .unwrap_or_else(|| "admin-token".to_string());
    let _ = store.record_audit_log(
        context.tenant_id.to_uuid(),
        &actor,
        "security_rule.create",
        "security_rule",
        &rule.id.to_string(),
        "success",
        Some(&format!("{rule_type} {action} {target}")),
        &now,
    );

    Ok((StatusCode::CREATED, Json(json!({ "rule": rule }))))
}

async fn delete_security_rule(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id_str): Path<String>,
) -> Result<StatusCode, ErrorResponse> {
    let context = website_authorization_context(&state, &headers).await?;
    require_website_permission(&context, Permission::TenantManageUsers, None)?;

    let rule_id = Uuid::parse_str(&id_str).map_err(|_| node_jobs::bad_request())?;
    let mut store = state.control_store.lock().await;
    let deleted = store
        .delete_security_rule(context.tenant_id.to_uuid(), rule_id)
        .map_err(|_| node_jobs::internal_error())?;
    if !deleted {
        return Err(ErrorResponse {
            error: ErrorDetail {
                code: "not_found",
                message: "Security rule not found",
            },
        });
    }

    let now = Utc::now().to_rfc3339();
    let actor = context
        .user_id
        .map(|u| u.to_string())
        .unwrap_or_else(|| "admin-token".to_string());
    let _ = store.record_audit_log(
        context.tenant_id.to_uuid(),
        &actor,
        "security_rule.delete",
        "security_rule",
        &rule_id.to_string(),
        "success",
        None,
        &now,
    );

    Ok(StatusCode::NO_CONTENT)
}

const BASE32_ALPHABET: &[u8; 32] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";

fn base32_encode(data: &[u8]) -> String {
    let mut out = String::new();
    let mut buffer: u32 = 0;
    let mut bits_left: u8 = 0;
    for &byte in data {
        buffer = (buffer << 8) | u32::from(byte);
        bits_left += 8;
        while bits_left >= 5 {
            bits_left -= 5;
            let idx = ((buffer >> bits_left) & 0x1f) as usize;
            out.push(BASE32_ALPHABET[idx] as char);
        }
    }
    if bits_left > 0 {
        let idx = ((buffer << (5 - bits_left)) & 0x1f) as usize;
        out.push(BASE32_ALPHABET[idx] as char);
    }
    out
}

fn base32_decode(encoded: &str) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    let mut buffer: u32 = 0;
    let mut bits_left: u8 = 0;
    for ch in encoded.bytes() {
        if ch == b'=' || ch.is_ascii_whitespace() {
            continue;
        }
        let upper = ch.to_ascii_uppercase();
        let val = BASE32_ALPHABET.iter().position(|&b| b == upper)? as u32;
        buffer = (buffer << 5) | val;
        bits_left += 5;
        if bits_left >= 8 {
            bits_left -= 8;
            out.push(((buffer >> bits_left) & 0xff) as u8);
        }
    }
    Some(out)
}

fn sha1_digest(data: &[u8]) -> [u8; 20] {
    let mut h: [u32; 5] = [0x67452301, 0xEFCDAB89, 0x98BADCFE, 0x10325476, 0xC3D2E1F0];
    let mut msg = data.to_vec();
    let bit_len = (data.len() as u64) * 8;
    msg.push(0x80);
    while (msg.len() % 64) != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bit_len.to_be_bytes());

    for chunk in msg.as_chunks::<64>().0 {
        let mut w = [0u32; 80];
        for (i, word) in w.iter_mut().take(16).enumerate() {
            let j = i * 4;
            *word = u32::from_be_bytes([chunk[j], chunk[j + 1], chunk[j + 2], chunk[j + 3]]);
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }
        let [mut a, mut b, mut c, mut d, mut e] = h;
        for (i, &wi) in w.iter().enumerate() {
            let (f, k) = match i {
                0..=19 => ((b & c) | ((!b) & d), 0x5A827999u32),
                20..=39 => (b ^ c ^ d, 0x6ED9EBA1u32),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1BBCDCu32),
                _ => (b ^ c ^ d, 0xCA62C1D6u32),
            };
            let temp = a
                .rotate_left(5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(wi);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = temp;
        }
        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
    }

    let mut out = [0u8; 20];
    for (i, word) in h.iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&word.to_be_bytes());
    }
    out
}

fn hmac_sha1(key: &[u8], msg: &[u8]) -> [u8; 20] {
    let mut k_block = [0u8; 64];
    if key.len() > 64 {
        let hashed = sha1_digest(key);
        k_block[..20].copy_from_slice(&hashed);
    } else {
        k_block[..key.len()].copy_from_slice(key);
    }
    let mut o_pad = [0x5cu8; 64];
    let mut i_pad = [0x36u8; 64];
    for i in 0..64 {
        o_pad[i] ^= k_block[i];
        i_pad[i] ^= k_block[i];
    }
    let mut inner = Vec::with_capacity(64 + msg.len());
    inner.extend_from_slice(&i_pad);
    inner.extend_from_slice(msg);
    let inner_hash = sha1_digest(&inner);

    let mut outer = Vec::with_capacity(64 + 20);
    outer.extend_from_slice(&o_pad);
    outer.extend_from_slice(&inner_hash);
    sha1_digest(&outer)
}

pub(crate) fn compute_totp_code(secret: &[u8], unix_secs: u64) -> String {
    let counter = unix_secs / 30;
    let hmac = hmac_sha1(secret, &counter.to_be_bytes());
    let offset = (hmac[19] & 0x0f) as usize;
    let bin_code = ((u32::from(hmac[offset]) & 0x7f) << 24)
        | (u32::from(hmac[offset + 1]) << 16)
        | (u32::from(hmac[offset + 2]) << 8)
        | u32::from(hmac[offset + 3]);
    format!("{:06}", bin_code % 1_000_000)
}

fn verify_totp_code(secret: &[u8], unix_secs: u64, candidate: &str) -> bool {
    let step = unix_secs / 30;
    for delta in [-1i64, 0, 1] {
        let target_step = if delta < 0 {
            step.saturating_sub(1)
        } else {
            step.saturating_add(delta as u64)
        };
        let expected = compute_totp_code(secret, target_step * 30);
        if expected.as_bytes().ct_eq(candidate.as_bytes()).into() {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{header::AUTHORIZATION, Request};
    use http_body_util::BodyExt;
    use tempfile::TempDir;
    use tower::ServiceExt;

    const ADMIN_TOKEN: &str = "audit-mfa-test-token";
    const ADMIN_TENANT_ID: &str = "00000000-0000-0000-0000-000000000001";

    fn test_app() -> (Router, TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("audit_mfa.sqlite");
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
                json!({ "name": "Admin Tenant" }),
            )
            .unwrap();
        let state = AppState {
            admin_token: std::sync::Arc::new(ADMIN_TOKEN.to_string()),
            identity_store,
            control_store: std::sync::Arc::new(tokio::sync::Mutex::new(control_store)),
            jobs_store: std::sync::Arc::new(tokio::sync::Mutex::new(
                fusionpanel_jobs::JobStore::new(&db_path).unwrap(),
            )),
            frontend_dir: None,
        };
        (crate::app(state), dir)
    }

    async fn json_body(res: axum::response::Response) -> Value {
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        serde_json::from_slice(&bytes).unwrap()
    }

    #[test]
    fn rfc6238_totp_test_vector() {
        // RFC 6238 SHA-1 secret "12345678901234567890" at T = 59s -> 94287082 -> 287082
        let secret = b"12345678901234567890";
        assert_eq!(compute_totp_code(secret, 59), "287082");
    }

    #[tokio::test]
    async fn mfa_enroll_verify_audit_and_security_rules_flow() {
        let (app, _dir) = test_app();

        // 1. Enroll MFA
        let enroll_res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/auth/mfa/enroll")
                    .header(AUTHORIZATION, format!("Bearer {ADMIN_TOKEN}"))
                    .header("x-fusionpanel-tenant-id", ADMIN_TENANT_ID)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(enroll_res.status(), StatusCode::OK);
        let enroll_json = json_body(enroll_res).await;
        let secret_b32 = enroll_json["secret"].as_str().unwrap();
        let secret_bytes = base32_decode(secret_b32).unwrap();
        let valid_code = compute_totp_code(&secret_bytes, Utc::now().timestamp() as u64);

        // 2. Reject invalid code
        let bad_verify = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/auth/mfa/verify")
                    .header(AUTHORIZATION, format!("Bearer {ADMIN_TOKEN}"))
                    .header("x-fusionpanel-tenant-id", ADMIN_TENANT_ID)
                    .header("content-type", "application/json")
                    .body(Body::from(json!({ "code": "000000" }).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        if valid_code != "000000" {
            assert_eq!(bad_verify.status(), StatusCode::BAD_REQUEST);
        }

        // 3. Accept valid code
        let ok_verify = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/auth/mfa/verify")
                    .header(AUTHORIZATION, format!("Bearer {ADMIN_TOKEN}"))
                    .header("x-fusionpanel-tenant-id", ADMIN_TENANT_ID)
                    .header("content-type", "application/json")
                    .body(Body::from(json!({ "code": valid_code }).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(ok_verify.status(), StatusCode::OK);
        assert_eq!(json_body(ok_verify).await["enabled"], true);

        // 4. Create security rule
        let rule_res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/security/rules")
                    .header(AUTHORIZATION, format!("Bearer {ADMIN_TOKEN}"))
                    .header("x-fusionpanel-tenant-id", ADMIN_TENANT_ID)
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "rule_type": "firewall",
                            "target": "203.0.113.5/32",
                            "action": "deny",
                            "description": "e2e test rule"
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(rule_res.status(), StatusCode::CREATED);

        // 5. Query audit log
        let audit_res = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/v1/audit")
                    .header(AUTHORIZATION, format!("Bearer {ADMIN_TOKEN}"))
                    .header("x-fusionpanel-tenant-id", ADMIN_TENANT_ID)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(audit_res.status(), StatusCode::OK);
        let audit_json = json_body(audit_res).await;
        let actions: Vec<&str> = audit_json["audit_logs"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|e| e["action"].as_str())
            .collect();
        assert!(actions.contains(&"mfa.enroll"));
        assert!(actions.contains(&"mfa.enable"));
        assert!(actions.contains(&"security_rule.create"));
    }
}
