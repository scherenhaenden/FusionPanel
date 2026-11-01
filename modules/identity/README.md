# Identity module — recovered S08 implementation slice

This standalone package was recovered from Jules session `3198712460135711531` onto master commit `3a8bf9ef39c35a43b36771c575dbc3b11bc601ba`. Only the session's `modules/identity/**` files were recovered. The session artifact also contained unrelated changes and deletions; those were excluded.

This package is a draft implementation and is not accepted for runtime integration or merge.

## Present source behavior

- UUID identifiers and basic user metadata validation.
- Role templates, permission identifiers, tenant/site scope matching and a deny-by-default evaluator for a supplied list of bindings.
- Argon2 password hashing and verification.
- In-memory session expiry, idle expiry and revocation registries.
- Explicit unsupported errors for TOTP, WebAuthn and OAuth/SSO.

## Capability status

| Capability | Status at product scope | Remaining requirements |
|---|---|---|
| SEC-01 TOTP | BLOCKED | No TOTP engine, recovery codes or real enrollment flow. |
| SEC-02 Passkeys/WebAuthn | BLOCKED | No ceremony, challenge storage or server-side verification. |
| SEC-03 OAuth/SSO | BLOCKED | No provider exchange, identity binding or token validation. |
| SEC-04 RBAC | PARTIAL | Primitive evaluator only; no authenticated principal boundary, persistence or API integration. |
| SEC-12 Revocable sessions | PARTIAL | In-memory only; no durable issuance/revocation or integrated API enforcement. |

The `Implemented` values returned by the recovered capability metadata describe primitives only. They do not establish product readiness.

## Review blockers

- The package contains no unit, integration or E2E tests. A zero-test Cargo success is compilation evidence only.
- `AccessControl::evaluate` does not accept a principal ID or filter `RoleBinding.user_id`. Its caller must provide bindings belonging only to the authenticated principal; no integrated caller enforcing that boundary exists yet.
- Session fields are public and deserializable, so construction checks do not protect persisted or external input invariants.
- Positive TTL and idle-timeout inputs are not bounded before Chrono duration/date arithmetic. Extreme values can panic rather than return a typed error.
- There is no persistence implementation; the presence of a SQLite dependency does not establish storage or reopen behavior.
- Real tenant separation, demotion/revocation, restart recovery and cross-tenant API journeys remain unverified.

## Validation of the recovery

On 1 November 2026:

- `cargo test --offline --manifest-path modules/identity/Cargo.toml`: compiled successfully, **0 unit tests and 0 doc tests**.
- Initial formatting check failed on the recovered artifact; `cargo fmt --manifest-path modules/identity/Cargo.toml` applied formatting only.
- `cargo fmt --manifest-path modules/identity/Cargo.toml -- --check`: passed after formatting.
- `git diff --check`: passed.

No real-service integration or E2E command is available in this source slice. Those gates remain BLOCKED. Issue #11 must remain open.
