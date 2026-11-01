# Behavior Characterization Record

Use black-box observation to preserve user outcomes and white-box analysis to understand mechanisms and failure boundaries. Avoid unnecessary reinvention, but do not preserve unsafe bugs or introduce material whose license conditions are not satisfied. Characterization describes expected behavior; acceptance proves FusionPanel's actual implementation.

For each capability and mode, record the following before consolidation:

1. Stable capability/mode ID, supported environment and prerequisite resources.
2. User entry points: UI, API, CLI; visible controls, defaults and permissions.
3. Valid inputs, observed output, durable state, actual external side effects and performance context.
4. Error/partial/cancel/retry behavior, state after interruption, cleanup and recovery path.
5. Internal invariants: validation, ownership, authorization, secret handling, state transitions and transaction/compensation boundaries.
6. Distinct useful user workflow variants and their compatibility constraints.
7. Unit cases, real dependency integration cases and user-visible E2E acceptance cases.
8. Deliberate changes with rationale and owner review; unresolved gaps remain in the ledger.
9. Sanitized evidence artifacts and exact tested build/environment identifiers.
10. Implementation PR, reviewer and final verification state.

Do not reduce several materially different workflows to an undocumented default. Plan the mode selection and persistence explicitly; if switching moves data or service ownership, expose a reviewed migration job and recovery boundary.
