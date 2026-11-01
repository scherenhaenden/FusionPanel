# Nodes Module

Provides node management capabilities.

## Capabilities

- NODE-01: Rust core (Implemented: state, placement, revocation logic)
- NODE-02: Lightweight frontend (Blocked: Node UI not yet implemented)
- NODE-03: Agent separation (Blocked: Needs integration with agent module)
- NODE-04: Restricted privileged surface (Blocked: Needs integration with agent protocol)
- NODE-05: Authenticated RPC (Blocked: Needs integration with api module)
- NODE-06: Master and worker node management (Partial: State models present)
- NODE-07: Move and copy sites between nodes (Partial: Move plans present, copy missing)
- NODE-08: Load-aware placement (Implemented: Placement plans present)
- NODE-09: LIFO provisioning compensation (Blocked: Missing provisioning system)
- NODE-10: Control-state persistence (Blocked: Missing database integration)
- NODE-11: Durable jobs (Blocked: Missing jobs integration)
- NODE-13: OpenAPI contract (Blocked: Missing API definition)
- NODE-14: Operational and recovery CLI (Blocked: Missing CLI integration)
- NODE-15: AGPL licensing and source compliance (Implemented: License set in Cargo.toml)

## Supported Features

- Enrollment with invitation expiration and key identity
- Load-aware placement planning
- Revocation tracking
- Movement plan generation

## Unsupported Features

- NODE-07: Copy sites between nodes is not yet fully implemented, only move plans are currently supported. Follow-on task needed to support site copying.

## Tests

- Unit tests for node state, placement, and revocation.
- Real local service identities integration test blocked pending available fixture implementations.
- E2E test blocked pending real runtime dependencies (API, agent).

## Proposed shared changes

None at this time.
