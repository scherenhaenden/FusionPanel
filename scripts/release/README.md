# Release Integration Module

This package implements the `Release Integration` and regression domain (S61).

## Scope
- **Capabilities**: DEP-07 (Release rollback), SEC-17 (Release integrity and provenance)
- **Functions**: Validates release manifests and manages deployment records.

## Usage
The module handles a custom release manifest definition which validates component hashes, versions, and metadata.
Run tests using:
```
cargo test --manifest-path Cargo.toml
```
E2E tests will result in a `BLOCKED` panic because `fusionpanel-cli` is not installed on the base system yet. This follows the E2E verification policy.

## Proposed Shared Changes
- None at this time.
- Future work: move `ReleaseManifest` to shared workspace types once API contracts for DEP-07 and SEC-17 are fully specified.
