# Sprint 6 Plan — Backups & Restore Core

**Base branch:** `sprint-6` (branched from `master` at `7617a9c`)  
**Target release:** `0.7.0-dev.1`

## Goal
Enable operators to create consistent snapshots of managed websites (site files) and databases (PostgreSQL / MariaDB dumps) to local or S3-compatible storage, inspect backup archives, and restore snapshots on demand with SHA-256 manifest verification and staged atomic swaps.

## Atomic Branches (`-> sprint-6`)

### 1. `feat/sprint-6-backups-api`
- `crates/agent-protocol`: Add `Operation::BackupCreate` (`backup.create`), `Operation::BackupRestore` (`backup.restore`), `BackupTarget` (`site` / `database` / `bundle`), `BackupDestination` (`local` / `s3`), and strict path/identifier/S3-endpoint validation.
- `crates/control-store`: Schema v7 (`backups`, `backup_operations`) with tenant-scoped state machine (`creating` -> `ready` | `failed`, `restoring` -> `ready` | `failed`), archive metadata (`manifest_sha256`, `size_bytes`, `storage_key`), and idempotent reservation.
- `crates/api`: Mount `GET /v1/backups`, `GET /v1/backups/{id}`, `POST /v1/backups`, and `POST /v1/backups/{id}/restore` with RBAC (`Permission::BackupCreate` / `Permission::BackupRestore`), tenant ownership, and S3 secret redaction in `sanitize_job_for_operator`.

### 2. `feat/sprint-6-backup-s3-worker`
- `crates/agent`: Implement `backup_driver.rs` supporting:
  - Archiving site files (`tar.gz`) and dumping PostgreSQL (`pg_dump`) / MariaDB (`mysqldump`/`mariadb-dump`) with per-artifact SHA-256 manifest.
  - Local directory destination and S3-compatible destination (SigV4 PUT/GET via `reqwest` + `hmac`/`sha2`).
  - Staged restore with SHA-256 verification before atomic file/DB restore.
  - Secret redaction in logs (`s3_secret_key`, DB admin passwords).

### 3. `feat/sprint-6-backups-ui`
- `frontend`: Update `api/index.ts`, `views/Backups/*` (`index.tsx`, `ArchiveList.tsx`, `BackupModal.tsx`, `RestoreWizard.tsx`, `types.ts`) for creating local/S3 backups, polling status, and triggering restores, with Jest tests in `Backups.test.tsx`.

### 4. `test/sprint-6-backups-e2e-fixture`
- `tests/e2e/hosting/fixtures/sprint6/`: Bounded Docker Compose fixture (`minio` S3 + `postgres:15-alpine` + `api` + `node`) + Playwright spec populating a site file + DB row, snapping to MinIO S3 via UI, deleting the file & DB row, restoring via UI, and verifying byte + row integrity.
