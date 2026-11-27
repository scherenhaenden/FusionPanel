# Sprint 12 Evidence & Handoff Log · Safe Upgrade and Rollback

Checkpoint · 27 November 2026 · Europe/Berlin

## Pull Requests Merged into `sprint-12`

| PR | Branch | Scope |
| --- | --- | --- |
| #710 | `docs/sprint-12-plan` | Sprint 12 kickoff plan (`docs/execution/SPRINT_12_PLAN.md`) |
| #711 | `feat/sprint-12-upgrade-preflight` | Upgrade preflight validation in `packaging/updater/` and unit tests in `packaging/tests/unit_updater.rs` |
| #712 | `feat/sprint-12-upgrade-rollback` | Staged upgrade preflight, state compatibility rules (`packaging/STATE_COMPATIBILITY.md`), API+agent health checks, and repeat-rollback hardening in `packaging/` |
| #713 | `test/sprint-12-update-vm` | Two-release update VM E2E lane scaffolding, evidence schema (`UpdateVmEvidenceBundle`), and loopback/interruption tests in `tests/e2e/administration/` |

## Verification Results on `sprint-12` (`45d84c0`)

| Check | Command | Result |
| --- | --- | --- |
| Updater unit tests | `cargo test --manifest-path packaging/updater/Cargo.toml` | **PASS** (`src/main.rs`: 3 passed, `unit_updater.rs`: 9 passed) |
| Updater clippy | `cargo clippy --manifest-path packaging/updater/Cargo.toml --all-targets -- -D warnings` | **PASS** (0 warnings, 0 errors) |
| Packaging layout & rollback | `bash packaging/tests/test_update_layout.sh` | **PASS** (`Updater package-layout and rollback tests passed.`) |
| Updater CLI tests | `bash packaging/tests/test_updater_cli.sh` | **PASS** (`All updater CLI tests passed.`) |
| Admin E2E & VM lane (local) | `cargo test --manifest-path tests/e2e/administration/Cargo.toml --lib --test audit_logging --test invitations --test tenant_quotas --test updater_rollback --test update_vm_lane` | **PASS** (`lib`: 3, `audit_logging`: 1, `invitations`: 1, `tenant_quotas`: 1, `updater_rollback`: 2, `update_vm_lane`: 3 passed, 1 ignored) |
| Full Debian VM lane | `tests/e2e/administration/scripts/run_update_vm_lane.sh` (`debian_vm_two_release_upgrade_and_rollback_lane`) | **NOT_RUN / BLOCKED** (requires disposable Debian VM fixture and S10/S11 landed on `master`) |

## Release Gate Prerequisite Before Merging `sprint-12 -> master`

1. Rebase/merge `sprint-12` onto `master` once Sprint 10 and Sprint 11 land on `master`.
2. Execute the disposable two-release Debian VM lane (`debian_vm_two_release_upgrade_and_rollback_lane`) with real `.deb` / release artifacts from Sprint 11 and record the resulting evidence bundle.
