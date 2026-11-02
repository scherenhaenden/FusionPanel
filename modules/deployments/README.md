# fp-deployments

This is the deployments module for FusionPanel, responsible for managing deployment workflows, environments, and deployment status.

## Capabilities

* **DEP-01: Git push-to-deploy** - Blocked (No Git repository integration)
* **DEP-02: Git webhooks** - Blocked (No webhooks implemented)
* **DEP-03: Multi-language builds** - Blocked (No builds implemented)
* **DEP-04: Blue-green deployments** - Blocked (No blue-green deployment implemented)
* **DEP-05: Per-commit preview environments** - Partial (Domain model supports `Environment::Preview`, but logic is unimplemented)
* **DEP-06: Staging** - Partial (Domain model supports `Environment::Staging`, but logic is unimplemented)
* **DEP-07: Release rollback** - Partial (Domain model supports `DeploymentStatus::RolledBack`, but actual rollback logic is unimplemented)
* **DEP-08: Scheduled deployments** - Blocked (No scheduling implemented)
* **DEP-09: Two-person deployment approval** - Blocked (No approval implemented)
* **DEP-10: Deployment history and progress** - Partial (Domain model tracks status, history storage unimplemented)
* **DEP-11: Failure diagnostics** - Blocked (No diagnostics implemented)
* **DEP-12: GitHub Actions integration** - Blocked (No integration implemented)

## Models

See `src/domain/models.rs` and `src/domain/state_machine.rs` for initial domain logic. Tests cover valid and negative transition state machine tests.
