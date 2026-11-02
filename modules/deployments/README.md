# Deployments Module

## Overview
This module handles deployment workflows, including git builds, webhooks, blue-green deployments, releases, and rollbacks.

## Implemented Capabilities
- DEP-04: Blue-green deployments (Partial - Atomic symlink switcher implemented)
- DEP-07: Release rollback (Partial - Core atomic switcher enables this)

## Blocked/Partial Capabilities
- DEP-01: Git push-to-deploy (Missing: Complete implementation)
- DEP-02: Git webhooks (Missing: Complete implementation)
- DEP-03: Multi-language builds (Missing: Complete implementation)
- DEP-04: Blue-green deployments (Missing: Full orchestrator)
- DEP-05: Per-commit preview environments (Missing: Complete implementation)
- DEP-06: Staging (Missing: Complete implementation)
- DEP-07: Release rollback (Missing: Full orchestrator API)
- DEP-08: Scheduled deployments (Missing: Complete implementation)
- DEP-09: Two-person deployment approval (Missing: Complete implementation)
- DEP-10: Deployment history and progress (Missing: Complete implementation)
- DEP-11: Failure diagnostics (Missing: Complete implementation)
- DEP-12: GitHub Actions integration (Missing: Complete implementation)

## Follow-on Work
- Implement git push-to-deploy receivers.
- Implement webhooks and payload verification.
- Implement the deployment orchestrator.
- Implement rollback control APIs.

## Testing
Unit tests can be run via:
```bash
cargo test --manifest-path Cargo.toml
```

## Shared Changes Proposed
None at this time.
