# Deployments Module

## Overview
This module handles deployment workflows, focusing currently on Git webhook payload verification for GitHub and GitLab.

## Current Capabilities (Implemented)
- **DEP-02: Git webhooks (Partial)**
  - Implemented secure webhook signature validation (HMAC SHA-256 for GitHub, simple token comparison for GitLab) using timing-safe operations.
  - Implemented payload event parsing to detect Push and Tag events.

## Missing Capabilities (Blocked/Pending)
- **DEP-01: Git push-to-deploy**
- **DEP-02: Git webhooks (Remaining)**
  - Connecting verified events to an actual deployment trigger logic.
- **DEP-03: Multi-language builds**
- **DEP-04: Blue-green deployments**
- **DEP-05: Per-commit preview environments**
- **DEP-06: Staging**
- **DEP-07: Release rollback**
- **DEP-08: Scheduled deployments**
- **DEP-09: Two-person deployment approval**
- **DEP-10: Deployment history and progress**
- **DEP-11: Failure diagnostics**
- **DEP-12: GitHub Actions integration**

## Tests
Run unit tests with `cargo test --manifest-path modules/deployments/Cargo.toml`.
