#![forbid(unsafe_code)]

use axum::{
    extract::{FromRequestParts, Path},
    http::{request::Parts, StatusCode},
    response::{IntoResponse, Response},
};
use identity::{
    rbac::{AccessControl, Permission, RoleBinding, RoleType, Scope},
    types::{SiteId, TenantId, UserId},
};
use std::collections::HashMap;
use std::future::Future;

#[derive(Debug)]
pub enum ExtractorError {
    MissingIdentity,
    Unauthorized(String),
}

impl IntoResponse for ExtractorError {
    fn into_response(self) -> Response {
        let (status, err_msg) = match self {
            Self::MissingIdentity => (
                StatusCode::UNAUTHORIZED,
                "Missing or invalid authentication context".to_string(),
            ),
            Self::Unauthorized(reason) => (StatusCode::FORBIDDEN, reason),
        };
        (status, err_msg).into_response()
    }
}

#[derive(Debug, Clone)]
pub struct AuthenticatedUser {
    pub user_id: UserId,
    pub bindings: Vec<RoleBinding>,
}

impl<S> FromRequestParts<S> for AuthenticatedUser
where
    S: Send + Sync,
{
    type Rejection = ExtractorError;

    fn from_request_parts(
        parts: &mut Parts,
        _state: &S,
    ) -> impl Future<Output = Result<Self, Self::Rejection>> + Send {
        let res = parts
            .extensions
            .get::<AuthenticatedUser>()
            .cloned()
            .ok_or(ExtractorError::MissingIdentity);
        std::future::ready(res)
    }
}

/// Helper function to asynchronously extract scope from Path or Extensions
async fn extract_scope_from_parts<S: Send + Sync>(parts: &mut Parts, state: &S) -> Scope {
    if let Some(scope) = parts.extensions.get::<Scope>() {
        return scope.clone();
    }

    if let Ok(path_params) = Path::<HashMap<String, String>>::from_request_parts(parts, state).await
    {
        let tenant_id_str = path_params.0.get("tenant_id");
        let site_id_str = path_params.0.get("site_id");

        if let (Some(tenant_str), Some(site_str)) = (tenant_id_str, site_id_str) {
            if let (Ok(tenant_id), Ok(site_id)) =
                (tenant_str.parse::<TenantId>(), site_str.parse::<SiteId>())
            {
                return Scope::Site { tenant_id, site_id };
            }
        } else if let Some(tenant_str) = tenant_id_str {
            if let Ok(tenant_id) = tenant_str.parse::<TenantId>() {
                return Scope::Tenant { tenant_id };
            }
        }
    }

    Scope::Global
}

/// Extractor to verify a specific role type.
/// The required `RoleType` is read from request extensions.
#[derive(Debug, Clone)]
pub struct RequireRole(pub RoleType);

impl<S> FromRequestParts<S> for RequireRole
where
    S: Send + Sync,
{
    type Rejection = ExtractorError;

    fn from_request_parts(
        parts: &mut Parts,
        state: &S,
    ) -> impl Future<Output = Result<Self, Self::Rejection>> + Send {
        // We must fetch from extensions before we pass parts to extract_scope_from_parts
        // because extract_scope_from_parts borrows parts mutably.
        let auth_user_opt = parts.extensions.get::<AuthenticatedUser>().cloned();
        let required_role_opt = parts.extensions.get::<RoleType>().cloned();

        let scope_fut = extract_scope_from_parts(parts, state);

        let res = async move {
            let scope = scope_fut.await;

            let auth_user = auth_user_opt.ok_or(ExtractorError::MissingIdentity)?;
            let required_role = required_role_opt.ok_or_else(|| {
                ExtractorError::Unauthorized("Role requirement not configured".into())
            })?;

            let mut has_role = false;
            for binding in &auth_user.bindings {
                if binding.user_id == auth_user.user_id
                    && binding.scope.matches(&scope)
                    && binding.role.role_type == required_role
                {
                    has_role = true;
                    break;
                }
            }

            if has_role {
                Ok(RequireRole(required_role))
            } else {
                Err(ExtractorError::Unauthorized(format!(
                    "Principal '{}' denied role '{:?}' on scope {:?}",
                    auth_user.user_id, required_role, scope
                )))
            }
        };

        Box::pin(res)
    }
}

/// Extractor to verify a specific permission.
/// The required `Permission` is read from request extensions.
#[derive(Debug, Clone)]
pub struct RequirePermission(pub Permission);

impl<S> FromRequestParts<S> for RequirePermission
where
    S: Send + Sync,
{
    type Rejection = ExtractorError;

    fn from_request_parts(
        parts: &mut Parts,
        state: &S,
    ) -> impl Future<Output = Result<Self, Self::Rejection>> + Send {
        let auth_user_opt = parts.extensions.get::<AuthenticatedUser>().cloned();
        let required_permission_opt = parts.extensions.get::<Permission>().cloned();

        let scope_fut = extract_scope_from_parts(parts, state);

        let res = async move {
            let scope = scope_fut.await;

            let auth_user = auth_user_opt.ok_or(ExtractorError::MissingIdentity)?;
            let required_permission = required_permission_opt.ok_or_else(|| {
                ExtractorError::Unauthorized("Permission requirement not configured".into())
            })?;

            if let Err(e) = AccessControl::evaluate_principal(
                auth_user.user_id,
                &auth_user.bindings,
                required_permission,
                &scope,
            ) {
                return Err(ExtractorError::Unauthorized(e.to_string()));
            }

            Ok(RequirePermission(required_permission))
        };

        Box::pin(res)
    }
}
