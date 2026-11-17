# Sprint 9 Implementation Plan — Multi-User RBAC, Audit & MFA (`0.10.0-dev.1`)

## Goal
Deliver multi-user operator management, role bindings, scoped personal API tokens, persistent audit logging, TOTP MFA enrollment/verification, and live Administration/Security UI wiring.

## Branches & Tasks
1. `docs/sprint-9-plan` -> `sprint-9`: Sprint 9 plan and 2026-11-17 readiness snapshot/catalog artifacts.
2. `feat/sprint-9-rbac-users-api` -> `sprint-9` (`Refs #680`):
   - `crates/control-store`: Schema v10 (`api_tokens`, `audit_logs`, `user_mfa`).
   - `crates/api`: `/v1/admin/users`, `/v1/admin/tokens`, scoped `fp_tok_` Bearer token verification & RBAC enforcement (`403` for non-admin / insufficient scope).
3. `feat/sprint-9-audit-mfa-api` -> `sprint-9` (`Refs #681`):
   - `crates/api`: `/v1/audit` mutation history queries, `/v1/auth/mfa` (RFC 6238 TOTP enroll/verify/disable), and `/v1/security/rules` persistence.
4. `feat/sprint-9-security-admin-ui` -> `sprint-9` (`Refs #682`):
   - `frontend`: Typed API client methods, live Administration (Users, API Tokens, Audit Log) and Security (TOTP MFA, Security Rules) views, and Jest unit tests.
5. `test/sprint-9-rbac-audit-mfa-e2e` -> `sprint-9`:
   - `tests/e2e/hosting/fixtures/sprint9/`: Isolated Docker Compose + Playwright E2E verifying multi-user RBAC (`403` for `Viewer`), scoped API token enforcement, audit log entries, and TOTP MFA enrollment.
6. `chore/sprint-9-release-prep` -> `sprint-9` -> `master` PR (`0.10.0-dev.1`).
