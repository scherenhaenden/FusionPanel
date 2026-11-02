# Administration E2E Tests

This package provides End-to-End (E2E) verification for the Administration domains (ADM-01 through ADM-15).

## Capabilities tracked
* **ADM-04: Panel updates and recovery**: Partial. `test_inplace_updater_rollback_guard` is implemented but blocked awaiting disposable test environment.
* **ADM-05: User and role management**: Partial. `test_team_member_invitation_flow` is implemented but blocked awaiting disposable test environment.
* **ADM-06: Reseller hierarchy (Tenant Quotas)**: Partial. `test_organization_tenant_quotas` is implemented but blocked awaiting disposable test environment.
* **Audit Logging**: Partial. `test_audit_logging_immutability` is implemented but blocked awaiting disposable test environment.

All other ADM- capabilities are currently missing and represent follow-on work items.

## Execution
Run `cargo test --manifest-path tests/e2e/administration/Cargo.toml`.
If the `FUSIONPANEL_E2E_READY` environment variable is not set or prerequisites are unavailable, tests will explicitly panic with a `BLOCKED` message to prevent silent passes.
