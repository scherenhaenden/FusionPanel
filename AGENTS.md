# Development instructions

All documentation, code comments, issues and pull requests are in English. Follow the exclusive write paths in your assigned task. Do not change sibling packages, root manifests, root Cargo.lock, or shared documentation in the parallel foundation batch. Dependencies may be added only to your own Cargo.toml. Do not commit Cargo.lock in this batch; integration owns it.

Read docs/FOUNDATION_CONTRACTS.md before implementation. Use safe Rust and forbid unsafe code. Never log credentials or return fabricated success for unsupported functionality. Do not execute privileged commands or contact production services. Add focused tests of meaningful behavior. Create a separate branch and PR against master; do not merge.

Write original implementation for the specified contracts. Preserve required license and copyright notices for any third-party material you introduce.

## Execution-window rules

The canonical branch is master. Read docs/execution/DELIVERY_PLAN.md and your task spec. Independent modules may edit only their assigned module and must use a standalone `[workspace]` until integration. The root-manifest prohibition applies except when your task explicitly assigns those files. Never remove a capability or user-visible mode to simplify implementation. Follow docs/engineering/FEATURE_PRESERVATION.md and docs/engineering/VARIANT_DESIGN.md. Read docs/testing/TEST_STRATEGY.md; no mocked success may qualify a real dependency or E2E journey. Missing prerequisites are BLOCKED, never PASS. Record actual tests, limits and unresolved gaps. Preserve required notices for reused material.
