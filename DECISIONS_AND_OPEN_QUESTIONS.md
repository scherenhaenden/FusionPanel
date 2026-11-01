# Decisions and Open Questions

## Decisions

- FusionPanel is the working project name and repository name.
- The owner authorized public open-source hosting on 2026-11-01, superseding the initial private setting.
- The product scope is the capability catalog, delivered in stages.
- Initial implementation uses Rust, independent packages and explicit interface contracts.
- SQLite is used for the first durable job-store implementation.
- Five implementation sessions work on disjoint package directories and submit separate pull requests against master without merging.

## Open questions

- Public branding, final frontend, control-state architecture and node transport authentication.
- Supported OS/architectures, performance targets and multi-node availability objectives.
- Actual Jules capacity, prerequisite review/merge turnaround and remaining blockers against the Nov 1–11 delivery plan.
- Actual production workload inventory, backup recovery evidence and cutover objectives.

No scaffold, session result or package-level test establishes production readiness.
