# Administration E2E Tests

The suite starts the real administration router on loopback with a disposable audit journal and invitation journal. It verifies authenticated audit append and tamper detection after restart, invitation acceptance and revocation across restarts, and concurrent quota reservations through HTTP. Updater checks exercise the production updater against temporary files. No shared service, host firewall, or production credential is used.

Run the core suite with:

```sh
cargo test --manifest-path tests/e2e/administration/Cargo.toml --lib
cargo test --manifest-path tests/e2e/administration/Cargo.toml --test audit_logging --test invitations --test tenant_quotas --test updater_rollback --test update_vm_lane
```

## Sprint 12 Two-Release Debian VM Update Lane

- **Evidence schema**: `tests/e2e/administration/src/update_vm_evidence.rs` defines `UpdateVmEvidenceBundle`, `GateStatus` (`PASS`/`FAIL`/`BLOCKED`/`NOT_RUN`), `TeardownStatus`, SHA-256 helpers, validation, and JSON serialization.
- **Local loopback & interruption suite**: `tests/e2e/administration/tests/update_vm_lane.rs` exercises:
  - Two-release upgrade (`0.9.0` -> `0.10.0`) via `packaging/update.sh` in `TEST_MODE` against disposable directories and a loopback HTTP release + `/v1/health` server, verifying seeded tenant/node/site ID preservation and writing a validated `UpdateVmEvidenceBundle`.
  - Failed-health candidate rejection and automatic rollback (`0.9.0` -> `0.10.1-bad-health` -> `0.9.0`) via `packaging/update.sh` + `packaging/rollback.sh` when loopback `/v1/health` returns `503`.
  - Interrupted staging and post-activation/pre-cleanup recovery boundaries via `release_integration::updater::Updater`.
- **Full Debian VM fixture entrypoint**:
  - Script with teardown trap: `tests/e2e/administration/scripts/run_update_vm_lane.sh`
  - Ignored VM test: `debian_vm_two_release_upgrade_and_rollback_lane` (`#[ignore = "Requires disposable Debian VM fixture"]`).
  - Inside a disposable Debian VM fixture, run:
    ```sh
    FUSIONPANEL_VM_E2E_DISPOSABLE=1 \
    FUSIONPANEL_VM_OLD_VERSION=0.9.0 \
    FUSIONPANEL_VM_NEW_VERSION=0.10.0 \
    FUSIONPANEL_VM_OLD_SHA256=<64-hex> \
    FUSIONPANEL_VM_NEW_SHA256=<64-hex> \
    bash tests/e2e/administration/scripts/run_update_vm_lane.sh
    ```
