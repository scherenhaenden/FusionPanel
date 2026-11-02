# Multi-node failure E2E tests

This module contains E2E tests for multi-node failure scenarios.
Because E2E requires multiple real VM nodes and external dependencies not currently available in this execution environment, all tests explicitly record a `BLOCKED` state.

## Shared changes proposed

No shared changes are proposed at this time.

## Tests

- `test_node_heartbeat_timeout`: Validates node heartbeat timeout detection.
- `test_node_health_transition`: Validates node health transition to degraded/offline status.
- `test_job_rescheduling_failover`: Validates job rescheduling failover simulation.
