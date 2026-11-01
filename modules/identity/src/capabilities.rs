use serde::{Deserialize, Serialize};
use crate::error::IdentityError;

/// Capability Status for S08 owned security capabilities:
/// - SEC-01: TOTP two-factor authentication
/// - SEC-02: Passkeys and WebAuthn
/// - SEC-03: OAuth and SSO
/// - SEC-04: Role-based access control
/// - SEC-12: Revocable sessions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityStatus {
    Implemented,
    Partial,
    Blocked,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityInfo {
    pub id: &'static str,
    pub name: &'static str,
    pub status: CapabilityStatus,
    pub description: &'static str,
    pub missing_behavior: Option<&'static str>,
}

pub fn get_owned_capabilities() -> Vec<CapabilityInfo> {
    vec![
        CapabilityInfo {
            id: "SEC-01",
            name: "TOTP two-factor authentication",
            status: CapabilityStatus::Blocked,
            description: "TOTP secret generation, QR code provisioning, and time-based code verification.",
            missing_behavior: Some("TOTP generation, validation, and recovery codes require S25 API integration and RFC 6238 engine."),
        },
        CapabilityInfo {
            id: "SEC-02",
            name: "Passkeys and WebAuthn",
            status: CapabilityStatus::Blocked,
            description: "FIDO2 / WebAuthn public key credential registration and authentication.",
            missing_behavior: Some("WebAuthn challenge-response ceremonies require frontend WebAuthn API (S17) and server-side FIDO2 validator (S25)."),
        },
        CapabilityInfo {
            id: "SEC-03",
            name: "OAuth and SSO",
            status: CapabilityStatus::Blocked,
            description: "OpenID Connect and OAuth2 federated authentication providers.",
            missing_behavior: Some("OIDC redirect handling, token exchange, and upstream provider configuration require S25/S36 integration."),
        },
        CapabilityInfo {
            id: "SEC-04",
            name: "Role-based access control",
            status: CapabilityStatus::Implemented,
            description: "Tenant and site scoped RBAC matrix, deny-by-default evaluation, role templates and user bindings.",
            missing_behavior: None,
        },
        CapabilityInfo {
            id: "SEC-12",
            name: "Revocable sessions",
            status: CapabilityStatus::Implemented,
            description: "Stateful session issuance, configurable TTL and idle expiry, instant single-session revocation, and user/tenant bulk revocation.",
            missing_behavior: None,
        },
    ]
}

/// Explicit stubs for SEC-01, SEC-02, SEC-03 ensuring unsupported features
/// fail with explicit typed errors rather than silent success.
pub struct TotpService;

impl TotpService {
    pub fn setup_totp(_user_id: &str) -> Result<(), IdentityError> {
        Err(IdentityError::Unsupported {
            capability: "SEC-01: TOTP two-factor authentication is blocked pending S25 API integration".to_string(),
        })
    }
}

pub struct WebAuthnService;

impl WebAuthnService {
    pub fn register_passkey(_user_id: &str) -> Result<(), IdentityError> {
        Err(IdentityError::Unsupported {
            capability: "SEC-02: Passkeys and WebAuthn is blocked pending S17/S25 integration".to_string(),
        })
    }
}

pub struct SsoService;

impl SsoService {
    pub fn authenticate_oauth(_provider: &str) -> Result<(), IdentityError> {
        Err(IdentityError::Unsupported {
            capability: "SEC-03: OAuth and SSO is blocked pending S25/S36 integration".to_string(),
        })
    }
}
