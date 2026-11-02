# Deployments Module

## APIs
- `rollback::manager`: Coordinates rollback execution logic.
- `rollback::inventory`: In-memory and persistent inventory of releases for targets.

## Tests
Unit tests are located in `tests/unit_rollback.rs`. Run with `cargo test --manifest-path modules/deployments/Cargo.toml`.

## Modes
Currently supports simple fast rollback to the last known healthy state.

## Supported Features
- Tracking release history per target (retention of N releases).
- Finding the last healthy release.
- Reverting to the last healthy release.

## Unsupported Features
- Multi-region or multi-environment rollbacks.
- State mutation checks during rollback.

## Proposed Shared Changes
None at this time.
