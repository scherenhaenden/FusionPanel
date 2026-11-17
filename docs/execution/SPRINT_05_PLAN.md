# Sprint 5 Plan — Databases Core (PostgreSQL & MariaDB)

**Base branch:** `sprint-5` (branched from `master` at `2aa430b`)  
**Target release:** `0.6.0-dev.1`

## Goal
Enable operators to provision and delete tenant-scoped PostgreSQL and MariaDB databases and users via the Control Plane API and Web UI, execute database lifecycle operations through `fusionpanel-agent` using `modules/databases` (`PostgresAdapter` and `MariaDbAdapter`), enforce cross-tenant isolation, and verify end-to-end with real `psql` and `mariadb` clients in a bounded Docker fixture.

## Atomic Branches (`-> sprint-5`)

### 1. `feat/sprint-5-databases-api`
- `crates/agent-protocol`: Add `Operation::DatabaseApply`, `Operation::DatabaseDelete`, `Operation::DatabaseUserCreate`, `DatabaseEngineKind` (`postgresql` | `mariadb`), typed payloads, and strict SQL identifier/reserved-name validation.
- `crates/control-store`: Schema v6 (`databases`, `database_users`, `database_operations`) with tenant-scoped state machines, idempotent reservation, and completion transitions.
- `crates/api`: Mount `GET /v1/databases`, `GET /v1/databases/{id}`, `POST /v1/databases`, `DELETE /v1/databases/{id}`, and `POST /v1/databases/{id}/users` with RBAC + tenant isolation + operator job password redaction; reject `database.*` on generic `POST /v1/jobs`.

### 2. `feat/sprint-5-db-agent-driver`
- `crates/agent`: Wire `modules/databases` (`PostgresAdapter` / `MariaDbAdapter` configured via `FUSIONPANEL_PG_*` and `FUSIONPANEL_MARIADB_*` env vars) into `worker.rs` for `database.apply`, `database.delete`, and `database.user.create`, ensuring least-privilege per-database grants and secret redaction in logs.

### 3. `feat/sprint-5-databases-ui`
- `frontend`: Add typed database & user API client methods, update `DatabasesView` for PostgreSQL & MariaDB creation, polling, user/one-time credential modal, and deletion, with Jest tests in `Databases.test.tsx`.

### 4. `test/sprint-5-databases-e2e-fixture`
- `tests/e2e/hosting/fixtures/sprint5/` (or `tests/e2e/databases/fixtures/sprint5/`): Bounded Docker Compose fixture with PostgreSQL, MariaDB, API, and Agent (`psql` + `mariadb` clients) + Playwright spec verifying UI creation, real client login, cross-tenant isolation, and clean drop.
