# Identity and Permissions Module

Standalone identity and permissions module for FusionPanel (Work package S08).
Provides tenant/site-scoped RBAC, user entity lifecycle, stateful session management with idle/absolute TTL and instant revocation, Argon2id password hashing, and SQLite persistence.

## Capability Ownership and Status

| Capability ID | Capability Name | Status | Implemented Slice | Blocked / Missing Behavior / Follow-up |
|---|---|---|---|---|
| **SEC-01** | TOTP two-factor authentication | Blocked | Type stubs returning explicit `IdentityError::Unsupported` | RFC 6238 TOTP generation/verification and recovery codes require S25 API integration. |
| **SEC-02** | Passkeys and WebAuthn | Blocked | Type stubs returning explicit `IdentityError::Unsupported` | WebAuthn challenge-response ceremonies require frontend WebAuthn API (S17) and server-side FIDO2 validator (S25). |
| **SEC-03** | OAuth and SSO | Blocked | Type stubs returning explicit `IdentityError::Unsupported` | OIDC redirect handling, token exchange, and upstream provider configuration require S25/S36 integration. |
| **SEC-04** | Role-based access control | Implemented | Tenant/site-scoped RBAC matrix, deny-by-default access evaluator, role templates, user bindings | Integrated HTTP API route guards belong to S25. |
| **SEC-12** | Revocable sessions | Implemented | Stateful sessions, absolute TTL + idle timeout, single-session revocation, user/tenant bulk revocation tombstones, SQLite persistence | Integrated HTTP session cookie validation belongs to S25. |

## Supported Features and Security Design

- **Safe Rust**: Configured with `#![forbid(unsafe_code)]`.
- **Zero Secret Leakage**: Passwords and secrets are never stored in plain text, logged, or serialized into event/plan/status bodies. Passwords use Argon2id with cryptographically secure random salts.
- **Granular Scoping**: Scopes support `Global`, `Tenant { tenant_id }`, and `Site { tenant_id, site_id }`.
- **Deny-by-Default RBAC**: Access evaluation checks bindings and scopes strictly, returning `Err(IdentityError::Unauthorized)` unless explicitly permitted.
- **Revocable Sessions**: Enforces absolute expiration timestamps, rolling idle access deadlines, and revocation tombstones at single-session, user-wide, and tenant-wide levels.
- **Persistence**: Real SQLite persistence with foreign keys, cascading deletes, and transactional revocation recording.
- **Explicit Gaps**: Unsupported capabilities (SEC-01, SEC-02, SEC-03) fail explicitly with `IdentityError::Unsupported` rather than fabricated success.

## Verification

### Unit Tests
Verify permission matrices, expiry, revocation, password hashing, and capability stubs:
```sh
cargo test --manifest-path modules/identity/Cargo.toml --lib
```

### Integration Tests
Verify real SQLite persistence reopen and independent tenant isolation with disposable temporary databases:
```sh
cargo test --manifest-path modules/identity/Cargo.toml --test integration
```

### End-to-End Verification (Blocked Driver)
Exercise cross-tenant API isolation:
```sh
cargo test --manifest-path modules/identity/Cargo.toml --test e2e
```
When prerequisites (integrated S25 API server and driver) are missing, this test exits non-zero and explicitly records:
`RESULT=BLOCKED; missing FUSIONPANEL_IDENTITY_E2E_DRIVER; required: the integrated S25 HTTP API server, real multi-tenant sessions, and an executable E2E fixture driver`

## Proposed Shared Changes
- S25 (Identity/API integration) will integrate this standalone package into the root workspace and connect HTTP route guards and cookie authentication to `AccessControl`, `Session`, and `IdentityStorage`.
