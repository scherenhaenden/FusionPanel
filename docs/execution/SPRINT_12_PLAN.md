# Sprint 12 Implementation Plan · Safe Upgrade and Rollback

Kickoff checkpoint · 27 November 2026 · Europe/Berlin

## Base and Sequencing Note

- **Integration branch:** `sprint-12` branched from `master` (`a8861e6110efaae7dbc589474335ca98c93a44c2`, Sprint 9 merge).
- **Sequencing:** Sprint 10 (`sprint-10`) and Sprint 11 (`sprint-11`, mail core and Debian package) are in flight. Sprint 12 kickoff work proceeds in parallel on `sprint-12` with disjoint write paths (`packaging/`, `tests/e2e/administration/`, `docs/execution/`). Before Sprint 12 qualification and release PR to `master`, `sprint-12` must rebase/merge the verified `master` containing Sprints 10 and 11.
- **Related references:** [SPRINTS_11_20_PLAN.md](SPRINTS_11_20_PLAN.md) · [S38](../sessions/S38.md) · [S51](../sessions/S51.md) · [#42](https://github.com/scherenhaenden/FusionPanel/issues/42) · [#53](https://github.com/scherenhaenden/FusionPanel/issues/53)

## Operator Demo Target

Upgrade a working Debian install, see its tenant, node, and sites return with stable IDs, then restore the previous working release after a deliberately rejected candidate (and after an interrupted staging/activation boundary).

## Work Packages and Parallel Ownership

| Branch | Effort | Worktree | Exclusive Write Paths | Owner / Status |
| --- | --- | --- | --- | --- |
| `docs/sprint-12-plan` | 1 | `FusionPanel-s12-docs` | `docs/execution/SPRINT_12_PLAN.md` | Coordinator · In progress |
| `feat/sprint-12-upgrade-preflight` | 3 | `FusionPanel-s12-preflight` | `packaging/updater/`, `packaging/tests/unit_updater.rs` | Subagent 1 · In progress |
| `feat/sprint-12-upgrade-rollback` | 5 | `FusionPanel-s12-rollback` | `packaging/` (outside `packaging/updater/`) | Subagent 2 · In progress |
| `test/sprint-12-update-vm` | 5 | `FusionPanel-s12-update-vm` | `tests/e2e/administration/` | Subagent 3 · In progress |

## Supported Upgrade Pair & Compatibility Contract

- **Supported upgrade pair:** Adjacent minor/patch releases (e.g., `0.10.0` -> `0.11.0` or `0.11.0` -> `0.12.0`). Major version jumps, arbitrary downgrades, and non-Debian/RHEL distributions remain outside this lane.
- **State migration rule:** Every state/schema migration must declare whether it is backward-compatible with the previous release binary. An incompatible schema migration blocks automatic rewind until an explicit restore path is demonstrated.
- **Preserved state across upgrade/rollback:** `/etc/fusionpanel/*.env`, `/var/lib/fusionpanel/` durable state, tenant/node/site IDs, and credentials.

## Verification Commands

- **Updater unit tests & clippy:**
  `cargo test --manifest-path packaging/updater/Cargo.toml`
  `cargo clippy --manifest-path packaging/updater/Cargo.toml --all-targets -- -D warnings`
- **Packaging layout & rollback tests:**
  `bash packaging/tests/test_update_layout.sh`
  `bash packaging/tests/test_updater_cli.sh`
- **Administration E2E & VM lane:**
  `cargo test --manifest-path tests/e2e/administration/Cargo.toml`

## Release Gate Checklist

- [ ] Preflight rejects wrong source version, corrupt artifact/digest, duplicate request, and insufficient disk space with unchanged install.
- [ ] Staged upgrade preserves config, credentials, and durable state; activates candidate; verifies API and agent health.
- [ ] Rejected bad-health candidate triggers automatic rollback; repeat rollback leaves previous release healthy.
- [ ] Interrupted staging/activation boundary recovers cleanly without orphaned locks or partial binaries.
- [ ] Disposable two-release Debian VM lane executes full upgrade + bad-candidate rollback + teardown with recorded manifests, SHA-256 hashes, state comparisons, and elapsed time.
