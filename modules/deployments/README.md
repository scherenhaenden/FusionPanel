# Deployments Module

This standalone package implements the deployment workflows core contracts, git webhook processing, build execution planning, release directory management with atomic rollback, two-person approvals, and preview environment lifecycles for FusionPanel.

## Capability status

Status here is specific to this package. The repository capability ledger remains authoritative for product acceptance.

| Capability | Package status | Implemented behavior | Missing behavior / follow-up |
|---|---|---|---|
| DEP-01: Git push-to-deploy | Partial | Webhook parsing for push events, branch extraction, Git source derivation, and build trigger planning | Live git push receive hook and daemon daemon integration; see [S30 deployments runtime/API, issue #31](https://github.com/scherenhaenden/FusionPanel/issues/31). |
| DEP-02: Git webhooks | Implemented | Constant-time HMAC-SHA256 signature verification, replay protection with timestamp drift and duplicate delivery prevention, GitHub event parsing | None in this module slice. |
| DEP-03: Multi-language builds | Partial | Automatic project type detection (Rust, Node, Python, Static, Custom), isolated build execution runner, secret redaction in build output | Language container image caching and buildpack executors; see issue #31. |
| DEP-04: Blue-green deployments | Partial | Blue/green slot directory management, slot file atomic cutover, and pre-cutover health check verification guard | Real reverse proxy reload and traffic routing integration; see issue #31. |
| DEP-05: Per-commit preview environments | Partial | Ephemeral preview lifecycle models, expiry tracking, teardown flags, and canonical ASCII DNS validation | Dynamic DNS zone provisioning and TLS certificate generation for previews; see issue #16 and issue #31. |
| DEP-06: Staging | Partial | Environment promotion typed models (Preview -> Staging -> Production) and environment boundary validation | Staging service orchestration; see issue #31. |
| DEP-07: Release rollback | Implemented | Atomic directory symlink cutover and rollback to prior releases, release history enumeration, and pruning while strictly preserving the active release | None in this module slice. |
| DEP-08: Scheduled deployments | Partial | Schedule maintenance window verification, postponement, and future time checks | Background scheduler daemon; see issue #31. |
| DEP-09: Two-person deployment approval | Implemented | Quorum approval records, strict non-self-approval enforcement (initiator cannot approve own deployment), immutable decision recording | Role-based permission integration with user service. |
| DEP-10: Deployment history and progress | Implemented | Ordered deployment state machine with terminal state invariants (Queued -> PendingApproval -> Building -> Deploying -> Active / RolledBack / Failed / Cancelled), progress event log with severity levels | Persistent database storage; see issue #31. |
| DEP-11: Failure diagnostics | Implemented | Exit status code capture, error categorization, and redaction of tokens/secrets in stdout/stderr | None in this module slice. |
| DEP-12: GitHub Actions integration | Partial | Webhook payload parsing and push event source models | Outbound GitHub deployment status callback dispatch; see issue #31. |

## Package API

- `webhook::verify_signature(secret, payload, signature_header)` verifies HMAC-SHA256 signatures in constant time.
- `webhook::ReplayGuard` checks timestamp tolerance and deduplicates delivery IDs.
- `webhook::parse_github_webhook(event_type, payload)` parses GitHub push and ping webhook payloads.
- `build::detect_project_type(path)` identifies project types by repository manifest files.
- `build::plan_build(path, spec, output_dir)` generates safe build commands without shell interpolation.
- `build::execute_build(working_dir, planned, secret_redaction_list)` executes build commands in isolated environments and sanitizes output logs.
- `release::ReleaseManager` handles isolated release directories, atomic symlink switching, rollbacks, and active release-safe pruning.
- `workflow::ApprovalRecord` enforces two-person approval rules.
- `workflow::PreviewEnvironment` validates preview domains against canonical ASCII DNS rules and manages TTL.
- `workflow::DeploymentRecord` manages valid deployment lifecycle state transitions.

## Verification

### Unit verification

Run the local unit test suite (17 tests):

```sh
cargo test --offline --manifest-path modules/deployments/Cargo.toml --lib
```

### Integration verification

Integration test using a real local git repository, real build runner, isolated release directories, atomic symlink cutover, and rollback verifying restored content:

```sh
cargo test --offline --manifest-path modules/deployments/Cargo.toml --test integration
```

### End-to-end verification

E2E test gate requires an executable driver that interacts with a live production server and real network cutover:

```sh
FUSIONPANEL_DEPLOYMENTS_E2E_DRIVER=/absolute/path/to/driver \
  cargo test --offline --manifest-path modules/deployments/Cargo.toml --test e2e -- --nocapture
```

Without an executable driver configured in `FUSIONPANEL_DEPLOYMENTS_E2E_DRIVER`, the test exits non-zero with `RESULT=BLOCKED; missing FUSIONPANEL_DEPLOYMENTS_E2E_DRIVER; required: live production HTTP server, real network cutover, and an executable fixture driver`. A blocked result is not a pass.

## Supported and Unsupported Features

### Supported
- Webhook signature validation (HMAC-SHA256) with constant-time equality.
- Replay attack rejection through delivery ID deduplication and timestamp tolerance.
- Multi-language build detection and execution without shell expansion.
- Atomic release directory pointer switching and rollbacks.
- Retention pruning that guarantees the currently active release is never deleted.
- Two-person deployment approval preventing self-approval.
- Canonical ASCII DNS validation for preview environments.

### Unsupported
- Shell interpolation and raw script execution without explicit program binaries.
- IDNA/Unicode U-labels for preview domain names (must be ASCII DNS names).
- Arbitrary database schema rollbacks (deployment rollback covers application files/symlinks only).
- Direct host mutation outside the configured release directories.

## Proposed shared changes

- Root workspace integration: Once reviewed, `modules/deployments` should be added to the members list of the root `Cargo.toml`.
- Coordinate deployment job contracts with `crates/jobs`.
