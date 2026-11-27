# FusionPanel State Compatibility and Rollback Rules

This document defines the state compatibility rules enforced by `packaging/update.sh` and `packaging/rollback.sh` during staged upgrades and rollbacks.

## 1. Release Version Marker (`STATE_DIR/.version`)

- **Location:** `/var/lib/fusionpanel/.version` (or `$FUSIONPANEL_TEST_STATE_DIR/.version` in layout tests).
- **Preflight Validation:** Before stopping services or modifying `BACKUP_DIR`, `update.sh` verifies that `STATE_DIR/.version` is either absent (fresh or pre-versioned install) or a regular file (never a symlink or directory).
- **Adjacent Release Rule:** Semver releases (`X.Y.Z`) must step through adjacent releases:
  - Same `major.minor` (patch update or same-version reinstall)
  - Next minor in the same major (`X.(Y+1).*`)
  - Next major with minor `0` (`(X+1).0.*`)
  - Non-adjacent jumps or downgrades via `update.sh` are rejected in preflight unless `FUSIONPANEL_ALLOW_NON_ADJACENT_UPGRADE=1` is set (or the symbolic `latest` target is requested).

## 2. Disk Space Preflight (`FUSIONPANEL_MIN_FREE_KB`)

- When `FUSIONPANEL_MIN_FREE_KB` is set to a non-negative integer, `update.sh` checks available space via `df -Pk` on both `VERSION_LIB_DIR` and `BACKUP_DIR` (or their nearest existing ancestor directories) before stopping services.
- If available space is below `FUSIONPANEL_MIN_FREE_KB`, the update aborts cleanly without touching services or backups.

## 3. State Compatibility Manifest (`BACKUP_DIR/state-compatibility`)

During backup creation, `update.sh` writes `BACKUP_DIR/state-compatibility` with:

- `format=1`
- `previous_version`: previous `STATE_DIR/.version` value (or `none`)
- `target_version`: target release version
- `adjacent_check`: `passed`, `overridden`, `symbolic`, or `fresh`
- `incompatible_schema_migration`: `0` or `1`
- `recorded_at`: UTC timestamp

## 4. Schema & Durable State Migrations (`FUSIONPANEL_INCOMPATIBLE_SCHEMA_MIGRATION`)

- **Backward-compatible migrations (`0`, default):** Additive schema changes or idempotent config updates allow automatic rollback on activation/health failure and manual `rollback.sh` execution.
- **Incompatible migrations (`1`):** When `FUSIONPANEL_INCOMPATIBLE_SCHEMA_MIGRATION=1` is set, `update.sh` writes `BACKUP_DIR/incompatible-migration`.
  - If activation or post-activation health checks fail, `rollback.sh` refuses automatic rewind to prevent corrupting migrated durable state.
  - Operators must either restore from a pre-upgrade snapshot or explicitly override with `rollback.sh --force-rewind` or `FUSIONPANEL_ALLOW_INCOMPATIBLE_REWIND=1`.

## 5. Configuration & Service Migrations

- **API & Agent Environment (`fusionpanel.env`, `fusionpanel-agent.env`):** Backed up to `BACKUP_DIR/config/` before any migration (including `FUSIONPANEL_FRONTEND_DIR` updates and legacy `FUSIONPANEL_AGENT_ENDPOINT` migration). Rollback restores `BACKUP_DIR/config/` idempotently without overwriting `STATE_DIR` durable data.
- **Systemd Units:** Existing units are backed up to `BACKUP_DIR/systemd/` with `present` and `installed-checksums` manifests so newly introduced units are removed on rollback and prior units are restored.
- **Repeat Rollback:** `rollback.sh` preserves `BACKUP_DIR`, credentials in `CONFIG_DIR`, and durable state in `STATE_DIR`, allowing repeated `rollback.sh` invocations to succeed idempotently.
