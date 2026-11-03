# deployments-e2e

## Description
This package provides End-to-End (E2E) automated tests for deployment mechanisms within FusionPanel, addressing Zero-downtime deployment pipelines.

## Tests included
- **Atomic symlink switching**: Verifies that when switching out active versions (via `current` symlink), it occurs atomically through a `.tmp` file and a filesystem `rename` operation to avoid application downtime.
- **Release rollback**: Validates that rollback properly reinstates the previous valid release path by safely atomically updating the symlink.
- **Git webhook pipeline triggers**: Ensures deployment pipelines activate exclusively upon appropriate git webhook events (like commits to `main`).

## Supported and Unsupported Features
- **Supported**: POSIX compliant atomic symlinks using `std::os::unix::fs::symlink` and temporary directories using standard library functions without relying on dependencies.
- **Unsupported**: Actual HTTP webhooks triggering are mocked due to E2E test constraints ensuring isolated environment execution.

## Changes
This module runs completely isolated from the standard workspace build with an independent `Cargo.toml`.
