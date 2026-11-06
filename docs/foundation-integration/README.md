# Foundation Integration

This directory contains integration documentation and artifacts for foundation crates (`crates/domain`, `crates/agent-protocol`, `crates/jobs`, `crates/cli`).

## Contracts

Foundation contracts are implemented as follows:
- Domain: Models and enumerations for entities in the system.
- Agent Protocol: V1 JSON requests/responses using serde JSON tagging. Operations include `system.info` and `site.validate`. Unknown fields are rejected.
- Jobs: Uses `rusqlite` for persistent state, providing robust job claiming, transitions, and large error message bounding.
- CLI: Uses `reqwest` and `clap` to provide `health` and `capabilities` endpoints. Outputs are JSON or human-readable.

These foundation implementations form the bedrock for higher-level module integrations, maintaining `#![forbid(unsafe_code)]` and zero mock passes policies.
