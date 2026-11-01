# Feature Preservation Policy

## Invariant

The capability catalog is a minimum product commitment. Delivery stages, refactoring, common abstractions and a different UI must not silently remove a capability, useful workflow or operational control. Preserve useful behavior, not incidental bugs or unsafe defaults. Record any deliberate compatibility break with its reason, affected data and migration path for owner review.

## Black-box and white-box characterization

For each capability, inventory observable user journeys: inputs, options/defaults, permissions, progress, result, errors, retry/cancel, persistence and recovery. Exercise real isolated services and capture sanitized expected outcomes. Review implementation paths and tests to understand mechanisms, invariants and failure boundaries. Reuse proven designs and compatible implementation where appropriate; do not reimplement for novelty. Any third-party code/assets/dependencies introduced must retain required license/copyright notices and satisfy the project's distribution obligations.

Repository product documentation describes FusionPanel behavior and its own mode names. Behavior observations, planned work and delivered behavior must remain distinguishable. Never turn a comparator score or plan into a support claim.

## States and evidence

PLANNED → IMPLEMENTING → IMPLEMENTED → VERIFIED. BLOCKED records a dependency; PARTIAL records exact supported and missing behavior. RESEARCH is not a support guarantee. DEFERRED requires an explicit owner scope decision and remains visible. A regression from VERIFIED reopens the feature; do not delete its row. Completion requires source location, supported modes/platforms, tests at all applicable layers, observed output, limitations and recovery notes.

Every capability must appear exactly once in the ledger with implementation, integration, E2E and closure owners. A new subfeature gets a stable sub-ID and a test owner. Mode-specific acceptance cases use the same capability ID plus a mode suffix. Refactors must rerun the union of affected mode cases.

## Definition of done

The user can achieve the documented outcome through the supported live UI/API/CLI path, authorization is consistent, durable state survives the declared restart/failure scenarios, recovery is tested and the documentation matches observed behavior. A test that only inspects generated config cannot certify a running service. A PR, completed remote session or green mock suite is not independently sufficient.
