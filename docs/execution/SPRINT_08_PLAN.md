# Sprint 8 Plan — File Manager & Cron Jobs

**Base branch:** `sprint-8` (branched from `master` at `d552948`)  
**Target release:** `0.9.0-dev.1`

## Goal
Enable tenants to browse, upload, edit, rename, and delete files inside their managed website document roots (with strict canonicalized jail enforcement rejecting `..` traversal and symlink escapes) and schedule/run per-tenant Cron jobs.

## Atomic Branches (`-> sprint-8`)

### 1. `feat/sprint-8-files-api`
- `crates/agent-protocol`: Add `Operation::CronApply` (`cron.apply`), `Operation::CronDelete` (`cron.delete`), `Operation::CronRunNow` (`cron.run_now`), `CronApplyPayload`, `CronDeletePayload`, `CronRunNowPayload`, and strict 5-field cron schedule + bounded command validation.
- `crates/control-store`: Schema v9 (`cron_jobs`, `cron_operations`) with tenant-scoped state (`provisioning` -> `ready` | `failed` | `deleting`), `last_run_at`, `last_run_status`, and idempotent reservation/completion.
- `crates/api`:
  - `crates/api/src/files.rs`: Tenant-scoped jailed File Manager endpoints for managed static sites (`/v1/files/{site_id}/list`, `/read`, `/write`, `/rename`, `/delete`) enforcing `canonicalize()` containment inside `document_root` and rejecting `..` traversal or external symlinks.
  - `crates/api/src/cron.rs`: Tenant-scoped Cron endpoints (`GET/POST /v1/cron`, `DELETE /v1/cron/{id}`, `POST /v1/cron/{id}/run`) + `node_jobs.rs` completion wiring.

### 2. `feat/sprint-8-cron-api-agent`
- `crates/agent`: Implement `cron_driver.rs` for atomic per-tenant crontab rendering (`FUSIONPANEL_CRON_DIR`, default `/etc/cron.d`) and bounded `cron.run_now` execution with timeout + output capture; wire `CronApply`, `CronDelete`, `CronRunNow` in `worker.rs`.

### 3. `feat/sprint-8-files-cron-ui`
- `frontend`: Add `/files` and `/cron` routes (`App.tsx`) and sidebar links (`DashboardLayout.tsx`), typed File Manager & Cron API methods in `frontend/src/api/index.ts`, `FilesView`, `CronView`, and Jest tests in `FilesCron.test.tsx`.

### 4. `test/sprint-8-files-cron-e2e-fixture`
- `tests/e2e/hosting/fixtures/sprint8/`: Bounded Docker Compose fixture (`api` + `node` with Nginx & shared `/srv/fusionpanel/sites` + `cron.d`) + Playwright spec verifying file upload into static site served over HTTP `200 OK`, `../` traversal rejection, and cron creation + `Run Now` execution.
