# Development instructions

All documentation, code comments, issues and pull requests are in English. Follow the exclusive write paths in your assigned task. Do not change sibling packages, root manifests, root Cargo.lock, or shared documentation in the parallel foundation batch. Dependencies may be added only to your own Cargo.toml. Do not commit Cargo.lock in this batch; integration owns it.

Read docs/FOUNDATION_CONTRACTS.md before implementation. Use safe Rust and forbid unsafe code. Never log credentials or return fabricated success for unsupported functionality. Do not execute privileged commands or contact production services. Add focused tests of meaningful behavior. Create a separate branch and PR against main; do not merge.

Write original implementation for the specified contracts. Preserve required license and copyright notices for any third-party material you introduce.
