# FusionPanel

A self-hosted server and hosting control panel for managing websites, containers, deployments, databases, DNS, email, security and operations through one interface, API and CLI.

## Status

**Open-source software, licensed under AGPL-3.0-only. Active early development.** The initial milestone establishes the Rust foundation; production hosting features are planned and must not be advertised as available before acceptance testing.

## Product principles

- One authoritative owner for each managed resource and service configuration.
- An unprivileged control plane and a narrowly scoped privileged node agent.
- Shared authorization, durable jobs, audit events and secrets management.
- Recoverable operations and independent, tested backup restoration.
- Consistent workflows across UI, API and CLI.

## Development

The Rust workspace contains five independently scoped foundation packages. See [development contracts](docs/FOUNDATION_CONTRACTS.md) and [initial tasks](docs/INITIAL_TASKS.md). Build and test with `cargo test --workspace --locked` after the integration task has generated the shared lockfile; individual sessions validate their own packages without committing the workspace lockfile.

## Documentation

- [Delivery plan and 63-session table](docs/execution/DELIVERY_PLAN.md)
- [Capability ownership and evidence ledger](docs/execution/CAPABILITY_LEDGER.md)
- [Feature preservation](docs/engineering/FEATURE_PRESERVATION.md)
- [Selectable modes](docs/engineering/VARIANT_DESIGN.md)
- [Real-system test strategy](docs/testing/TEST_STRATEGY.md)
- [Project charter](PROJECT_CHARTER.md)
- [Capability catalog](CAPABILITY_CATALOG.md)
- [Architecture](ARCHITECTURE.md)
- [Roadmap](ROADMAP.md)
- [Test plan](TEST_PLAN.md)
- [Migration plan](ARCHITECTURE_AND_MIGRATION.md)
- [Decisions and open questions](DECISIONS_AND_OPEN_QUESTIONS.md)

License: AGPL-3.0-only. Preserve applicable copyright and license notices when introducing dependencies or third-party material.
