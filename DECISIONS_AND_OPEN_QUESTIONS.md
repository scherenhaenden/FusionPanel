# Decisions and Open Questions

## Decisions

- FusionPanel is the working project name and repository name.
- Initial hosting is private on GitHub.
- The product scope is the capability catalog, delivered in stages.
- Initial implementation uses Rust, independent packages and explicit interface contracts.
- SQLite is used for the first durable job-store implementation.
- Five implementation sessions work on disjoint package directories and submit separate pull requests without merging.

## Open questions

- Public branding, final frontend, control-state architecture and node transport authentication.
- Supported OS/architectures, performance targets and multi-node availability objectives.
- Implementation capacity, delivery dates and first usable hosting release scope.
- Actual production workload inventory, backup recovery evidence and cutover objectives.

No scaffold, session result or package-level test establishes production readiness.
