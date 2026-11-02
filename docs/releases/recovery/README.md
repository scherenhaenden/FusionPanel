# Recovery and Operator Runbook Tests

This module provides test fixtures and specifications for operator recovery workflows.
Due to strict write constraints in this task (exclusive paths: `docs/operations/`, `docs/releases/recovery/`), we log `BLOCKED` states in E2E/integration tests as the real API and CLI requirements (like `fusionpanel-cli` recovery commands, live Docker fixtures) are not yet integrated into this boundary or accessible for a true E2E execution at this phase.

## Usage
Run tests with:
```bash
cargo test --manifest-path docs/releases/recovery/Cargo.toml
```

## Proposed Shared Changes
- Expose administrative CLI endpoints for triggering maintenance mode.
- Integrate the target database/file restore flows tested here into `fusionpanel-cli`.
